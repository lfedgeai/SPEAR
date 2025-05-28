#!/usr/bin/env python3
import logging
import sys

import spear.client as client
from spear.stream import close_stream, create_stream

logging.basicConfig(
    level=logging.DEBUG,  # Set the desired logging level
    # Customize the log format
    format="%(asctime)s - %(name)s - %(levelname)s - %(message)s",
    handlers=[logging.StreamHandler(stream=sys.stderr)],  # Log to stderr
)

logger = logging.getLogger(__name__)
logger.setLevel(logging.DEBUG)


@client.handle_stream
def handle_stream(ctx):
    """
    handle the request
    if nothing is returned or exception is raised, the stream will be closed
    """
    logger.info("Handling request: %s", ctx)
    ctx.send_raw(f"Got msg: {ctx.payload}")
    return


def handle_rt_asr(ctx):
    """
    Handle real-time ASR stream requests.
    """
    logger.info("Handling real-time ASR request: %s", ctx)


def main():
    """Main function to initialize the client."""
    client.init()

    rt_asr_id = create_stream("rt-asr", handle_rt_asr)

    close_stream(rt_asr_id)


if __name__ == "__main__":
    main()
