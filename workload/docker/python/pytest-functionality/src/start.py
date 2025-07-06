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


TEST_LLM_MODEL = "gpt-4o"  # "deepseek-toolchat"


def handle(ctx):
    """
    handle the request
    """
    logger.info("Handling request: %s", ctx.payload)

    logger.info("testing tool")
    test_tool(TEST_LLM_MODEL)

    logger.info("testing chat")
    test_chat(TEST_LLM_MODEL)

    logger.info("testing speak")
    test_speak("tts-1")

    logger.info("testing record")
    test_record("whisper-1")

    logger.info("testing input")
    test_input()

    # test("text-embedding-ada-002")
    # test("bge-large-en-v1.5")

    time.sleep(10)
    # agent.stop()


def test_chat(model):
    """
    test the model
    """
    logger.info("Testing model: %s", model)

    resp = chat.chat("hi", model=model, agent=agent)
    logger.info(resp)
    resp = chat.chat(
        "what is the time now?",
        model=model,
        builtin_tools=[
            BuiltinToolID.BuiltinToolID.Datetime,
        ],
        agent=agent,
    )
    logger.info(resp)


def test_speak(model):
    """
    test the model
    """
    logger.info("Testing model: %s", model)

    resp = io.speak("test test test", dryrun=True, agent=agent)
    assert resp is not None


def test_record(model):
    """
    test the model
    """
    logger.info("Testing model: %s", model)

    resp = io.record("recording test", dryrun=True, agent=agent)
    assert resp is not None


def test_input():
    """
    test the model
    """
    logger.info("Testing input")

    resp = io.input("input", True, agent=agent)
    logger.info(resp)


def test_tool_cb(param1, param2):
    """
    spear tool function for getting the sum of two numbers

    @param param1: first number
    @param param2: second number
    """
    logger.info("Testing tool callback %s %s", param1, param2)
    # parse params as int
    return str(int(param1) + int(param2))


def test_tool(model):
    """
    test the model
    """
    logger.info("Testing tool")
    tid = register_internal_tool(test_tool_cb, agent=agent)
    logger.info("Registered tool: %d", tid)

    resp = chat.chat("hi", model=model, agent=agent)
    logger.info(resp)
    resp = chat.chat(
        ["hi", "what is sum of 123 and 456?"],
        model=model,
        builtin_tools=[
            BuiltinToolID.BuiltinToolID.Datetime,
        ],
        internal_tools=[
            tid,
        ],
        agent=agent,
    )
    logger.info(resp)
    resp = chat.chat(
        [("system", "you are a calculator"), ("user", "what is sum of 123 and 456?")],
        model=model,
        builtin_tools=[
            BuiltinToolID.BuiltinToolID.Datetime,
        ],
        internal_tools=[
            tid,
        ],
        agent=agent,
    )
    logger.info(resp)


if __name__ == "__main__":
    agent.register_handler("handle", handle)
    agent.loop()
