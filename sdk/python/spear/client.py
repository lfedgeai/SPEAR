#!/usr/bin/env python3
import logging
import os
import queue
import selectors
import socket
import struct
import threading
import time
import traceback
from typing import Callable

import flatbuffers as fbs

from spear.proto.custom import (CustomRequest, CustomResponse,
                                NormalRequestInfo, RequestInfo)
from spear.proto.stream import StreamEvent
from spear.proto.tool import (InternalToolInfo, ToolInfo,
                              ToolInvocationRequest, ToolInvocationResponse)
from spear.proto.transport import (Method, Signal, TransportMessageRaw,
                                   TransportMessageRaw_Data, TransportRequest,
                                   TransportResponse, TransportSignal)

MAX_INFLIGHT_REQUESTS = 128
DEFAULT_MESSAGE_SIZE = 4096

logger = logging.getLogger(__name__)
logger.setLevel(logging.DEBUG)


class Handler(object):
    """
    Handler is the base class for all handlers
    """

    def __init__(self, handle, in_stream: bool = False, out_stream: bool = False):
        if not callable(handle):
            raise ValueError("handle must be a callable")
        if not isinstance(in_stream, bool):
            raise ValueError("in_stream must be a boolean")
        if not isinstance(out_stream, bool):
            raise ValueError("out_stream must be a boolean")
        self._handle = handle
        self._in_stream = in_stream
        self._out_stream = out_stream

    @property
    def in_stream(self) -> bool:
        """
        get the input stream flag
        """
        return self._in_stream

    @property
    def out_stream(self) -> bool:
        """
        get the output stream flag
        """
        return self._out_stream

    def handle(self, *args, **kwargs):
        """
        handle the request
        """
        return self._handle(*args, **kwargs)


class RequestContext(object):
    """
    RequestContext is the context of the request
    """

    def __init__(self, payload=None):
        self._payload = payload

    @property
    def payload(self) -> str:
        """
        get the payload
        """
        return self._payload


class StreamRequestContext(object):
    """
    StreamRequestContext is the context of the stream request
    """

    def __init__(self, data, last_message=False, stream_id: int = None):
        self._data = data
        self._last_message = last_message
        self._stream_id = stream_id

    @property
    def data(self) -> str:
        """
        get the payload
        """
        return self._data

    @property
    def last_message(self) -> bool:
        """
        get the last message flag
        """
        return self._last_message

    def __repr__(self):
        return (f"StreamRequestContext(data={self._data}, " +
                f"last_message={self._last_message}), " +
                f"stream_id={self._stream_id})")

    def __str__(self):
        return self.__repr__()


class HostAgent(object):
    """
    HostAgent is the agent that connects to the host
    """

    _instance = None

    def __init__(self):
        self._send_queue = None
        self._recv_queue = None
        self._global_id = 1
        self._send_task = None
        self._send_task_pipe_r, self._send_task_pipe_w = os.pipe()
        self._recv_task = None
        self._handlers = {}
        self._internal_tools = {}
        event_sock_r, event_sock_w = socket.socketpair()
        self._stop_event_r = event_sock_r
        self._stop_event_w = event_sock_w
        event_sock_r.setblocking(False)
        self._inflight_requests_lock = threading.Lock()
        self._inflight_requests_count = 0
        self._pending_requests = {}
        self._pending_requests_lock = threading.Lock()
        self._client = None
        self._sig_handlers = {}
        self._stream_sequence_ids = {}
        self._stream_sequence_ids_lock = threading.Lock()

    def __new__(cls, *args, **kwargs):
        if cls._instance is None:
            cls._instance = super(HostAgent, cls).__new__(cls)
        return cls._instance

    def connect_host(self, host_addr: str, host_secret: int) -> socket:
        """
        create a tcp connection to the server
        """
        client = socket.socket(socket.AF_INET, socket.SOCK_STREAM)
        self._client = client
        # convert the address to tuple
        host_addr = host_addr.split(":")
        host_addr = (host_addr[0], int(host_addr[1]))
        client.connect(host_addr)
        # send little endian secret 64-bit integer
        client.send(struct.pack("<Q", host_secret))
        self._client.setblocking(False)
        self._send_queue = queue.Queue(512)
        self._recv_queue = queue.Queue(512)
        self._global_id = 0

    def run(self, host_addr=None, host_secret=None):
        """
        start the agent
        """
        if host_addr is None and host_secret is None:
            # get the host address and secret from the environment variables
            # make sure the environment variables are set
            if "SERVICE_ADDR" not in os.environ or "SECRET" not in os.environ:
                raise ValueError("SERVICE_ADDR or SECRET is not set")
            host_addr = os.environ.get("SERVICE_ADDR")
            host_secret = int(os.environ.get("SECRET"))

        logger.info("Connecting to host %s, sec %d", host_addr, host_secret)
        self.connect_host(host_addr, host_secret)

        logger.debug("Starting I/O threads")
        # start the send thread
        send_thread = threading.Thread(target=self._send_thread)
        send_thread.start()
        self._send_task = send_thread

        # start the recv thread
        recv_thread = threading.Thread(target=self._recv_thread)
        recv_thread.start()
        self._recv_task = recv_thread

        self._main_loop()

    def _main_loop(self):
        """
        main loop to handle the rpc calls
        """

        def handle_worker(handler_obj: Handler, req_id: int, ctx: RequestContext):
            with self._inflight_requests_lock:
                self._inflight_requests_count += 1
            try:
                result = handler_obj.handle(ctx)
                if result is None:
                    result = b""
                else:
                    if isinstance(result, str):
                        result = result.encode("utf-8")
                    if not isinstance(result, bytes):
                        raise ValueError(
                            f"Invalid response type: {type(result)}")
                builder = fbs.Builder(1024)
                off = builder.CreateByteVector(result)
                CustomResponse.CustomResponseStart(builder)
                CustomResponse.CustomResponseAddData(builder, off)
                end = CustomResponse.CustomResponseEnd(builder)
                builder.Finish(end)

                self._put_rpc_response(req_id, builder.Output())
            except Exception as e:
                logger.error("Error: %s", traceback.format_exc())
                self._put_rpc_error(req_id, -32603, str(e),
                                    "Internal error: ")
            with self._inflight_requests_lock:
                self._inflight_requests_count -= 1
            logger.debug("Inflight requests: %d",
                         self._inflight_requests_count)

        while True:
            rpc_data = self._get_rpc_data()
            if (
                rpc_data.DataType()
                == TransportMessageRaw_Data.TransportMessageRaw_Data.TransportRequest
            ):
                # handle the request
                req = TransportRequest.TransportRequest()
                req.Init(rpc_data.Data().Bytes, rpc_data.Data().Pos)

                if req.Method() != Method.Method.Custom:
                    if req.Method() == Method.Method.ToolInvoke:
                        tool_invoke = ToolInvocationRequest.ToolInvocationRequest.\
                            GetRootAsToolInvocationRequest(
                                req.RequestAsNumpy())
                        if tool_invoke.ToolInfoType() != ToolInfo.ToolInfo.InternalToolInfo:
                            logger.error("Invalid tool info type: %s",
                                         tool_invoke.ToolInfoType())
                            raise ValueError("Invalid tool info type")
                        tool_tbl = tool_invoke.ToolInfo()
                        if tool_tbl is None:
                            logger.error("Invalid tool info")
                            raise ValueError("Invalid tool info")
                        tool_info = InternalToolInfo.InternalToolInfo()
                        tool_info.Init(tool_tbl.Bytes, tool_tbl.Pos)
                        tool_id = tool_info.ToolId()
                        logger.info("Invoking tool: %d", tool_id)
                        if tool_id not in self._internal_tools:
                            logger.error("tool id does not exist")
                            raise ValueError("tool id does not exist")
                        handler = self._internal_tools[tool_id]

                        def internal_tool_handler(handler, **kwargs):
                            try:
                                result = handler(**kwargs)
                                logger.debug("Result: %s", result)
                                builder = fbs.Builder(1024)
                                res_off = builder.CreateString(result)
                                ToolInvocationResponse.ToolInvocationResponseStart(
                                    builder)
                                ToolInvocationResponse.ToolInvocationResponseAddResult(
                                    builder, res_off)
                                end = ToolInvocationResponse.ToolInvocationResponseEnd(
                                    builder)
                                builder.Finish(end)
                                self._put_rpc_response(
                                    req.Id(), builder.Output())
                            except Exception as e:
                                logger.error(
                                    "Error: %s", traceback.format_exc())
                                self._put_rpc_error(req.Id(), -32603, str(e),
                                                    "Internal error: ")
                        params_dict = {}
                        for i in range(tool_invoke.ParamsLength()):
                            k = tool_invoke.Params(i).Key().decode("utf-8")
                            v = tool_invoke.Params(i).Value().decode("utf-8")
                            logger.info("Param: %s %s", k, v)
                            params_dict[k] = v
                        t = threading.Thread(
                            target=internal_tool_handler,
                            args=(
                                handler,
                            ),
                            kwargs=params_dict
                        )
                        t.daemon = True
                        t.start()
                        continue
                    logger.error("Invalid method: %s", req.Method())
                    raise ValueError("Invalid method")

                custom_req = CustomRequest.CustomRequest.GetRootAsCustomRequest(
                    req.RequestAsNumpy(), 0
                )
                handler_obj = self._handlers.get(
                    custom_req.MethodStr().decode("utf-8"))
                if handler_obj is None:
                    logger.error("Method not found: %s",
                                 custom_req.MethodStr())
                    self._put_rpc_error(
                        req.Id(),
                        -32601,
                        "Method not found",
                        "Method not found",
                    )
                    continue

                if custom_req.RequestInfoType() == RequestInfo.RequestInfo.NormalRequestInfo:
                    if handler_obj.in_stream or handler_obj.out_stream:
                        logger.error("Invalid request type: %s",
                                     custom_req.RequestInfoType())
                        self._put_rpc_error(
                            req.Id(),
                            -32601,
                            "invalid request type",
                            "invalid request type",
                        )
                        continue
                    # handle the normal request
                    normal_req = NormalRequestInfo.NormalRequestInfo()
                    normal_req.Init(custom_req.RequestInfo().Bytes,
                                    custom_req.RequestInfo().Pos)
                    params_str = normal_req.ParamsStr().decode("utf-8")
                    req_ctx = RequestContext(payload=params_str)
                    if self._inflight_requests_count > MAX_INFLIGHT_REQUESTS:
                        self._put_rpc_error(
                            req.Id(),
                            -32000,
                            "Too many requests",
                            "Too many requests",
                        )
                    else:
                        # create a thread to handle the request
                        t = threading.Thread(
                            target=handle_worker,
                            args=(
                                handler_obj,
                                req.Id(),
                                req_ctx,
                            ),
                        )
                        t.daemon = True
                        t.start()
                    continue
                logger.error("invalid request type: %s",
                             custom_req.RequestInfoType())
                self._put_rpc_error(
                    req.Id(),
                    -32601,
                    "invalid request type",
                    "invalid request type",
                )
            elif (
                rpc_data.DataType()
                == TransportMessageRaw_Data.TransportMessageRaw_Data.TransportResponse
            ):
                # handle the response
                # convert from TransportMessageRaw to TransportResponse
                resp = TransportResponse.TransportResponse()
                resp.Init(rpc_data.Data().Bytes, rpc_data.Data().Pos)
                with self._pending_requests_lock:
                    if resp.Id() not in self._pending_requests:
                        logger.error("Invalid response id: %d", resp.Id())
                    else:
                        req = self._pending_requests[resp.Id()]
                        req["cb"](resp)
                        del self._pending_requests[resp.Id()]
            elif (
                rpc_data.DataType()
                == TransportMessageRaw_Data.TransportMessageRaw_Data.TransportSignal
            ):
                sig = TransportSignal.TransportSignal()
                sig.Init(rpc_data.Data().Bytes, rpc_data.Data().Pos)
                if sig.Method() == Signal.Signal.Terminate:
                    logger.info("Terminating the agent")
                    self.stop()
                    return
                if sig.Method() == Signal.Signal.StreamEvent:
                    stream_req = StreamEvent.StreamEvent.GetRootAsStreamEvent(
                        sig.PayloadAsNumpy(), 0
                    )
                    ctx = StreamRequestContext(
                        data=stream_req.DataAsNumpy(),
                        last_message=stream_req.Final(),
                        stream_id=stream_req.StreamId(),
                    )
                    if self._sig_handlers.get(Signal.Signal.StreamEvent):
                        for handler in self._sig_handlers[Signal.Signal.StreamEvent]:
                            resp_data = None
                            try:
                                resp_data = handler(ctx)
                            except Exception as e:
                                logger.error(
                                    "Error: %s", str(e))
                            if resp_data is not None:
                                if isinstance(resp_data, str):
                                    resp_data = resp_data.encode("utf-8")
                                # check if sequence id is set
                                with self._stream_sequence_ids_lock:
                                    if stream_req.StreamId() in self._stream_sequence_ids:
                                        seq_id = self._stream_sequence_ids[
                                            stream_req.StreamId()]
                                        self._stream_sequence_ids[stream_req.StreamId(
                                        )] += 1
                                    else:
                                        seq_id = 0
                                        self._stream_sequence_ids[stream_req.StreamId(
                                        )] = 1
                                self._put_streamevent_signal(
                                    -1, stream_req.ReplyStreamId(),
                                    seq_id,
                                    resp_data,
                                    stream_req.Final(),
                                )
                            else:
                                self._put_streamevent_signal(
                                    -1, stream_req.ReplyStreamId(),
                                    stream_req.SequenceId(),
                                    b"",
                                    True,
                                )
                else:
                    logger.error("Invalid signal method: %s", sig.Method())
                    raise ValueError("Invalid signal method")
            else:
                logger.error("Invalid rpc data")
                raise ValueError("Invalid rpc data")

    def set_internal_tool(self, tid: int, handler):
        """
        register internal tool callback function
        """
        self._internal_tools[tid] = handler

    def register_signal_handler(self, sig_type, handler: Callable):
        """
        register the signal handler for the signal type
        """
        if sig_type not in self._sig_handlers:
            self._sig_handlers[sig_type] = []
        if not isinstance(handler, Callable):
            raise ValueError("handler must be a callable")
        self._sig_handlers[sig_type].append(handler)
        logger.debug("Registered signal handler for %s", sig_type)

    def register_handler(self, method: str, handler: Callable,
                         in_stream: bool = False, out_stream: bool = False):
        """
        register the handler for the method
        """
        if not isinstance(method, str):
            raise ValueError("method must be a string")
        if not isinstance(handler, Callable):
            raise ValueError("handler must be a callable")
        if not isinstance(in_stream, bool):
            raise ValueError("in_stream must be a boolean")
        if not isinstance(out_stream, bool):
            raise ValueError("out_stream must be a boolean")
        if method in self._handlers:
            raise ValueError("method already registered")
        self._handlers[method] = Handler(handler, in_stream, out_stream)

    def unregister_handler(self, method):
        """
        unregister the handler for the method
        """
        del self._handlers[method]

    def _put_raw_object(self, data: bytes):
        """
        finalize the data and add it to the outgoing queue
        """
        self._send_queue.put(data)
        os.write(self._send_task_pipe_w, b"\x01")

    def _get_raw_data(self):
        """
        get the data from the incoming queue
        """
        return self._recv_queue.get()

    def _get_rpc_data(self) -> TransportMessageRaw.TransportMessageRaw:
        trans_resp = (
            TransportMessageRaw.TransportMessageRaw.GetRootAsTransportMessageRaw(
                self._get_raw_data()
            )
        )
        if not isinstance(trans_resp, TransportMessageRaw.TransportMessageRaw):
            raise ValueError("Invalid rpc data")
        return trans_resp

    def exec_request(self, method: int, req_buf: bytes):
        """
        send the rpc request and return the response as numpy array
        """
        # create mutex
        mutex = threading.Lock()
        # create a condition variable
        cond = threading.Condition(mutex)
        # create a list to store the response
        response = []

        def cb(rpc_data: TransportResponse.TransportResponse):
            with mutex:
                response.append(rpc_data)
                cond.notify()

        self._put_rpc_request(method, req_buf, cb)
        with mutex:
            cond.wait()
            resp = response[0]
            if resp.Code() != 0:
                raise RuntimeError(resp.Message())
            return resp.ResponseAsNumpy()

    def _put_streamevent_signal(self, stream_id: int, reply_stream_id: int,
                                seq_id: int, data: bytes, last_message: bool):
        """
        send the rpc signal
        """
        builder = fbs.Builder(len(data) + 1024)
        data_off = builder.CreateByteVector(data)

        StreamEvent.StreamEventStart(builder)
        StreamEvent.AddStreamId(builder, stream_id)
        StreamEvent.AddReplyStreamId(builder, reply_stream_id)
        StreamEvent.AddSequenceId(builder, seq_id)
        StreamEvent.AddData(builder, data_off)
        StreamEvent.AddFinal(builder, last_message)
        req_off = StreamEvent.End(builder)
        builder.Finish(req_off)

        stream_event_data = builder.Output()

        builder = fbs.Builder(len(stream_event_data) + 1024)
        req_off = builder.CreateByteVector(stream_event_data)

        TransportSignal.TransportSignalStart(builder)
        TransportSignal.AddMethod(
            builder, Signal.Signal.StreamEvent
        )
        TransportSignal.AddPayload(builder, req_off)
        req_off = TransportSignal.End(builder)

        TransportMessageRaw.TransportMessageRawStart(builder)
        TransportMessageRaw.AddDataType(
            builder, TransportMessageRaw_Data.TransportMessageRaw_Data.TransportSignal
        )
        TransportMessageRaw.AddData(builder, req_off)
        msg_off = TransportMessageRaw.End(builder)
        builder.Finish(msg_off)

        self._put_raw_object(builder.Output())

    def _put_signal(self, method: int, req_buf: bytes):
        """
        send the rpc signal
        """
        builder = fbs.Builder(len(req_buf) + 1024)
        req_buf_off = builder.CreateByteVector(req_buf)

        TransportSignal.TransportSignalStart(builder)
        TransportSignal.AddMethod(builder, method)
        TransportSignal.AddPayload(builder, req_buf_off)
        req_off = TransportSignal.End(builder)

        TransportMessageRaw.TransportMessageRawStart(builder)
        TransportMessageRaw.AddDataType(
            builder, TransportMessageRaw_Data.TransportMessageRaw_Data.TransportSignal
        )
        TransportMessageRaw.AddData(builder, req_off)
        msg_off = TransportMessageRaw.End(builder)
        builder.Finish(msg_off)

        self._put_raw_object(builder.Output())

    def _put_rpc_request(
        self,
        method: int,
        req_buf: bytes,
        cb: Callable[[TransportResponse.TransportResponse], None],
    ):
        new_id = self._global_id
        self._global_id += 1
        builder = fbs.Builder(len(req_buf) + 1024)
        req_buf_off = builder.CreateByteVector(req_buf)

        TransportRequest.Start(builder)
        TransportRequest.AddId(builder, new_id)
        TransportRequest.AddMethod(builder, method)
        TransportRequest.AddRequest(builder, req_buf_off)
        req_off = TransportRequest.End(builder)

        TransportMessageRaw.TransportMessageRawStart(builder)
        TransportMessageRaw.AddDataType(
            builder, TransportMessageRaw_Data.TransportMessageRaw_Data.TransportRequest
        )
        TransportMessageRaw.AddData(builder, req_off)
        msg_off = TransportMessageRaw.End(builder)
        builder.Finish(msg_off)

        data = builder.Output()
        with self._pending_requests_lock:
            self._pending_requests[new_id] = {
                "time": time.time(),
                "obj": data,
                "cb": cb,
            }
        self._put_raw_object(data)

    def _put_rpc_response(self, req_id: int, result: bytes):
        if result is None:
            sz = 0
        else:
            sz = len(result)
        builder = fbs.Builder(sz + 512)
        if result is not None:
            result_off = builder.CreateByteVector(result)

        if req_id < 0:
            raise ValueError("Invalid request id")

        TransportResponse.TransportResponseStart(builder)
        TransportResponse.AddId(builder, req_id)
        if result is not None:
            TransportResponse.AddResponse(builder, result_off)
        end = TransportResponse.End(builder)

        TransportMessageRaw.TransportMessageRawStart(builder)
        TransportMessageRaw.AddDataType(
            builder, TransportMessageRaw_Data.TransportMessageRaw_Data.TransportResponse
        )
        TransportMessageRaw.AddData(builder, end)
        end2 = TransportMessageRaw.End(builder)
        builder.Finish(end2)
        self._put_raw_object(builder.Output())

    def _put_rpc_error(self, req_id: int, code: int, message, data=None):
        builder = fbs.Builder(512 + len(message) + len(data))
        message_off = builder.CreateString(message)
        if data is not None:
            data_off = builder.CreateString(data)
        else:
            data_off = 0

        if req_id < 0:
            raise ValueError("Invalid request id")

        TransportResponse.TransportResponseStart(builder)
        TransportResponse.AddId(builder, req_id)
        TransportResponse.AddCode(builder, code)
        TransportResponse.AddMessage(builder, message_off)
        if data_off != 0:
            TransportResponse.AddResponse(builder, data_off)
        end = TransportResponse.End(builder)

        TransportMessageRaw.TransportMessageRawStart(builder)
        TransportMessageRaw.AddDataType(
            builder, TransportMessageRaw_Data.TransportMessageRaw_Data.TransportResponse
        )
        TransportMessageRaw.AddData(builder, end)
        end2 = TransportMessageRaw.End(builder)
        builder.Finish(end2)
        self._put_raw_object(builder.Output())

    def _send_thread(self):
        """
        send the data to the socket
        """

        def send_remaining_data():
            while not self._send_queue.empty():
                data = self._send_queue.get()
                # data = strdata.encode("utf-8")
                length = len(data)
                lendata = length.to_bytes(8, byteorder="little")
                self._client.sendall(lendata)
                self._client.sendall(data)
            # send a data with length 0
            lendata = (0).to_bytes(8, byteorder="little")
            self._client.sendall(lendata)

        def send_data():
            # clear the pipe
            os.read(self._send_task_pipe_r, 1)
            data = self._send_queue.get()
            # data = strdata.encode("utf-8")
            # get the length of utf8 string
            length = len(data)
            lendata = length.to_bytes(8, byteorder="little")
            # send the length of the data
            self._client.sendall(lendata)
            self._client.sendall(data)

        sel = selectors.DefaultSelector()
        sel.register(self._stop_event_r, selectors.EVENT_READ)
        sel.register(self._send_task_pipe_r, selectors.EVENT_READ)
        while True:
            events = sel.select()
            for key, _ in events:
                if key.fileobj == self._stop_event_r:
                    # send remaining data
                    send_remaining_data()
                    return
                if key.fileobj == self._send_task_pipe_r:
                    send_data()

    def _recv_thread(self):
        """
        get the data from socket and parse it
        """

        def recv_data() -> bool:
            # read int64 from the socket and convert to integer
            data = self._client.recv(8)
            if len(data) == 0:
                return False
            length = int.from_bytes(data, byteorder="little")
            # read the data
            data = b""
            while len(data) < length:
                try:
                    tmp = self._client.recv(length - len(data))
                    if len(tmp) == 0:
                        return False
                    data += tmp
                except BlockingIOError as e:
                    if e.errno == 11:
                        continue
            self._recv_queue.put(data)
            return True

        sel = selectors.DefaultSelector()
        sel.register(self._client, selectors.EVENT_READ)
        sel.register(self._stop_event_r, selectors.EVENT_READ)
        while True:
            events = sel.select()
            for key, _ in events:
                if key.fileobj == self._stop_event_r:
                    return
                if key.fileobj == self._client:
                    if not recv_data():
                        logger.info("Connection closed")
                        return

    def stop(self):
        """
        stop the agent
        """

        def stop_worker():
            # wait until all the inflight requests are completed
            while True:
                with self._inflight_requests_lock:
                    if self._inflight_requests_count == 0:
                        break
            self._stop_event_w.send(b"\x01")
            self._send_task.join()
            self._recv_task.join()
            logger.debug("Stopping the agent")
            self._client.close()
            os._exit(0)

        # create a thread to stop the agent
        threading.Thread(target=stop_worker).start()
