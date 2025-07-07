#!/usr/bin/env python3
import json
import logging
import os
import platform
import subprocess
import tempfile

import pyaudio
import requests
import websocket
import websockets
from websockets.sync.client import connect

logging.basicConfig(
    level=logging.INFO,  # Set the desired logging level
    # Customize the log format
    format="%(asctime)s - %(name)s - %(levelname)s - %(message)s",
    handlers=[logging.StreamHandler()],  # Log to stderr
)
logger = logging.getLogger(__name__)
logger.setLevel(logging.INFO)

SpearFuncTypeHeader = "Spear-Func-Type"
SpearFuncNameHeader = "Spear-Func-Name"


def speak(text: str):
    """Convert text to speech using pyttsx3."""
    if not text:
        return
    # run following command example
    #     curl https://api.openai.com/v1/audio/speech \
    #   -H "Authorization: Bearer $OPENAI_API_KEY" \
    #   -H "Content-Type: application/json" \
    #   -d '{
    #     "model": "gpt-4o-mini-tts",
    #     "input": "Today is a wonderful day to build something people love!",
    #     "voice": "coral",
    #     "instructions": "Speak in a cheerful and positive tone.",
    #     "response_format": "wav"
    #   }' | ffplay -i -
    try:
        # get environment variable OPENAI_API_KEY
        openai_api_key = os.getenv("OPENAI_API_KEY")
        if not openai_api_key:
            logger.error("OPENAI_API_KEY environment variable is not set.")
            return
        # send request to OpenAI API
        response = requests.post(
            "https://api.openai.com/v1/audio/speech",
            headers={
                "Authorization": f"Bearer {openai_api_key}",
                "Content-Type": "application/json",
            },
            json={
                "model": "tts-1",
                "input": text,
                "voice": "fable",
                "response_format": "wav",
            },
            timeout=10,
        )
        if response.status_code != 200:
            logger.error(
                f"Failed to get response from OpenAI API: {response.status_code} - {response.text}"
            )
            return
        # save the response content to a temporary file
        with tempfile.NamedTemporaryFile(delete=False, suffix=".wav") as temp_file:
            temp_file.write(response.content)
            temp_file_path = temp_file.name
        try:
            # play the audio
            if platform.system() == "Windows":
                os.startfile(temp_file_path)  # Windows-specific way to play audio
            elif platform.system() == "Darwin":
                subprocess.call(
                    ["afplay", temp_file_path]
                )  # macOS-specific way to play audio
            else:
                subprocess.call(
                    ["aplay", temp_file_path]
                )  # Linux-specific way to play audio
        finally:
            # Clean up the temporary file
            try:
                os.remove(temp_file_path)
            except Exception as cleanup_error:
                logger.warning(f"Failed to delete temporary file {temp_file_path}: {cleanup_error}")
    except requests.RequestException as e:
        logger.error(f"HTTP request error occurred while trying to speak: {e}")
    except subprocess.CalledProcessError as e:
        logger.error(f"Subprocess error occurred while trying to play audio: {e}")
    except OSError as e:
        logger.error(f"OS error occurred while handling audio file: {e}")
    except Exception as e:
        logger.error(f"An unexpected error occurred while trying to speak: {e}")


def main():
    """Main function to start the WebSocket client."""
    dest = "ws://localhost:8080/stream"
    logger.info(f"Connecting to {dest}...")

    header = {}

    header[SpearFuncTypeHeader] = 2
    header[SpearFuncNameHeader] = "realtime-agent.py"

    with connect(
        dest,
        additional_headers=header,
    ) as ws:
        logger.info("WebSocket connection established.")

        # Set up event handlers
        ws.on_open = lambda ws: logger.info("Connection opened.")
        ws.on_message = lambda ws, message: logger.info(f"Received message: {message}")
        ws.on_error = lambda ws, error: logger.error(f"Error encountered: {error}")
        ws.on_close = lambda ws, close_status_code, close_msg: logger.info(
            f"Connection closed with status code {close_status_code} and message: {close_msg}"
        )

        audio = pyaudio.PyAudio()
        stream = audio.open(
            format=pyaudio.paInt16,
            channels=1,
            rate=24000,
            input=True,
            frames_per_buffer=1024,
        )

        try:
            logger.info("Please start speaking...")
            while True:
                stream.start_stream()
                data = stream.read(
                    stream.get_read_available(), exception_on_overflow=False
                )
                ws.send(data)
                # receive response without blocking
                try:
                    response = ws.recv(timeout=0.01, decode=True)
                    stream.stop_stream()
                    data = json.loads(response)
                    if not isinstance(data, list):
                        logger.error(f"Unexpected response format: {data}")
                        continue
                    if len(data) == 0:
                        continue
                    assert isinstance(data[0], list) and len(data[0]) == 2
                    logger.info(f"YOU SAID: {data[0][1]}")
                    for i in data[1:]:
                        assert isinstance(i, list) and len(i) == 2
                        if i[0] == "assistant":
                            # print in green color
                            logger.info(f"\033[92m{i[1]}\033[0m")
                            speak(i[1])  # Speak the first item in the response
                except TimeoutError:
                    pass
                except websockets.exceptions.ConnectionClosedError as e:
                    logger.info(f"Connection closed: {e}")
                    break
        except websocket.WebSocketConnectionClosedException as e:
            logger.info(f"Connection closed: {e}")
        except KeyboardInterrupt:
            logger.info("Keyboard interrupt received. Stopping the client.")
        finally:
            stream.stop_stream()
            stream.close()
            audio.terminate()


if __name__ == "__main__":
    main()
