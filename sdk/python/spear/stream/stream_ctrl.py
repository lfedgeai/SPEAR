#!/usr/bin/env python3
import logging
import sys
import uuid
from typing import Callable

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


def create_stream(class_name: str, handler: Callable) -> int:
    """
    Create a stream
    """
    logger.info("Creating stream")
    agent = client.global_agent()

    # Generate a unique 32-bit signed request ID
    req_id = uuid.uuid4().int & 0x7FFFFFFF  # Ensure it's within int32 positive range
    builder = fbs.Builder(0)
    str_off = builder.CreateString(class_name)

    StreamControlRequest.StreamControlRequestStart(builder)
    StreamControlRequest.StreamControlRequestAddRequestId(builder, req_id)
    StreamControlRequest.StreamControlRequestAddClassName(builder, str_off)
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
    client.register_stream_handler(resp.StreamId(), handler)
    return resp.StreamId()


def close_stream(stream_id: int) -> None:
    """
    Close a stream
    """
    logger.info("Closing stream with ID: %d", stream_id)
    agent = client.global_agent()

    # Generate a unique 32-bit signed request ID
    req_id = uuid.uuid4().int & 0x7FFFFFFF  # Ensure it's within int32 positive range
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
    client.unregister_stream_handler(stream_id)
