#!/usr/bin/env python3
# -*- coding: utf-8 -*-
import ssl
import threading
import time

import websocket

WS_DEST = "wss://localhost:8080"


def on_message(ws, message):
    ''' called when a message is received
    '''
    print(f"received: {message}")


def on_error(ws, error):
    ''' called when an error is encountered
    '''
    print(f"error: {error}")


def on_close(ws, close_status_code, close_msg):
    ''' called when the connection is closed
    '''
    print("### closed ###")


def on_open(ws):
    ''' called when the connection is opened
    '''
    def run(*args):
        ''' send a message to the server
        '''
        ws.send("Hello, Server!")
        while True:
            message = input("Enter message: ")
            if message.lower() == 'exit':
                ws.close()
                break
            ws.send(message)
    threading.Thread(target=run).start()


if __name__ == "__main__":
    ws = websocket.WebSocketApp(WS_DEST,
                                header={
                                    "Spear-Func-Streaming": "true",
                                    "Spear-Func-Type": "2",
                                    "Spear-Func-Name": "test.py",
                                },
                                on_open=on_open,
                                on_message=on_message,
                                on_error=on_error,
                                on_close=on_close)

    ws.run_forever(reconnect=5, sslopt={
        "cert_reqs": ssl.CERT_NONE,
        "check_hostname": False
    })
