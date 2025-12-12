#!/usr/bin/env python3
import logging
import sys
import threading
from typing import List

import numpy as np
import sail.proto as sailproto
import spear.client as client
from spear.proto.stream.OperationType import OperationType
from spear.stream import AbstractStreamHandler

from spear.proto.stream import NotificationEventType

logging.basicConfig(
    level=logging.DEBUG,  # Set the desired logging level
    # Customize the log format
    format="%(asctime)s - %(name)s - %(levelname)s - %(message)s",
    handlers=[logging.StreamHandler(stream=sys.stderr)],  # Log to stderr
)

logger = logging.getLogger(__name__)
logger.setLevel(logging.DEBUG)


class AtomicCounter:
    """A thread-safe counter that allows atomic increment operations."""

    def __init__(self, initial_value=0):
        self._value = initial_value
        self._lock = threading.Lock()

    def get_and_increment(self):
        """
        Atomically get the current value and increment it by 1.
        Returns:
            int: The current value before incrementing.
        """
        with self._lock:
            current_value = self._value
            self._value += 1
            return current_value


handler = sailproto.SAILProtocolHandler()
g_sequence = AtomicCounter(1)  # Start sequence from 1
g_stream_handler = None

g_ctx = None


def create_empty_server_response(sequence):
    """Create an empty server response with the given sequence number."""
    response_data = {
        "audio_info": {"duration": 3696},
        "result": {},
    }
    res = handler.create_full_response(sequence, response_data)
    return res


def create_delta_response(sequence, delta_list: List[str], full_test: str):
    """Create a server response with delta text."""
    response_data = {
        "audio_info": {"duration": 3696},
        "result": {
            "text": full_test,
            "confidence": 95,
            "utterances": [
                {
                    "definite": True,
                    "end_time": 3696,
                    "start_time": 0,
                    "text": i,
                    "words": [
                        {
                            "blank_duration": 0,
                            "end_time": 3696,
                            "start_time": 0,
                            "text": i,
                        }
                    ],
                }
                for i in delta_list
            ],
        },
    }
    res = handler.create_full_response(sequence, response_data)
    return res


@client.handle_stream
def handle_stream(ctx: client.RawStreamRequestContext):
    """
    handle the request
    """
    global g_stream_handler
    global g_ctx

    g_ctx = ctx

    final = False
    # logger.info("Handling request: %s", ctx)
    if ctx.last_message and len(ctx.data) == 0:
        logger.info("got last message")
        final = True
    if final:
        if g_stream_handler is not None:
            logger.info(
                "Closing real-time ASR stream with ID: %d", g_stream_handler.stream_id
            )
            g_stream_handler.close_stream()
            g_stream_handler = None
        g_ctx.send_raw(
            handler.serialize_message(
                create_delta_response(
                    g_sequence.get_and_increment(),
                    [],
                    "",
                )
            ),
            final=True,
        )
        client.stop()
        return

    # ctx.send_raw(f"Got msg: {ctx.data}")
    req = handler.parse_message(ctx.data)
    if isinstance(req, sailproto.FullClientRequest):
        # logger.debug("Client->Agent Received FullClientRequest: %s", req)
        if g_stream_handler is None:
            g_stream_handler = RealtimeASRStreamHandler()
            g_stream_handler.open_stream("rt-asr")
            logger.info(
                "Creating real-time ASR stream with ID: %d", g_stream_handler.stream_id
            )

        g_stream_handler.send_operation_event(OperationType.Create, "")
        logger.info("Real-time ASR stream created successfully.")

        # need to send a response to the client
        resp = create_empty_server_response(g_sequence.get_and_increment())
        ctx.send_raw(handler.serialize_message(resp), final=final)
    elif isinstance(req, sailproto.AudioOnlyRequest):
        # logger.debug("Client->Agent Received AudioOnlyRequest: %s", req)
        g_stream_handler.send_operation_event(OperationType.Append, req.audio_data)
    else:
        logger.error("Unknown request type: %s", type(req))
        ctx.send_raw("", final=final)


class RealtimeASRStreamHandler(AbstractStreamHandler):
    """
    Handler for real-time ASR streams.
    This class can be extended to handle specific ASR stream logic.
    """

    def operation(self, ctx: client.OperationStreamRequestContext):
        """Handle operations from the ASR stream."""
        logger.debug("Received operation: %s", ctx)

    def raw(self, ctx: client.RawStreamRequestContext):
        """Handle raw data from the ASR stream."""
        logger.debug("Received raw data: %s", ctx.data)

    def notification(self, ctx: client.NotificationStreamRequestContext):
        """
        Handle notifications from the ASR stream.
        """
        deltas = []
        if ctx.notification_type == NotificationEventType.NotificationEventType.Created:
            logger.info("Backend->Agent Stream created: %s", ctx)
        elif ctx.notification_type == NotificationEventType.NotificationEventType.Error:
            logger.error("Backend->Agent Error notification received: %s", ctx)
        elif (
            ctx.notification_type
            == NotificationEventType.NotificationEventType.Configured
        ):
            logger.debug("Backend->Agent Stream configured: %s", ctx)
        elif (
            ctx.notification_type == NotificationEventType.NotificationEventType.Updated
        ):
            # logger.debug("Backend->Agent Stream updated: %s", ctx)
            if ctx.name == "rt-asr.delta":
                data = ctx.data
                if isinstance(data, np.ndarray):
                    data = data.tobytes()
                if data:
                    deltas.append(data.decode("utf-8"))
            elif ctx.name == "rt-asr.completed":
                # convert numpy array to bytes
                data = ctx.data
                if isinstance(data, np.ndarray):
                    data = data.tobytes()
                g_ctx.send_raw(
                    handler.serialize_message(
                        create_delta_response(
                            g_sequence.get_and_increment(), deltas, data.decode("utf-8")
                        )
                    ),
                    final=False,
                )
                deltas = []
            elif ctx.name == "rt-asr.stopped":
                pass
            elif ctx.name == "rt-asr.appended":
                pass
        elif (
            ctx.notification_type
            == NotificationEventType.NotificationEventType.Completed
        ):
            logger.info("Backend->Agent Stream completed: %s", ctx)


def main():
    """Main function to initialize the client."""
    client.init()
    client.wait()


if __name__ == "__main__":
    main()
