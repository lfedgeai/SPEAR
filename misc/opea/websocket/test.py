#!/usr/bin/env python3
import logging
import sys
import time

import spear.client as client
import spear.transform.chat as chat
import spear.utils.io as io
from spear.utils.tool import register_internal_tool

from spear.proto.tool import BuiltinToolID

logging.basicConfig(
    level=logging.DEBUG,  # Set the desired logging level
    # Customize the log format
    format="%(asctime)s - %(name)s - %(levelname)s - %(message)s",
    handlers=[logging.StreamHandler(stream=sys.stderr)],  # Log to stderr
)

logger = logging.getLogger(__name__)
logger.setLevel(logging.DEBUG)

agent = client.HostAgent()


def handle(ctx):
    """
    handle the request
    """
    logger.info("Handling request: %s", ctx)
    logger.info("Instream ID: %s", ctx.istream)
    logger.info("Outstream ID: %s", ctx.ostream)


if __name__ == "__main__":
    agent.register_handler("handle", handle)
    agent.run()
