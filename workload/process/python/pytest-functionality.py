#!/usr/bin/env python3
import logging
import sys
import time

import spear.client as client
import spear.transform.chat as chat
import spear.utils.io as io
from spear.stream import (create_stream, close_stream,
                          stream_sendoperation, OperationType)
from spear.utils.tool import register_internal_tool

from spear.proto.tool import BuiltinToolID
from spear.proto.transport import Signal

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

    test_stream_data()

    time.sleep(10)
    # agent.stop()


def handle_stream(data):
    """
    handle streaming request
    """
    logger.info("Handling streaming request: %s", data)

    # test("text-embedding-ada-002")
    # test("bge-large-en-v1.5")

    return f"#Hi I got the context: {data}#"


def test_chat(model):
    """
    test the model
    """
    logger.info("Testing model: %s", model)

    resp = chat.chat(agent, "hi", model=model)
    logger.info(resp)
    resp = chat.chat(agent, "what is the time now?",
                     model=model, builtin_tools=[
                         BuiltinToolID.BuiltinToolID.Datetime,
                     ])
    logger.info(resp)


def test_speak(model):
    """
    test the model
    """
    logger.info("Testing model: %s", model)

    resp = io.speak(agent, "test test test", dryrun=True)
    assert resp is not None


def test_record(model):
    """
    test the model
    """
    logger.info("Testing model: %s", model)

    resp = io.record(agent, "recording test", dryrun=True)
    assert resp is not None


def test_input():
    """
    test the model
    """
    logger.info("Testing input")

    resp = io.input(agent, "input", True)
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
    tid = register_internal_tool(agent, test_tool_cb)
    logger.info("Registered tool: %d", tid)

    resp = chat.chat(agent, "hi", model=model)
    logger.info(resp)
    resp = chat.chat(agent,
                     [
                         "hi",
                         "what is sum of 123 and 456?"
                     ],
                     model=model, builtin_tools=[
                         BuiltinToolID.BuiltinToolID.Datetime,
                     ],
                     internal_tools=[
                         tid,
                     ])
    logger.info(resp)


def test_stream_data():
    """
    test streamdata
    """
    logger.info("Testing streamdata")
    stream_id = create_stream(agent)
    logger.info("Stream ID: %d", stream_id)

    stream_sendoperation(agent, stream_id, "test",
                         OperationType.Create, b"test data")

    # close stream
    close_stream(agent, stream_id)
    logger.info("Stream closed")


if __name__ == "__main__":
    agent.register_handler("handle", handle)
    agent.register_signal_handler(Signal.Signal.StreamData, handle_stream)
    agent.run()
