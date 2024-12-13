#!/usr/bin/env python3
import logging

import spear.client as client

logger = logging.getLogger(__name__)

def input(agent: client.HostAgent, prompt: str) -> str:
    """
    get user input
    """
    user_input = agent.exec_request(
        "input",
        prompt,
    )
    if isinstance(user_input, client.JsonRpcOkResp):
        user_input = user_input.result
    else:
        raise ValueError("Error getting user input")
    return user_input


def speak(agent: client.HostAgent, data) -> str:
    """
    get user input
    """
    res = agent.exec_request(
        "speak",
        data,
    )
    if isinstance(res, client.JsonRpcOkResp):
        return
    else:
        raise ValueError("Error speaking")

def record(agent: client.HostAgent, prompt: str) -> str:
    """
    get user input
    """
    res = agent.exec_request(
        "record",
        prompt,
    )
    if isinstance(res, client.JsonRpcOkResp):
        res = res.result
    else:
        raise ValueError("Error recording")
    return res
