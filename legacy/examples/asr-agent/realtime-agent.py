#!/usr/bin/env python3
import json
import logging
import sys

import numpy as np
import spear.client as client
import spear.transform.chat as chat
from spear.proto.stream.OperationType import OperationType
from spear.stream import AbstractStreamHandler

from spear.proto.stream import NotificationEventType
from spear.proto.tool import BuiltinToolID

logging.basicConfig(
    level=logging.DEBUG,  # Set the desired logging level
    # Customize the log format
    format="%(asctime)s - %(name)s - %(levelname)s - %(message)s",
    handlers=[logging.StreamHandler(stream=sys.stderr)],  # Log to stderr
)

logger = logging.getLogger(__name__)
logger.setLevel(logging.DEBUG)

LLM_MODEL = "gpt-4o"

g_ctx = None
g_stream_handler = None

msg_memory = []


def chat_msg(msg: str):
    """Send a chat message."""
    global msg_memory
    old_len = len(msg_memory)
    msg_memory.append(("user", msg))

    try:
        rtn = chat.chat(
            msg_memory,
            model=LLM_MODEL,
            builtin_tools=[
                BuiltinToolID.BuiltinToolID.Datetime,
                BuiltinToolID.BuiltinToolID.FullScreenshot,
                BuiltinToolID.BuiltinToolID.MouseRightClick,
                BuiltinToolID.BuiltinToolID.SearchContactEmail,
                BuiltinToolID.BuiltinToolID.SendEmailDraftWindow,
                BuiltinToolID.BuiltinToolID.ComposeEmail,
                BuiltinToolID.BuiltinToolID.ListOpenEmails,
                BuiltinToolID.BuiltinToolID.OpenURL,
            ],
        )
    except RuntimeError as e:
        logger.error("Error in chat: %s", e)
        return None
    msg_memory = [(e[0], e[1]) if e[0] != "tool" else ("user", e[1]) for e in rtn]

    resp = rtn[old_len:]
    return resp


@client.handle_stream
def handle_stream(ctx: client.RawStreamRequestContext):
    """
    handle the request
    """
    global g_ctx
    global g_stream_handler

    final = False
    g_ctx = ctx
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
        g_ctx.send_raw("", final=True)
        client.stop()
        return
    else:
        if g_stream_handler is None:
            g_stream_handler = RealtimeASRStreamHandler()
            g_stream_handler.open_stream("rt-asr")
            logger.info(
                "Creating real-time ASR stream with ID: %d", g_stream_handler.stream_id
            )
            g_stream_handler.send_operation_event(OperationType.Create, "")
            logger.info("Real-time ASR stream created successfully.")
    g_stream_handler.send_operation_event(OperationType.Append, ctx.data)


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
                resp = chat_msg(data.decode("utf-8"))
                if resp is None:
                    return
                g_ctx.send_raw(
                    json.dumps(resp),
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
