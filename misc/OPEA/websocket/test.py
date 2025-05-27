#!/usr/bin/env python3
import logging
import sys

import spear.client as client

logging.basicConfig(
    level=logging.DEBUG,  # Set the desired logging level
    # Customize the log format
    format="%(asctime)s - %(name)s - %(levelname)s - %(message)s",
    handlers=[logging.StreamHandler(stream=sys.stderr)],  # Log to stderr
)

logger = logging.getLogger(__name__)
logger.setLevel(logging.DEBUG)

# global counter
counter = 0


@client.handle_stream
def handle_stream(ctx):
    """
    handle the request
    if nothing is returned or exception is raised, the stream will be closed
    """
    global counter
    counter += 1
    logger.info("Handling request: %s", ctx)
    if counter > 5:
        return
    ctx.send_raw(f"I got your message {ctx}")
    return


if __name__ == "__main__":
    client.init()
