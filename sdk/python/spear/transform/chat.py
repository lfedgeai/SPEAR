#!/usr/bin/env python3
import logging
import sys
from typing import Tuple, Union

import flatbuffers as fbs
import spear.client as client

from spear.proto.chat import (ChatCompletionRequest, ChatCompletionResponse,
                              ChatMessage, ChatMetadata, Role)
from spear.proto.chat import ToolInfo as ChatToolInfo
from spear.proto.tool import BuiltinToolInfo, InternalToolInfo, ToolInfo
from spear.proto.transform import (TransformOperation, TransformRequest,
                                   TransformRequest_Params, TransformResponse,
                                   TransformResponse_Data, TransformType)
from spear.proto.transport import Method

logging.basicConfig(
    level=logging.INFO,  # Set the desired logging level
    # Customize the log format
    format="%(asctime)s - %(name)s - %(levelname)s - %(message)s",
    handlers=[logging.StreamHandler(stream=sys.stderr)],  # Log to stderr
)

logger = logging.getLogger(__name__)
logger.setLevel(logging.INFO)

DEFAULT_LLM_MODEL = "llama"  # "gpt-4o"


def chat(
    message: Union[str, list[str], list[Tuple[str, str]]],
    model: str = DEFAULT_LLM_MODEL,
    builtin_tools: list[int] = [],
    internal_tools: list[int] = [],
    agent: client.HostAgent = client.global_agent(),
):
    """
    handle the llm request
    """
    role_offs = []
    content_offs = []
    if isinstance(message, list) and len(message) > 0 and isinstance(message[0], str):
        builder = fbs.Builder(sum([len(m) for m in message]) + 2048)
        for m in message:
            if not isinstance(m, str):
                raise ValueError("Invalid message type")
            content_offs.append(builder.CreateString(m))
            role_offs.append(Role.Role.User)
    elif isinstance(message, str):
        builder = fbs.Builder(len(message) + 2048)
        content_offs.append(builder.CreateString(message))
        role_offs.append(Role.Role.User)
    elif (
        isinstance(message, list)
        and len(message) > 0
        and isinstance(message[0], tuple)
        and len(message[0]) == 2
        and isinstance(message[0][0], str)
        and isinstance(message[0][1], str)
    ):
        builder = fbs.Builder(sum([len(m[1]) for m in message]) + 2048)
        for m in message:
            content_offs.append(builder.CreateString(m[1]))
            if m[0] == "user":
                role_offs.append(Role.Role.User)
            elif m[0] == "assistant":
                role_offs.append(Role.Role.Assistant)
            elif m[0] == "system":
                role_offs.append(Role.Role.System)
            elif m[0] == "developer":
                role_offs.append(Role.Role.Developer)
            elif m[0] == "tool":
                role_offs.append(Role.Role.Tool)
            elif m[0] == "other":
                role_offs.append(Role.Role.Other)
            else:
                raise ValueError("Invalid message type")
    else:
        raise ValueError("Invalid message type")
    model_off = builder.CreateString(model)

    tools_off = -1
    builtin_tool_offs = []
    internal_tool_offs = []
    if len(builtin_tools) > 0:
        for tool in builtin_tools:
            assert isinstance(tool, int)
            BuiltinToolInfo.BuiltinToolInfoStart(builder)
            BuiltinToolInfo.AddToolId(builder, tool)
            tmp = BuiltinToolInfo.End(builder)
            ChatToolInfo.ToolInfoStart(builder)
            ChatToolInfo.ToolInfoAddData(builder, tmp)
            ChatToolInfo.AddDataType(
                builder,
                ToolInfo.ToolInfo.BuiltinToolInfo,
            )
            builtin_tool_offs.append(ChatToolInfo.End(builder))
    if len(internal_tools) > 0:
        for tool in internal_tools:
            assert isinstance(tool, int)
            InternalToolInfo.InternalToolInfoStart(builder)
            InternalToolInfo.AddToolId(builder, tool)
            tmp = InternalToolInfo.End(builder)
            ChatToolInfo.ToolInfoStart(builder)
            ChatToolInfo.ToolInfoAddData(builder, tmp)
            ChatToolInfo.AddDataType(
                builder,
                ToolInfo.ToolInfo.InternalToolInfo,
            )
            internal_tool_offs.append(ChatToolInfo.End(builder))

    if len(builtin_tool_offs) + len(internal_tool_offs) > 0:
        ChatCompletionRequest.StartToolsVector(
            builder, len(builtin_tool_offs) + len(internal_tool_offs)
        )
        for off in builtin_tool_offs:
            builder.PrependUOffsetTRelative(off)
        for off in internal_tool_offs:
            builder.PrependUOffsetTRelative(off)
        tools_off = builder.EndVector()

    msg_offs = []
    for i, off in enumerate(content_offs):
        role = role_offs[i]
        ChatMetadata.ChatMetadataStart(builder)
        ChatMetadata.AddRole(builder, role)
        meta_off = ChatMetadata.End(builder)

        ChatMessage.ChatMessageStart(builder)
        ChatMessage.AddContent(builder, off)
        ChatMessage.AddMetadata(builder, meta_off)
        msg_offs.append(ChatMessage.End(builder))

    ChatCompletionRequest.StartMessagesVector(builder, len(msg_offs))
    for msg_off in reversed(msg_offs):
        builder.PrependUOffsetTRelative(msg_off)
    msglist_off = builder.EndVector()

    ChatCompletionRequest.ChatCompletionRequestStart(builder)
    ChatCompletionRequest.AddMessages(builder, msglist_off)
    ChatCompletionRequest.AddModel(builder, model_off)
    if tools_off != -1:
        ChatCompletionRequest.AddTools(builder, tools_off)
    chatcomp_off = ChatCompletionRequest.End(builder)

    TransformRequest.StartInputTypesVector(builder, 1)
    builder.PrependInt32(TransformType.TransformType.Text)
    input_types_off = builder.EndVector()

    TransformRequest.StartOutputTypesVector(builder, 1)
    builder.PrependInt32(TransformType.TransformType.Text)
    output_types_off = builder.EndVector()

    TransformRequest.StartOperationsVector(builder, 1)
    builder.PrependInt32(TransformOperation.TransformOperation.LLM)
    if len(builtin_tools) > 0 or len(internal_tools) > 0:
        builder.PrependInt32(TransformOperation.TransformOperation.Tools)
    operations_off = builder.EndVector()

    TransformRequest.TransformRequestStart(builder)
    TransformRequest.AddInputTypes(builder, input_types_off)
    TransformRequest.AddOutputTypes(builder, output_types_off)
    TransformRequest.AddOperations(builder, operations_off)
    TransformRequest.AddParams(builder, chatcomp_off)
    TransformRequest.AddParamsType(
        builder,
        TransformRequest_Params.TransformRequest_Params.spear_proto_chat_ChatCompletionRequest,
    )
    builder.Finish(TransformRequest.End(builder))

    data = agent.exec_request(Method.Method.Transform, builder.Output())

    resp = TransformResponse.TransformResponse.GetRootAsTransformResponse(data, 0)
    if (
        resp.DataType()
        != TransformResponse_Data.TransformResponse_Data.spear_proto_chat_ChatCompletionResponse
    ):
        raise ValueError("Unexpected response data type")

    chat_resp = ChatCompletionResponse.ChatCompletionResponse()
    chat_resp.Init(resp.Data().Bytes, resp.Data().Pos)

    if chat_resp.Code() != 0:
        raise ValueError(chat_resp.Error())

    msg_len = chat_resp.MessagesLength()
    res = []
    for i in range(msg_len):
        role = chat_resp.Messages(i).Metadata().Role()
        if role == Role.Role.Assistant:
            role_str = "assistant"
        elif role == Role.Role.Developer:
            role_str = "developer"
        elif role == Role.Role.System:
            role_str = "system"
        elif role == Role.Role.User:
            role_str = "user"
        elif role == Role.Role.Tool:
            role_str = "tool"
        elif role == Role.Role.Other:
            role_str = "other"
        else:
            raise Exception(f"invalid role value {role}")
        res.append((role_str, chat_resp.Messages(i).Content().decode("utf-8")))

    return res
