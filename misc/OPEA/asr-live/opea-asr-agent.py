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
global_sequence = None

def create_server_response(sequence):
    response_data = {
        "audio_info": {"duration": 3696},
        "result": {
            "text": "这是字节跳动，今日头条母公司。",
            "confidence": 95,
            "utterances": [
                {
                    "definite": True,
                    "end_time": 1705,
                    "start_time": 0,
                    "text": "这是字节跳动，",
                    "words": [
                        {
                            "blank_duration": 0,
                            "end_time": 860,
                            "start_time": 740,
                            "text": "这",
                        },
                        {
                            "blank_duration": 0,
                            "end_time": 1020,
                            "start_time": 860,
                            "text": "是",
                        },
                    ],
                },
                {
                    "definite": True,
                    "end_time": 3696,
                    "start_time": 2110,
                    "text": "今日头条母公司。",
                    "words": [
                        {
                            "blank_duration": 0,
                            "end_time": 3070,
                            "start_time": 2910,
                            "text": "今",
                        },
                        {
                            "blank_duration": 0,
                            "end_time": 3230,
                            "start_time": 3070,
                            "text": "日",
                        },
                    ],
                },
            ],
        },
    }
    res = handler.create_full_response(
        sequence,
        response_data
    )
    return res

@client.handle_stream
def handle_stream(ctx: client.RawStreamRequestContext):
    """
    handle the request
    """
    global global_sequence
    if global_sequence is None:
        global_sequence = 1
    # logger.info("Handling request: %s", ctx)
    if ctx.last_message and len(ctx.data) == 0:
        logger.info("got last message")
        return
    # ctx.send_raw(f"Got msg: {ctx.data}")
    req = handler.parse_message(ctx.data)
    if isinstance(req, sailproto.FullClientRequest):
        logger.info("Received FullClientRequest: %s", req)
        resp = create_server_response(global_sequence)
        global_sequence += 1
        ctx.send_raw(handler.serialize_message(resp))
    elif isinstance(req, sailproto.AudioOnlyRequest):
        logger.info("Received AudioOnlyRequest: %s", req)
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
