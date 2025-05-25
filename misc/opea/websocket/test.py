#!/usr/bin/env python3
import logging
import sys

import spear.client as client
import spear.proto.transport.Signal as Signal

logging.basicConfig(
    level=logging.DEBUG,  # Set the desired logging level
    # Customize the log format
    format="%(asctime)s - %(name)s - %(levelname)s - %(message)s",
    handlers=[logging.StreamHandler(stream=sys.stderr)],  # Log to stderr
)

logger = logging.getLogger(__name__)
logger.setLevel(logging.DEBUG)

agent = client.HostAgent()

# global counter
counter = 0


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
    return f'I got your message"{ctx}"'


if __name__ == "__main__":
    agent.register_signal_handler(
        Signal.Signal.StreamData,
        handle_stream,
    )
    agent.run()
