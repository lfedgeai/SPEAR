#!/usr/bin/env python3
import logging
import sys

import sail.proto as sailproto
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

handler = sailproto.SAILProtocolHandler()


@client.handle_stream
def handle_stream(ctx):
    """
    handle the request
    """
    logger.info("Handling request: %s", ctx)
    # ctx.send_raw(f"Got msg: {ctx.data}")
    req = handler.parse_message(ctx.data)
    if isinstance(req, sailproto.FullClientRequest):
        pass
    elif isinstance(req, sailproto.AudioOnlyRequest):
        pass
    else:
        logger.error("Unknown request type: %s", type(req))
    return


class RealtimeASRStreamHandler:
    """
    Handler for real-time ASR streams.
    This class can be extended to handle specific ASR stream logic.
    """

    def handle_rt_asr(self, ctx):
        """
        Handle real-time ASR stream requests.
        """
        logger.info("Handling real-time ASR request: %s", ctx)
        if ctx.is_opeartion_event:
            self.operation(ctx)
        elif ctx.is_notification_event:
            self.notification(ctx)
        elif ctx.is_raw:
            self.raw()

    def notification(self, ctx):
        """
        Handle notifications from the ASR stream.
        """
        logger.info("Received notification: %s", ctx)

    def operation(self, ctx):
        """
        Handle operation events from the ASR stream.
        """
        logger.info("Received operation event: %s", ctx)

    def raw(self):
        """
        Handle raw data from the ASR stream.
        This method can be extended to process raw audio data.
        """
        logger.info("Received raw data from ASR stream.")


def main():
    """Main function to initialize the client."""
    asr_instance = RealtimeASRStreamHandler()
    client.init()
    rt_asr_id = create_stream("rt-asr", asr_instance.handle_rt_asr)
    client.wait()
    close_stream(rt_asr_id)


if __name__ == "__main__":
    main()
