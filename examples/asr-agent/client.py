#!/usr/bin/env python3
import logging

import json
import pyaudio
import websocket
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
            logger.info("Recording audio...")
            while True:
                data = stream.read(1024)
                ws.send(data)
                # receive response without blocking
                try:
                    response = ws.recv(timeout=0.01, decode=True)
                    data = json.loads(response)
                    if not isinstance(data, list):
                        logger.error(f"Unexpected response format: {data}")
                        continue
                    if len(data) == 0:
                        continue
                    logger.info(f"YOU SAID: {data[0]}")
                    for i in data[1:]:
                        logger.info(f"Response: {i}")
                except TimeoutError:
                    pass
        except websocket.WebSocketConnectionClosedException as e:
            logger.info(f"Connection closed: {e}")
        finally:
            stream.stop_stream()
            stream.close()
            audio.terminate()


if __name__ == "__main__":
    main()
