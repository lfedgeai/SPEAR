#!/usr/bin/env python3
import argparse
import base64
import json
import logging
import sys

import spear.client as client
import spear.hostcalls.tools as tools
import spear.hostcalls.transform as tf
import spear.transform.chat as chat
import spear.transform.speech as speech
import spear.utils.io as io

from spear.proto.tool import BuiltinToolID

LLM_MODEL = "gpt-4o"  # "llama-toolchat" # "llama-toolchat-70b" # "qwen-toolchat-72b"
STT_MODEL = "gaia-whisper"
TTS_MODEL = "tts-1"

SPEAK_MESSAGE = True


logging.basicConfig(
    level=logging.DEBUG,  # Set the desired logging level
    # Customize the log format
    format="%(asctime)s - %(name)s - %(levelname)s - %(message)s",
    handlers=[logging.StreamHandler(stream=sys.stderr)],  # Log to stderr
)

logger = logging.getLogger(__name__)
logger.setLevel(logging.INFO)

agent = client.HostAgent()


def handle(ctx):
    """
    handle the request
    """
    logger.debug("Handling request: %s", ctx.payload)

    msg_memory = [("system",
                   ("You will be provided with a set of tools you could potentially use. " +
                    "But do not make tool calls unless it is necessary for you to answer " +
                    "the user question. "))]
    while True:
        user_input = io.input(agent, "(? for help) > ")

        # trim the user input, remove space and newline
        user_input = user_input.strip().decode('utf-8')
        if not user_input:
            continue
        if user_input == "q":
            print("Quitting", flush=True)
            break
        if user_input == "?":
            help_msg = """q: quit
r: record voice input"""
            print(help_msg, flush=True)
            continue
        if user_input == "r":
            user_input = io.record(agent, "Assistant is listening")
            if user_input:
                print(f"User: {user_input}", flush=True)
            else:
                print("Failed to convert audio to text", flush=True)
                continue

        msg_memory.append(
            ("user", user_input)
        )

        resp = chat.chat(agent, msg_memory, model=LLM_MODEL,
                         builtin_tools=[
                             BuiltinToolID.BuiltinToolID.Datetime,
                             BuiltinToolID.BuiltinToolID.FullScreenshot,
                             BuiltinToolID.BuiltinToolID.MouseRightClick,
                             BuiltinToolID.BuiltinToolID.SearchContactEmail,
                             BuiltinToolID.BuiltinToolID.SendEmailDraftWindow,
                             BuiltinToolID.BuiltinToolID.ComposeEmail,
                             BuiltinToolID.BuiltinToolID.ListOpenEmails,
                             BuiltinToolID.BuiltinToolID.OpenURL,
                         ])

        tmp_msgs = resp[len(msg_memory):]
        for e in tmp_msgs:
            role = e[0]
            msg = e[1]
            print(f"{role}: {msg}", flush=True)
            # and not msg.metadata.tool_calls
            if role == "assistant" and len(msg) > 0:
                if SPEAK_MESSAGE:
                    io.speak(agent, msg)

        msg_memory = [(e[0], e[1]) if e[0] != "tool" else ("user", e[1])
                      for e in resp]

    agent.stop()
    return "done"


if __name__ == "__main__":
    agent.register_handler("handle", handle)
    agent.run()
