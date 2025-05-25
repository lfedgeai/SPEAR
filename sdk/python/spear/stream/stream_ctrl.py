#!/usr/bin/env python3
import logging
import sys
import uuid

import flatbuffers as fbs
import spear.client as client

from spear.proto.stream import (StreamControlOps, StreamControlRequest,
                                StreamControlResponse)
from spear.proto.transport import Method

logging.basicConfig(
    level=logging.DEBUG,  # Set the desired logging level
    # Customize the log format
    format="%(asctime)s - %(name)s - %(levelname)s - %(message)s",
    handlers=[logging.StreamHandler(stream=sys.stderr)],  # Log to stderr
)

logger = logging.getLogger(__name__)
logger.setLevel(logging.INFO)


def create_stream(agent: client.HostAgent, class_name: str) -> int:
    """
    Create a stream
    """
    logger.info("Creating stream")

    req_id = uuid.uuid4().int & (1 << 32) - 1  # Generate a unique 32-bit request ID
    builder = fbs.Builder(0)
    strOff = builder.CreateString(class_name)

    StreamControlRequest.StreamControlRequestStart(builder)
    StreamControlRequest.StreamControlRequestAddRequestId(builder, req_id)
    StreamControlRequest.StreamControlRequestAddClassName(builder, strOff)
    StreamControlRequest.StreamControlRequestAddOp(
        builder, StreamControlOps.StreamControlOps.New
    )
    end_off = StreamControlRequest.StreamControlRequestEnd(builder)
    builder.Finish(end_off)

    data = agent.exec_request(Method.Method.StreamCtrl, builder.Output())
    resp = StreamControlResponse.StreamControlResponse.GetRootAs(data, 0)
    if resp.RequestId() != req_id:
        raise ValueError(
            f"Request ID mismatch: expected {req_id}, got {resp.RequestId()}"
        )
    if resp.StreamId() <= 0:
        raise ValueError(f"Invalid stream ID: {resp.StreamId()}")
    logger.info("Stream created with ID: %d", resp.StreamId())
    return resp.StreamId()


def close_stream(agent: client.HostAgent, stream_id: int) -> None:
    """
    Close a stream
    """
    logger.info("Closing stream with ID: %d", stream_id)

    req_id = 1234  # temporary value
    builder = fbs.Builder(0)
    StreamControlRequest.StreamControlRequestStart(builder)
    StreamControlRequest.StreamControlRequestAddRequestId(builder, req_id)
    StreamControlRequest.StreamControlRequestAddStreamId(builder, stream_id)
    StreamControlRequest.StreamControlRequestAddOp(
        builder, StreamControlOps.StreamControlOps.Close
    )
    end_off = StreamControlRequest.StreamControlRequestEnd(builder)
    builder.Finish(end_off)

    data = agent.exec_request(Method.Method.StreamCtrl, builder.Output())
    resp = StreamControlResponse.StreamControlResponse.GetRootAs(data, 0)
    if resp.StreamId() != stream_id:
        raise ValueError(
            f"Request ID mismatch: expected {stream_id}, got {resp.StreamId()}"
        )
    if resp.RequestId() != req_id:
        raise ValueError(f"Invalid request ID: {resp.RequestId()}")
    logger.info("Stream closed with ID: %d", resp.StreamId())
