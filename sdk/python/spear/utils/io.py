#!/usr/bin/env python3
import logging

import flatbuffers as fbs
import spear.client as client

from spear.proto.io import (InputRequest, InputResponse, RecordRequest,
                            RecordResponse, SpeakRequest, SpeakResponse)
from spear.proto.transport import Method

logger = logging.getLogger(__name__)


def input(
    prompt: str, dryrun: bool = False, agent: client.HostAgent = client.global_agent()
) -> str:
    """
    get user input
    """
    builder = fbs.Builder(len(prompt) + 32)
    prompt_off = builder.CreateString(prompt)
    InputRequest.InputRequestStart(builder)
    InputRequest.InputRequestAddPrompt(builder, prompt_off)
    InputRequest.AddDryrun(builder, dryrun)
    data_off = InputRequest.InputRequestEnd(builder)
    builder.Finish(data_off)

    data = agent.exec_request(
        Method.Method.Input,
        builder.Output(),
    )

    resp = InputResponse.InputResponse.GetRootAsInputResponse(data, 0)
    return resp.Text()


def speak(
    data: str,
    model: str = None,
    voice: str = None,
    fmt: str = None,
    dryrun=False,
    agent: client.HostAgent = client.global_agent(),
) -> bytes:
    """
    get user input
    """
    builder = fbs.Builder(len(data) + 32)
    data_off = builder.CreateString(data)
    if model:
        model_off = builder.CreateString(model)
    if voice:
        voice_off = builder.CreateString(voice)
    if fmt:
        fmt_off = builder.CreateString(format)
    SpeakRequest.SpeakRequestStart(builder)
    SpeakRequest.SpeakRequestAddText(builder, data_off)
    if model:
        SpeakRequest.SpeakRequestAddModel(builder, model_off)
    if voice:
        SpeakRequest.SpeakRequestAddVoice(builder, voice_off)
    if fmt:
        SpeakRequest.SpeakRequestAddFormat(builder, fmt_off)
    if dryrun:
        SpeakRequest.SpeakRequestAddDryrun(builder, dryrun)

    data_off = SpeakRequest.SpeakRequestEnd(builder)
    builder.Finish(data_off)
    res = agent.exec_request(
        Method.Method.Speak,
        builder.Output(),
    )
    resp = SpeakResponse.SpeakResponse.GetRootAsSpeakResponse(res, 0)
    return resp.Data()


def record(
    prompt: str,
    model: str = "whisper-1",
    dryrun=False,
    agent: client.HostAgent = client.global_agent(),
) -> str:
    """
    get user input
    """
    builder = fbs.Builder(len(prompt) + 32)
    prompt_off = builder.CreateString(prompt)
    if model:
        model_off = builder.CreateString(model)

    RecordRequest.RecordRequestStart(builder)
    RecordRequest.RecordRequestAddPrompt(builder, prompt_off)
    if model:
        RecordRequest.RecordRequestAddModel(builder, model_off)
    RecordRequest.RecordRequestAddDryrun(builder, dryrun)
    data_off = RecordRequest.RecordRequestEnd(builder)
    builder.Finish(data_off)
    res = agent.exec_request(
        Method.Method.Record,
        builder.Output(),
    )

    resp = RecordResponse.RecordResponse.GetRootAsRecordResponse(res, 0)
    return resp.Text()
