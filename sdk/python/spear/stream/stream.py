#!/usr/bin/env python3
import logging
import sys

import flatbuffers as fbs
import spear.client as client

from spear.proto.stream import OperationType

logging.basicConfig(
    level=logging.DEBUG,  # Set the desired logging level
    # Customize the log format
    format="%(asctime)s - %(name)s - %(levelname)s - %(message)s",
    handlers=[logging.StreamHandler(stream=sys.stderr)],  # Log to stderr
)

logger = logging.getLogger(__name__)
logger.setLevel(logging.INFO)


def stream_sendoperation(agent: client.HostAgent, stream_id: int, resource: str,
                         op: OperationType, data: bytes, final: bool = False) -> None:
    """
    Send a stream operation
    """
    logger.info("Sending stream operation: %d", op)

    agent.send_operation_event(stream_id, resource, op, data, final)
