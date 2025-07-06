#!/usr/bin/env python3
import json
import logging

import pyaudio
import pyttsx3
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
    engine = pyttsx3.init()
    engine.setProperty("rate", 150)  # Set speech rate
    engine.setProperty("volume", 1)  # Set volume level (0.0 to 1.0)
    engine.say(text)
    engine.runAndWait()
    engine.stop()


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
