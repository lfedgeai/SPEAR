#!/usr/bin/env python3
import logging
import sys
import time

import spear.client as client
import spear.transform.speech as speech

logging.basicConfig(
    level=logging.DEBUG,  # Set the desired logging level
    # Customize the log format
    format="%(asctime)s - %(name)s - %(levelname)s - %(message)s",
    handlers=[logging.StreamHandler(stream=sys.stderr)],  # Log to stderr
)

logger = logging.getLogger(__name__)
logger.setLevel(logging.DEBUG)

agent = client.HostAgent()


TEST_ASR_MODEL = "whisper-small"
WAV_FILE = "../../../misc/opea/asr/english_male_tts.wav"


def handle(ctx):
    """
    handle the request
    """
    logger.info("Handling request: %s", ctx.payload)

    test_asr(TEST_ASR_MODEL)


def test_asr(model):
    """
    test ASR
    """
    logger.info("Testing ASR: %s", model)

    # load file content using relative path from current directory
    # ../../../misc/opea/asr/english_male_tts.wav
    with open(WAV_FILE, "rb") as f:
        data = f.read()
        res = speech.audio_asr(data, model=model, agent=agent)
        logger.info(res)


if __name__ == "__main__":
    agent.register_handler("handle", handle)
    agent.loop()
