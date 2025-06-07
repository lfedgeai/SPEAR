#!/usr/bin/env python3
# -*- coding: utf-8 -*-
import asyncio
import functools
import gzip
import json
import logging
import multiprocessing
import os
import subprocess
import time
import uuid
import wave
from enum import Enum
from io import BytesIO
from typing import Generator, List, Tuple

import aiofiles
import aiohttp
import websockets

PROTOCOL_VERSION = 0b0001
DEFAULT_HEADER_SIZE = 0b0001

PROTOCOL_VERSION_BITS = 4
HEADER_BITS = 4
MESSAGE_TYPE_BITS = 4
MESSAGE_TYPE_SPECIFIC_FLAGS_BITS = 4
MESSAGE_SERIALIZATION_BITS = 4
MESSAGE_COMPRESSION_BITS = 4
RESERVED_BITS = 8

# Message Type:
CLIENT_FULL_REQUEST = 0b0001
CLIENT_AUDIO_ONLY_REQUEST = 0b0010
SERVER_FULL_RESPONSE = 0b1001
SERVER_ACK = 0b1011
SERVER_ERROR_RESPONSE = 0b1111

# Message Type Specific Flags
NO_SEQUENCE = 0b0000  # no check sequence
POS_SEQUENCE = 0b0001
NEG_SEQUENCE = 0b0010
NEG_WITH_SEQUENCE = 0b0011
NEG_SEQUENCE_1 = 0b0011

# Message Serialization
NO_SERIALIZATION = 0b0000
JSON = 0b0001
THRIFT = 0b0011
CUSTOM_TYPE = 0b1111

# Message Compression
NO_COMPRESSION = 0b0000
GZIP = 0b0001
CUSTOM_COMPRESSION = 0b1111

ResourceIdHeader = "X-Api-Resource-Id"
AccessKeyHeader = "X-Api-Access-Key"
AppKeyHeader = "X-Api-App-Key"
ConnectIdHeader = "X-Api-Connect-Id"
RequestIdHeader = "X-Api-Request-Id"

SpearFuncTypeHeader = "Spear-Func-Type"
SpearFuncNameHeader = "Spear-Func-Name"

filedir = os.path.dirname(os.path.abspath(__file__))


def generate_header(
    version=PROTOCOL_VERSION,
    message_type=CLIENT_FULL_REQUEST,
    message_type_specific_flags=NO_SEQUENCE,
    serial_method=JSON,
    compression_type=GZIP,
    reserved_data=0x00,
    event_type=0x01,
    req_meta_data=bytes(),
    extension_header=bytes(),
):
    """
    protocol_version(4 bits), header_size(4 bits),
    message_type(4 bits), message_type_specific_flags(4 bits)
    serialization_method(4 bits) message_compression(4 bits)
    reserved （8bits) 保留字段
    header_extensions 扩展头(大小等于 8 * 4 * (header_size - 1) )
    """
    header = bytearray()
    header_size = int(len(extension_header) / 4) + 1
    header.append((version << 4) | header_size)
    header.append((message_type << 4) | message_type_specific_flags)
    header.append((serial_method << 4) | compression_type)
    header.append(reserved_data)
    # 这里需要传入len(request_meta_json)
    header.extend(req_meta_data)
    header.extend(extension_header)
    return header


def generate_full_default_header(message_type_specific_flags=POS_SEQUENCE):
    return generate_header(message_type_specific_flags=message_type_specific_flags)


def generate_audio_default_header(message_type_specific_flags=POS_SEQUENCE):
    return generate_header(
        message_type=CLIENT_AUDIO_ONLY_REQUEST,
        message_type_specific_flags=message_type_specific_flags,
    )


def generate_last_audio_default_header(message_type_specific_flags=NEG_SEQUENCE):
    return generate_header(
        message_type=CLIENT_AUDIO_ONLY_REQUEST,
        message_type_specific_flags=message_type_specific_flags,
    )


def generate_before_payload(sequence: int, event: int, session_id: str):
    before_payload = bytearray()
    before_payload.extend(sequence.to_bytes(4, "big", signed=True))  # sequence
    # before_payload.extend(event.to_bytes(4, 'big'))  # event
    # before_payload.extend(len(session_id).to_bytes(4, 'big'))  # session_id len
    # before_payload.extend(bytes(session_id, "utf-8"))  # session_id
    return before_payload


def parse_response(res):
    """
    protocol_version(4 bits), header_size(4 bits),
    message_type(4 bits), message_type_specific_flags(4 bits)
    serialization_method(4 bits) message_compression(4 bits)
    reserved （8bits) 保留字段
    header_extensions 扩展头(大小等于 8 * 4 * (header_size - 1) )
    payload 类似与http 请求体
    """
    protocol_version = res[0] >> 4
    header_size = res[0] & 0x0F
    message_type = res[1] >> 4
    message_type_specific_flags = res[1] & 0x0F
    serialization_method = res[2] >> 4
    message_compression = res[2] & 0x0F
    reserved = res[3]
    header_extensions = res[4 : header_size * 4]
    payload = res[header_size * 4 :]

    sequence = 0
    if message_type_specific_flags & 0x01:
        sequence = int.from_bytes(payload[:4], "big", signed=True)
        payload = payload[4:]
    result = {"seq": sequence}
    payload_msg = None
    payload_size = 0
    if message_type == SERVER_FULL_RESPONSE:
        payload_size = int.from_bytes(payload[:4], "big", signed=True)
        payload_msg = payload[4:]
    elif message_type == SERVER_ACK:
        seq = int.from_bytes(payload[:4], "big", signed=True)
        result["seq"] = seq
        if len(payload) >= 8:
            payload_size = int.from_bytes(payload[4:8], "big", signed=False)
            payload_msg = payload[8:]
    elif message_type == SERVER_ERROR_RESPONSE:
        code = int.from_bytes(payload[:4], "big", signed=False)
        result["code"] = code
        payload_size = int.from_bytes(payload[4:8], "big", signed=False)
        payload_msg = payload[8:]
        print(f"error response, code {code}, {payload_msg}")
    if payload_msg is None:
        return result
    if message_compression == GZIP:
        payload_msg = gzip.decompress(payload_msg)
    if serialization_method == JSON:
        payload_msg = json.loads(str(payload_msg, "utf-8"))
    elif serialization_method != NO_SERIALIZATION:
        payload_msg = str(payload_msg, "utf-8")
    result["payload_msg"] = payload_msg
    result["payload_size"] = payload_size
    return result


def read_wav_info(data: bytes) -> Tuple[int, int, int, int, bytes]:
    with BytesIO(data) as _f:
        wave_fp = wave.open(_f, "rb")
        nchannels, sampwidth, framerate, nframes = wave_fp.getparams()[:4]
        wave_bytes = wave_fp.readframes(nframes)
    return nchannels, sampwidth, framerate, nframes, wave_bytes


def judge_wav(ori_date):
    if len(ori_date) < 44:
        return False
    if ori_date[0:4] == b"RIFF" and ori_date[8:12] == b"WAVE":
        return True
    return False


def convert_wav_with_path(audio_path, sample_rate) -> bytes:
    try:
        cmd = [
            "ffmpeg",
            "-v",
            "quiet",
            "-y",
            "-i",
            audio_path,
            "-acodec",
            "pcm_s16le",
            "-ac",
            "1",
            "-ar",
            str(sample_rate),
            "-f",
            "wav",
            "-",
        ]
        process = subprocess.run(cmd, stdout=subprocess.PIPE, timeout=60)
        if os.path.exists(audio_path):
            os.remove(audio_path)
        if process.returncode != 0:
            return bytes()
        return process.stdout
    except Exception as e:
        if os.path.exists(audio_path):
            os.remove(audio_path)
        logging.warning(e)
        return bytes()


def convert_wav_with_url(url, sample_rate) -> bytes:
    if str(url).startswith("https"):
        url = url.replace("https", "http")
    cmd = [
        "ffmpeg",
        "-v",
        "quiet",
        "-y",
        "-i",
        url,
        "-acodec",
        "pcm_s16le",
        "-ac",
        "1",
        "-ar",
        str(sample_rate),
        "-f",
        "wav",
        "-",
    ]
    try:
        process = subprocess.run(cmd, stdout=subprocess.PIPE, timeout=60)
        if process.returncode != 0:
            return bytes()
        return process.stdout
    except Exception as e:
        logging.warning(e)
        return bytes()


class AudioType(Enum):
    LOCAL = 1  # 使用本地音频文件
    URL = 2  # 使用音频url


class AsrWsClient:
    def __init__(self, audio_path, **kwargs):
        """
        :param config: config
        """
        self.audio_path = audio_path
        self.success_code = 1000  # success code, default is 1000
        self.seg_duration = int(kwargs.get("seg_duration", 100))
        self.nbest = int(kwargs.get("nbest", 1))
        self.appkey = kwargs.get("appkey", "ailab_test")
        self.access_key = kwargs.get("access_key", "access_token")
        self.ws_url = kwargs.get("ws_url", "ws://localhost:8080/stream")
        self.uid = kwargs.get("uid", "ailab")
        self.workflow = kwargs.get("workflow", "audio_in,resample,vad")
        self.skip_logging = kwargs.get("skip_logging", False)
        self.show_language = kwargs.get("show_language", False)
        self.show_utterances = kwargs.get("show_utterances", False)
        self.show_word_additions = kwargs.get("show_word_additions", False)
        self.result_type = kwargs.get("result_type", "full")
        self.format = kwargs.get("format", "wav")
        self.rate = kwargs.get("sample_rate", 16000)
        self.language = kwargs.get("language", "zh-CN")
        self.bits = kwargs.get("bits", 16)
        self.channel = kwargs.get("channel", 1)
        self.codec = kwargs.get("codec", "raw")
        self.audio_type = kwargs.get("audio_type", AudioType.LOCAL)
        self.secret = kwargs.get("secret", "access_secret")
        self.hot_words = kwargs.get("hot_words", None)
        self.streaming = kwargs.get("streaming", False)
        self.enable_itn = kwargs.get("enable_itn", True)
        self.enable_ddc = kwargs.get("enable_ddc", True)
        self.enable_punc = kwargs.get("enable_punc", True)
        self.boosting_table_id = kwargs.get("boosting_table_id", "")
        self.boosting_table_name = kwargs.get("boosting_table_name", "")
        self.corpus_context = kwargs.get("corpus_context", "")
        self.resource_id = kwargs.get("resource_id", "")
        self.req_event = 1

    def construct_request(self, reqid, data=None):
        req = {
            "user": {
                "uid": self.uid,
            },
            "audio": {
                "format": self.format,
                "rate": self.rate,
                "bits": self.bits,
                "channel": self.channel,
                "codec": self.codec,
                "language": self.language,
            },
            "request": {
                "enable_itn": self.enable_itn,
                "enable_ddc": self.enable_ddc,
                "enable_punc": self.enable_punc,
                "corpus": {
                    "context": self.corpus_context,
                    "boosting_table_id": self.boosting_table_id,
                    "boosting_table_name": self.boosting_table_name,
                },
            },
        }
        return req

    @staticmethod
    def slice_data(
        data: bytes, chunk_size: int
    ) -> Generator[Tuple[bytes, bool], None, None]:
        """
        slice data
        :param data: wav data
        :param chunk_size: the segment size in one request
        :return: segment data, last flag
        """
        data_len = len(data)
        offset = 0
        while offset + chunk_size < data_len:
            yield data[offset : offset + chunk_size], False
            offset += chunk_size
        else:
            yield data[offset:data_len], True

    async def segment_data_processor(self, wav_data: bytes, segment_size: int):
        reqid = str(uuid.uuid4())
        seq = 1
        request_params = self.construct_request(reqid)
        print(request_params)
        payload_bytes = str.encode(json.dumps(request_params))
        payload_bytes = gzip.compress(payload_bytes)
        full_client_request = bytearray(
            generate_full_default_header(message_type_specific_flags=NO_SEQUENCE)
        )
        # full_client_request.extend(generate_before_payload(sequence=seq, event=self.req_event, session_id=reqid))
        full_client_request.extend(
            (len(payload_bytes)).to_bytes(4, "big")
        )  # payload size(4 bytes)
        # req_str = ' '.join(format(byte, '02x') for byte in full_client_request)
        # print("seq", seq, "req", req_str)
        full_client_request.extend(payload_bytes)  # payload
        header = {}
        print("reqid", reqid)
        header[ResourceIdHeader] = self.resource_id
        header[AccessKeyHeader] = self.access_key
        header[AppKeyHeader] = self.appkey
        header[ConnectIdHeader] = reqid
        header[RequestIdHeader] = reqid
        header[SpearFuncTypeHeader] = 2
        header[SpearFuncNameHeader] = "opea-asr-agent.py"
        # header["X-Use-Ppe"] = "1"
        # header["X-Tt-Env"] = "ppe_console_v1_0"

        print(self.ws_url)
        async with websockets.connect(
            self.ws_url,
            max_size=1000000000,
            additional_headers=header,
        ) as ws:
            print(f"Sending full client request: {full_client_request.hex(' ')}")
            await ws.send(full_client_request)
            res = await ws.recv()
            print(f"Received response: {res.hex(' ')}")
            # res_str = ' '.join(format(byte, '02x') for byte in res)
            # print(res_str)
            result = parse_response(res)
            print(result)
            # if 'payload_msg' in result and result['payload_msg']['code'] != self.success_code:
            #     return result
            for _, (chunk, last) in enumerate(
                AsrWsClient.slice_data(wav_data, segment_size), 1
            ):
                # if no compression, comment this line
                seq += 1
                if last:
                    seq = -seq
                start = time.time()
                payload_bytes = gzip.compress(chunk)
                audio_only_request = bytearray(
                    generate_audio_default_header(
                        message_type_specific_flags=NO_SEQUENCE
                    )
                )
                if last:
                    audio_only_request = bytearray(
                        generate_last_audio_default_header(
                            message_type_specific_flags=NEG_SEQUENCE
                        )
                    )
                # audio_only_request.extend(generate_before_payload(sequence=abs(seq), event=self.req_event, session_id=reqid))
                audio_only_request.extend(
                    (len(payload_bytes)).to_bytes(4, "big")
                )  # payload size(4 bytes)
                # req_str = " ".join(format(byte, "02x") for byte in audio_only_request)
                # print("seq", seq, "req", req_str)
                audio_only_request.extend(payload_bytes)  # payload
                await ws.send(audio_only_request)
                res = await ws.recv()
                # print("seq", seq, "res", res)
                # res_str = " ".join(format(byte, "02x") for byte in res)
                # print("seq", seq, "res", res_str)
                result = parse_response(res)
                print("seq", seq, "res", result)
                # if 'payload_msg' in result and result['payload_msg']['code'] != self.success_code:
                #     return result
                if self.streaming:
                    sleep_time = max(
                        0, (self.seg_duration / 1000.0 - (time.time() - start))
                    )
                    await asyncio.sleep(sleep_time)
        return result

    async def execute(self):
        if self.audio_type == AudioType.LOCAL:
            async with aiofiles.open(self.audio_path, mode="rb") as _f:
                data = await _f.read()
        else:
            async with aiohttp.ClientSession() as _session:
                async with _session.get(self.audio_path) as resp:
                    data = await resp.content.read()
        audio_data = bytes(data)
        if self.format in ("mp3", "ogg", "pcm"):
            # mp3 暂时全量发送
            segment_size = self.seg_duration
            return await self.segment_data_processor(audio_data, segment_size)
        if self.format == "any":
            segment_size = len(audio_data)
            return self.segment_data_processor(audio_data, segment_size)
        if self.format != "wav" and self.format != "pcm":
            raise Exception("format should in wav, pcm or mp3")
        if not judge_wav(audio_data):
            if self.audio_type == AudioType.LOCAL:
                audio_data = convert_wav_with_path(self.audio_path, self.rate)
            else:
                audio_data = convert_wav_with_url(self.audio_path, self.rate)
        nchannels, sampwidth, framerate, nframes, wave_bytes = read_wav_info(audio_data)
        size_per_sec = nchannels * sampwidth * framerate
        segment_size = int(size_per_sec * self.seg_duration / 1000)
        print(segment_size)
        if self.format == "pcm":
            audio_data = wave_bytes
        return await self.segment_data_processor(audio_data, segment_size)


def execute_one(audio_item, **kwargs):
    """

    :param audio_item: {"id": xxx, "path": "xxx"}
    :return:
    """
    assert "id" in audio_item
    assert "path" in audio_item
    audio_id = audio_item["id"]
    audio_path = audio_item["path"]
    if str(audio_path).startswith("http"):
        audio_type = AudioType.URL
    else:
        audio_type = AudioType.LOCAL
    asr_http_client = AsrWsClient(
        audio_path=audio_path, audio_type=audio_type, **kwargs
    )
    result = asyncio.run(asr_http_client.execute())
    return {"id": audio_id, "path": audio_path, "result": result}


def execute_multi(audio_list: List[dict], parallel: int):
    """
    :param audio_list: [{"id": xxx, "path": "xxx"}]
    :param parallel: 10
    :return:
    """
    task = functools.partial(execute_one)
    with multiprocessing.Pool(parallel) as _pool:
        results = _pool.map(task, audio_list)
    return results


def test_multi():
    print("测试多条")
    audio_list = [
        {
            "id": i,
            "path": "https://tosv.byted.org/obj/lab-speech-pelican-testset/smart_dog_201912_993_1_6687362551523098635.wav",
        }
        for i in range(10)
    ]
    results = execute_multi(audio_list, parallel=10)
    print(results)


def test_one():
    print("测试单条")
    result = execute_one(
        {
            "id": 1,
            "path": "https://tosv.byted.org/obj/ailab-speech-hproject-product/robot_eh_00_479d27a9-3923-4b80-8548-d97078c813d0.ogg",
        },
        format="ogg",
        codec="opus",
    )
    print(result)


def test_stream():
    result = execute_one(
        {
            "id": 1,
            "path": f"{filedir}/../asr/english_male_tts.wav",
        },
        seg_duration=100,
        appkey="",
        access_key="",
        resource_id="asr.streaming.model.big",
        # format="ogg",
        # format="pcm",
    )
    print(result)
    with open("sauc_result.json", "w") as f:
        f.write(json.dumps(result["result"], ensure_ascii=False, indent=2))


def test_url():
    print("测试http request参数使用url")
    result = execute_one(
        {
            "id": 1,
            "path": "https://voiceage.com/wbsamples/in_mono/Conference.wav",
        },
        seg_duration=100,
        appkey="",
        access_key="",
        resource_id="asr.streaming.model.big",
    )
    print(result)


if __name__ == "__main__":
    # print the current working directory
    test_stream()
    test_url()
