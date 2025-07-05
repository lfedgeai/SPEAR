#!/usr/bin/env python3
# -*- coding: utf-8 -*-
import argparse
import logging
import ssl
import sys
import threading

import websocket

DEST = "localhost:8080/stream"
AGENT_NAME = "opea-live-asr.py"


logging.basicConfig(
    level=logging.DEBUG,  # Set the desired logging level
    # Customize the log format
    format="%(asctime)s - %(name)s - %(levelname)s - %(message)s",
    handlers=[logging.StreamHandler(stream=sys.stderr)],  # Log to stderr
)

logger = logging.getLogger(__name__)
logger.setLevel(logging.DEBUG)


def on_message(ws, message):
    """called when a message is received"""
    print(f'received data:\n"{message}"')


def on_error(ws, error):
    """called when an error is encountered"""
    logger.error(f"Error encountered: {error}")


def on_close(ws, close_status_code, close_msg):
    """called when the connection is closed"""
    logger.info(
        f"Connection closed with status code {close_status_code} and message: {close_msg}"
    )


def on_open(ws):
    """called when the connection is opened"""

    def run(*_):
        """send a message to the server"""
        try:
            while True:
                message = input("Enter message: ")
                if message.lower() == "exit":
                    ws.close()
                    break
                ws.send(message)
        except websocket.WebSocketConnectionClosedException as e:
            logger.info(f"Connection closed: {e}")

    threading.Thread(target=run).start()


def main(args):
    """main function"""
    dest = ""
    if args.secure:
        dest = "wss://"
    else:
        dest = "ws://"
    dest += args.dest
    print(f"Connecting to {dest}...")
    ws = websocket.WebSocketApp(
        dest,
        header={
            "Spear-Func-Type": "2",
            "Spear-Func-Name": AGENT_NAME,
        },
        on_open=on_open,
        on_message=on_message,
        on_error=on_error,
        on_close=on_close,
    )

    ws.run_forever(
        reconnect=5, sslopt={"cert_reqs": ssl.CERT_NONE, "check_hostname": False}
    )


if __name__ == "__main__":
    """entry point"""
    # -s option for secure connection
    parser = argparse.ArgumentParser(description="WebSocket client")
    parser.add_argument(
        "-s",
        "--secure",
        action="store_true",
        default=False,
        help="use secure connection",
    )
    parser.add_argument("-d", "--dest", type=str, default=DEST, help="destination URL")
    args = parser.parse_args()
    main(args)
