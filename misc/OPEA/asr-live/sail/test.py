#!/usr/bin/env python3
import gzip
import json
import struct
import unittest

from sail.proto import (COMPRESSION_GZIP, COMPRESSION_NONE, ERR_AUDIO_FORMAT,
                        ERR_EMPTY_AUDIO, ERR_INVALID_PARAM, ERR_SERVER_BUSY,
                        ERR_SUCCESS, ERR_TIMEOUT, FLAG_LAST_AUDIO,
                        FLAG_LAST_RESPONSE, FLAG_NORMAL, HEADER_SIZE_VALUE,
                        MSG_AUDIO_ONLY_REQUEST, MSG_FULL_CLIENT_REQUEST,
                        MSG_FULL_SERVER_RESPONSE, MSG_SERVER_ERROR,
                        PROTOCOL_VERSION, SERIALIZATION_JSON,
                        SERIALIZATION_NONE, AudioOnlyRequest, BaseMessage,
                        ErrorResponse, FullClientRequest, FullServerResponse,
                        Header, SAILProtocolHandler)


class TestHeader(unittest.TestCase):
    def test_header_packing(self):
        header = Header(
            MSG_FULL_CLIENT_REQUEST, FLAG_NORMAL, SERIALIZATION_JSON, COMPRESSION_GZIP
        )
        packed = header.pack()

        # Verify length
        self.assertEqual(len(packed), 4)

        # Unpack manually
        byte0, byte1, byte2, byte3 = struct.unpack("!4B", packed)

        # Verify fields
        self.assertEqual(byte0, (PROTOCOL_VERSION << 4) | HEADER_SIZE_VALUE)
        self.assertEqual(byte1, (MSG_FULL_CLIENT_REQUEST << 4) | FLAG_NORMAL)
        self.assertEqual(byte2, (SERIALIZATION_JSON << 4) | COMPRESSION_GZIP)
        self.assertEqual(byte3, 0)

    def test_header_unpacking(self):
        # Create binary header data
        data = struct.pack(
            "!4B",
            (PROTOCOL_VERSION << 4) | HEADER_SIZE_VALUE,
            (MSG_AUDIO_ONLY_REQUEST << 4) | FLAG_LAST_AUDIO,
            (SERIALIZATION_NONE << 4) | COMPRESSION_NONE,
            0,
        )

        header = Header.unpack(data)

        # Verify fields
        self.assertEqual(header.version, PROTOCOL_VERSION)
        self.assertEqual(header.header_size, HEADER_SIZE_VALUE)
        self.assertEqual(header.message_type, MSG_AUDIO_ONLY_REQUEST)
        self.assertEqual(header.flags, FLAG_LAST_AUDIO)
        self.assertEqual(header.serialization, SERIALIZATION_NONE)
        self.assertEqual(header.compression, COMPRESSION_NONE)

    def test_invalid_header(self):
        # Test short header
        with self.assertRaises(ValueError):
            Header.unpack(b"\x01")

        # Test invalid version
        invalid_ver = struct.pack("!4B", (2 << 4) | 1, 0, 0, 0)
        with self.assertRaisesRegex(ValueError, "Unsupported protocol version"):
            Header.unpack(invalid_ver)

        # Test invalid header size
        invalid_size = struct.pack("!4B", (1 << 4) | 2, 0, 0, 0)
        with self.assertRaisesRegex(ValueError, "Unsupported header size"):
            Header.unpack(invalid_size)


class TestFullClientRequest(unittest.TestCase):
    def setUp(self):
        self.config = {
            "user": {"uid": "test_user"},
            "audio": {"format": "wav", "rate": 16000},
            "request": {"model_name": "bigmodel"},
        }
        self.header = Header(
            MSG_FULL_CLIENT_REQUEST, FLAG_NORMAL, SERIALIZATION_JSON, COMPRESSION_NONE
        )

    def test_json_serialization(self):
        request = FullClientRequest(self.header, self.config)
        payload = request.pack_payload()

        # Should be JSON string without compression
        decoded = json.loads(payload.decode("utf-8"))
        self.assertEqual(decoded, self.config)

    def test_gzip_compression(self):
        # Create header with compression
        comp_header = Header(
            MSG_FULL_CLIENT_REQUEST, FLAG_NORMAL, SERIALIZATION_JSON, COMPRESSION_GZIP
        )
        request = FullClientRequest(comp_header, self.config)
        payload = request.pack_payload()

        # Decompress and decode
        decompressed = gzip.decompress(payload)
        decoded = json.loads(decompressed.decode("utf-8"))
        self.assertEqual(decoded, self.config)

    def test_parse_payload(self):
        # Create JSON payload
        payload_data = json.dumps(self.config).encode("utf-8")

        # Parse without compression
        parsed = FullClientRequest.parse_payload(
            payload_data, SERIALIZATION_JSON, COMPRESSION_NONE
        )
        self.assertEqual(parsed, self.config)

        # Parse with compression
        compressed = gzip.compress(payload_data)
        parsed = FullClientRequest.parse_payload(
            compressed, SERIALIZATION_JSON, COMPRESSION_GZIP
        )
        self.assertEqual(parsed, self.config)


class TestAudioOnlyRequest(unittest.TestCase):
    def setUp(self):
        self.audio_data = b"\x00\x01\x02\x03" * 100  # 400 bytes of fake audio
        self.header = Header(
            MSG_AUDIO_ONLY_REQUEST,
            FLAG_LAST_AUDIO,
            SERIALIZATION_NONE,
            COMPRESSION_NONE,
        )

    def test_pack_unpack(self):
        request = AudioOnlyRequest(self.header, self.audio_data)

        # Should return same data without compression
        packed = request.pack_payload()
        self.assertEqual(packed, self.audio_data)

        # Parse should return original data
        parsed = AudioOnlyRequest.parse_payload(packed, COMPRESSION_NONE)
        self.assertEqual(parsed, self.audio_data)

    def test_gzip_compression(self):
        comp_header = Header(
            MSG_AUDIO_ONLY_REQUEST, FLAG_NORMAL, SERIALIZATION_NONE, COMPRESSION_GZIP
        )
        request = AudioOnlyRequest(comp_header, self.audio_data)
        packed = request.pack_payload()

        # Should be compressed
        self.assertLess(len(packed), len(self.audio_data))

        # Parse should decompress to original
        parsed = AudioOnlyRequest.parse_payload(packed, COMPRESSION_GZIP)
        self.assertEqual(parsed, self.audio_data)

    def test_is_last_flag(self):
        # With last audio flag
        last_header = Header(
            MSG_AUDIO_ONLY_REQUEST,
            FLAG_LAST_AUDIO,
            SERIALIZATION_NONE,
            COMPRESSION_NONE,
        )
        last_request = AudioOnlyRequest(last_header, b"")
        self.assertTrue(last_request.is_last)

        # Without last audio flag
        normal_header = Header(
            MSG_AUDIO_ONLY_REQUEST, FLAG_NORMAL, SERIALIZATION_NONE, COMPRESSION_NONE
        )
        normal_request = AudioOnlyRequest(normal_header, b"")
        self.assertFalse(normal_request.is_last)


class TestServerResponses(unittest.TestCase):
    def test_full_server_response(self):
        response_data = {"result": {"text": "Hello world", "confidence": 95}}
        header = Header(
            MSG_FULL_SERVER_RESPONSE,
            FLAG_LAST_RESPONSE,
            SERIALIZATION_JSON,
            COMPRESSION_GZIP,
        )
        response = FullServerResponse(header, 123, response_data)

        # Test payload packing
        payload = response.pack_payload()
        decompressed = gzip.decompress(payload)
        decoded = json.loads(decompressed.decode("utf-8"))
        self.assertEqual(decoded, response_data)

        # Test payload size calculation
        self.assertEqual(response.get_payload_size(), len(payload))

    def test_error_response(self):
        # Change from dict to string
        error_message = "Invalid parameter"
        header = Header(
            MSG_SERVER_ERROR, FLAG_NORMAL, SERIALIZATION_NONE, COMPRESSION_NONE
        )
        response = ErrorResponse(header, ERR_INVALID_PARAM, error_message)

        handler = SAILProtocolHandler()
        # Serialize the error response
        serialized = handler.serialize_message(response)

        # Test payload packing
        payload = serialized[4:]  # Skip header (first 4 bytes)

        # Should be: [error_code (4B)] [size (4B)] [message]
        code, size = struct.unpack("!II", payload[:8])
        message = payload[8:].decode("utf-8")

        self.assertEqual(code, ERR_INVALID_PARAM)
        self.assertEqual(size, len(error_message))
        self.assertEqual(message, error_message)

        # Test payload size
        self.assertEqual(response.get_payload_size(), 8 + len(error_message))


class TestProtocolHandler(unittest.TestCase):
    def setUp(self):
        self.handler = SAILProtocolHandler()
        self.config = {
            "user": {"uid": "test_user"},
            "audio": {"format": "wav", "rate": 16000},
            "request": {"model_name": "bigmodel"},
        }

    def create_client_request(
        self, serialization=SERIALIZATION_JSON, compression=COMPRESSION_NONE
    ):
        """Helper to create full client request binary"""
        header = Header(
            MSG_FULL_CLIENT_REQUEST, FLAG_NORMAL, serialization, compression
        )
        request = FullClientRequest(header, self.config)

        # Build binary message: header + payload_size + payload
        payload = request.pack_payload()
        payload_size = struct.pack("!I", len(payload))
        return header.pack() + payload_size + payload

    def test_parse_full_client_request(self):
        # Create request with JSON and no compression
        request_data = self.create_client_request()
        message = self.handler.parse_message(request_data)

        self.assertIsInstance(message, FullClientRequest)
        self.assertEqual(message.payload_data, self.config)

        # Verify handler state updated
        self.assertEqual(self.handler.serialization, SERIALIZATION_JSON)
        self.assertEqual(self.handler.compression, COMPRESSION_NONE)

    def test_parse_audio_request(self):
        # First parse client request to set handler state
        request_data = self.create_client_request()
        self.handler.parse_message(request_data)

        # Create audio request
        audio_data = b"\x01\x02\x03" * 100
        header = Header(
            MSG_AUDIO_ONLY_REQUEST, FLAG_NORMAL, SERIALIZATION_NONE, COMPRESSION_GZIP
        )
        payload = gzip.compress(audio_data)
        payload_size = struct.pack("!I", len(payload))
        audio_msg = header.pack() + payload_size + payload

        # Parse audio request
        message = self.handler.parse_message(audio_msg)
        self.assertIsInstance(message, AudioOnlyRequest)
        self.assertEqual(message.audio_data, audio_data)

    def test_create_responses(self):
        # Parse client request
        request_data = self.create_client_request()
        self.handler.parse_message(request_data)

        # Create server response
        result_data = {"text": "Test response"}
        response = self.handler.create_full_response(1, result_data)
        self.assertIsInstance(response, FullServerResponse)

        # Verify response uses same serialization/compression
        self.assertEqual(response.header.serialization, SERIALIZATION_JSON)
        self.assertEqual(response.header.compression, COMPRESSION_NONE)

        # Create error response
        error_response = self.handler.create_error_response(
            ERR_INVALID_PARAM, "Test error"
        )
        self.assertIsInstance(error_response, ErrorResponse)

    def test_serialize_messages(self):
        # Test serialization of FullServerResponse
        header = Header(
            MSG_FULL_SERVER_RESPONSE, FLAG_NORMAL, SERIALIZATION_JSON, COMPRESSION_NONE
        )
        response = FullServerResponse(header, 1, {"text": "Hello"})
        serialized = self.handler.serialize_message(response)

        # Should have: header (4B) + sequence (4B) + payload_size (4B) + payload
        self.assertEqual(
            len(serialized), 4 + 4 + 4 + len(json.dumps({"text": "Hello"}).encode())
        )

        # Test serialization of ErrorResponse
        error_header = Header(
            MSG_SERVER_ERROR, FLAG_NORMAL, SERIALIZATION_JSON, COMPRESSION_NONE
        )
        error_resp = ErrorResponse(error_header, ERR_INVALID_PARAM, "Test error")
        serialized = self.handler.serialize_message(error_resp)
        # Header + error code + payload size
        self.assertGreater(len(serialized), 12)

        # Test serialization of ErrorResponse
        error_header = Header(
            MSG_SERVER_ERROR, FLAG_NORMAL, SERIALIZATION_NONE, COMPRESSION_NONE
        )
        # Change from dict to string
        error_resp = ErrorResponse(error_header, ERR_INVALID_PARAM, "Test error")
        serialized = self.handler.serialize_message(error_resp)

        # Should have: header (4B) + error_code (4B) + msg_size (4B) + message
        self.assertEqual(len(serialized), 4 + 4 + 4 + len("Test error"))

    def test_unknown_message_type(self):
        # Create invalid message type
        invalid_header = struct.pack(
            "!4B",
            (PROTOCOL_VERSION << 4) | HEADER_SIZE_VALUE,
            (0b0100 << 4) | 0,  # Invalid type
            0,
            0,
        )
        invalid_msg = invalid_header + struct.pack("!I", 0)  # Zero payload

        with self.assertRaisesRegex(ValueError, "Unsupported message type"):
            self.handler.parse_message(invalid_msg)


class TestFullClientRequestFields(unittest.TestCase):
    def setUp(self):
        self.config = {
            "user": {
                "uid": "test_user_123",
                "did": "device_abc",
                "platform": "iOS",
                "sdk_version": "1.2.3",
                "app_version": "4.5.6",
            },
            "audio": {
                "format": "wav",
                "codec": "opus",
                "rate": 44100,
                "bits": 24,
                "channel": 2,
            },
            "request": {
                "model_name": "bigmodel",
                "enable_itn": True,
                "enable_punc": True,
                "enable_ddc": True,
                "show_utterances": True,
                "sensitive_words_filter": "custom_filter",
                "result_type": "single",
                "corpus": {
                    "boosting_table_name": "hotwords_table",
                    "correct_table_name": "replace_table",
                    "context": '{"hotwords": [{"word": "订一家"}]}',
                },
            },
        }
        self.header = Header(
            MSG_FULL_CLIENT_REQUEST, FLAG_NORMAL, SERIALIZATION_JSON, COMPRESSION_NONE
        )
        self.request = FullClientRequest(self.header, self.config)

    def test_user_fields(self):
        self.assertEqual(self.request.user_uid, "test_user_123")
        self.assertEqual(self.request.user_did, "device_abc")
        self.assertEqual(self.request.user_platform, "iOS")
        self.assertEqual(self.request.user_sdk_version, "1.2.3")
        self.assertEqual(self.request.user_app_version, "4.5.6")

    def test_audio_fields(self):
        self.assertEqual(self.request.audio_format, "wav")
        self.assertEqual(self.request.audio_codec, "opus")
        self.assertEqual(self.request.audio_rate, 44100)
        self.assertEqual(self.request.audio_bits, 24)
        self.assertEqual(self.request.audio_channel, 2)

    def test_request_fields(self):
        self.assertEqual(self.request.request_model_name, "bigmodel")
        self.assertTrue(self.request.request_enable_itn)
        self.assertTrue(self.request.request_enable_punc)
        self.assertTrue(self.request.request_enable_ddc)
        self.assertTrue(self.request.request_show_utterances)
        self.assertEqual(self.request.request_sensitive_words_filter, "custom_filter")
        self.assertEqual(self.request.request_result_type, "single")

    def test_corpus_fields(self):
        self.assertEqual(self.request.request_boosting_table_name, "hotwords_table")
        self.assertEqual(self.request.request_correct_table_name, "replace_table")
        self.assertEqual(
            self.request.request_context, '{"hotwords": [{"word": "订一家"}]}'
        )

    def test_default_values(self):
        # Test with minimal configuration
        minimal_config = {
            "audio": {"format": "raw"},
            "request": {"model_name": "bigmodel"},
        }
        minimal_request = FullClientRequest(self.header, minimal_config)

        # User fields should be None
        self.assertIsNone(minimal_request.user_uid)

        # Audio fields should have defaults
        self.assertEqual(minimal_request.audio_codec, "raw")
        self.assertEqual(minimal_request.audio_rate, 16000)
        self.assertEqual(minimal_request.audio_bits, 16)
        self.assertEqual(minimal_request.audio_channel, 1)

        # Request fields should have defaults
        self.assertFalse(minimal_request.request_enable_itn)
        self.assertFalse(minimal_request.request_enable_punc)
        self.assertFalse(minimal_request.request_enable_ddc)
        self.assertEqual(minimal_request.request_result_type, "full")

        # Corpus fields should be None
        self.assertIsNone(minimal_request.request_boosting_table_name)

    def test_corpus_handling(self):
        # Test with different corpus structure
        alt_config = {
            "audio": {"format": "mp3"},
            "request": {
                "model_name": "bigmodel",
                "corpus": {"context": "simple context"},
            },
        }
        alt_request = FullClientRequest(self.header, alt_config)

        self.assertEqual(alt_request.request_context, "simple context")
        self.assertIsNone(alt_request.request_boosting_table_name)
        self.assertIsNone(alt_request.request_correct_table_name)

    def test_missing_sections(self):
        # Test with completely empty payload
        empty_request = FullClientRequest(self.header, {})

        # Should all return None or defaults
        self.assertIsNone(empty_request.user_uid)
        self.assertIsNone(empty_request.audio_format)
        self.assertIsNone(empty_request.request_model_name)
        self.assertEqual(empty_request.request_result_type, "full")


class TestFullServerResponseFields(unittest.TestCase):
    def setUp(self):
        self.response_data = {
            "audio_info": {"duration": 3696},
            "result": {
                "text": "这是字节跳动，今日头条母公司。",
                "confidence": 95,
                "utterances": [
                    {
                        "definite": True,
                        "end_time": 1705,
                        "start_time": 0,
                        "text": "这是字节跳动，",
                        "words": [
                            {
                                "blank_duration": 0,
                                "end_time": 860,
                                "start_time": 740,
                                "text": "这",
                            },
                            {
                                "blank_duration": 0,
                                "end_time": 1020,
                                "start_time": 860,
                                "text": "是",
                            },
                        ],
                    },
                    {
                        "definite": True,
                        "end_time": 3696,
                        "start_time": 2110,
                        "text": "今日头条母公司。",
                        "words": [
                            {
                                "blank_duration": 0,
                                "end_time": 3070,
                                "start_time": 2910,
                                "text": "今",
                            },
                            {
                                "blank_duration": 0,
                                "end_time": 3230,
                                "start_time": 3070,
                                "text": "日",
                            },
                        ],
                    },
                ],
            },
        }
        header = Header(
            MSG_FULL_SERVER_RESPONSE, FLAG_NORMAL, SERIALIZATION_JSON, COMPRESSION_NONE
        )
        self.response = FullServerResponse(header, 1, self.response_data)

    def test_top_level_fields(self):
        self.assertEqual(self.response.audio_duration, 3696)
        self.assertEqual(self.response.full_text, "这是字节跳动，今日头条母公司。")
        self.assertEqual(self.response.confidence, 95)
        self.assertEqual(self.response.utterance_count, 2)

    def test_utterance_fields(self):
        # First utterance
        self.assertEqual(self.response.get_utterance_text(0), "这是字节跳动，")
        self.assertEqual(self.response.get_utterance_start(0), 0)
        self.assertEqual(self.response.get_utterance_end(0), 1705)
        self.assertTrue(self.response.is_utterance_definite(0))
        self.assertEqual(self.response.word_count(0), 2)

        # Second utterance
        self.assertEqual(self.response.get_utterance_text(1), "今日头条母公司。")
        self.assertEqual(self.response.get_utterance_start(1), 2110)
        self.assertEqual(self.response.get_utterance_end(1), 3696)
        self.assertTrue(self.response.is_utterance_definite(1))
        self.assertEqual(self.response.word_count(1), 2)

        # Invalid index
        self.assertIsNone(self.response.get_utterance_text(2))

    def test_word_fields(self):
        # First word of first utterance
        self.assertEqual(self.response.get_word_text(0, 0), "这")
        self.assertEqual(self.response.get_word_start(0, 0), 740)
        self.assertEqual(self.response.get_word_end(0, 0), 860)
        self.assertEqual(self.response.get_word_blank_duration(0, 0), 0)

        # Second word of second utterance
        self.assertEqual(self.response.get_word_text(1, 1), "日")
        self.assertEqual(self.response.get_word_start(1, 1), 3070)
        self.assertEqual(self.response.get_word_end(1, 1), 3230)
        self.assertEqual(self.response.get_word_blank_duration(1, 1), 0)

        # Invalid indices
        self.assertIsNone(self.response.get_word_text(0, 2))
        self.assertIsNone(self.response.get_word_text(2, 0))

    def test_partial_response(self):
        # Test with minimal response data
        minimal_data = {"result": {"text": "Hello world"}}
        header = Header(
            MSG_FULL_SERVER_RESPONSE, FLAG_NORMAL, SERIALIZATION_JSON, COMPRESSION_NONE
        )
        minimal_response = FullServerResponse(header, 1, minimal_data)

        # Top-level fields
        self.assertIsNone(minimal_response.audio_duration)
        self.assertEqual(minimal_response.full_text, "Hello world")
        self.assertIsNone(minimal_response.confidence)
        self.assertEqual(minimal_response.utterance_count, 0)

        # Utterance/word access
        self.assertIsNone(minimal_response.get_utterance_text(0))
        self.assertEqual(minimal_response.word_count(0), 0)

    def test_empty_response(self):
        # Test with empty response
        empty_response = FullServerResponse(None, 0, {})

        self.assertIsNone(empty_response.audio_duration)
        self.assertIsNone(empty_response.full_text)
        self.assertIsNone(empty_response.confidence)
        self.assertEqual(empty_response.utterance_count, 0)


class TestErrorResponse(unittest.TestCase):
    def setUp(self):
        self.header = Header(
            MSG_SERVER_ERROR, FLAG_NORMAL, SERIALIZATION_NONE, COMPRESSION_NONE
        )

    def test_error_serialization(self):
        # Create error with English message
        error = ErrorResponse(self.header, ERR_INVALID_PARAM, "Invalid parameter")

        handler = SAILProtocolHandler()
        # Serialize the error response
        serialized = handler.serialize_message(error)

        payload = serialized[4:]  # Skip header (first 4 bytes)

        # Verify structure: [error_code (4B)] [msg_size (4B)] [message]
        self.assertEqual(len(error.pack_payload()), len("Invalid parameter"))

        # Verify fields
        code, size = struct.unpack("!II", payload[:8])
        self.assertEqual(code, ERR_INVALID_PARAM)
        self.assertEqual(size, len("Invalid parameter"))
        self.assertEqual(payload[8:].decode("utf-8"), "Invalid parameter")

        # Test with Chinese message
        chinese_error = ErrorResponse(self.header, ERR_AUDIO_FORMAT, "音频格式不正确")

        serialized = handler.serialize_message(chinese_error)
        payload = serialized[4:]  # Skip header (first 4 bytes)
        code, size = struct.unpack("!II", payload[:8])
        self.assertEqual(code, ERR_AUDIO_FORMAT)
        self.assertEqual(payload[8:].decode("utf-8"), "音频格式不正确")

    def test_payload_size(self):
        error = ErrorResponse(self.header, ERR_SERVER_BUSY, "Server busy")
        self.assertEqual(error.get_payload_size(), 8 + len("Server busy"))

        long_error = ErrorResponse(self.header, 12345, "a" * 1000)
        self.assertEqual(long_error.get_payload_size(), 8 + 1000)

    def test_parse_payload(self):
        # Create test payload
        error_code = ERR_TIMEOUT
        message = "Request timed out"
        payload = struct.pack("!II", error_code, len(message)) + message.encode("utf-8")

        # Parse
        parsed_code, parsed_msg = ErrorResponse.parse_payload(payload)
        self.assertEqual(parsed_code, error_code)
        self.assertEqual(parsed_msg, message)

    def test_parse_errors(self):
        # Test short payload
        with self.assertRaises(ValueError):
            ErrorResponse.parse_payload(b"\x00\x00")

        # Test size mismatch
        payload = struct.pack("!II", 123, 10) + b"short"
        with self.assertRaises(ValueError):
            ErrorResponse.parse_payload(payload)

    def test_handler_parsing(self):
        # Create complete error message
        header = Header(
            MSG_SERVER_ERROR, FLAG_NORMAL, SERIALIZATION_NONE, COMPRESSION_NONE
        )
        error = ErrorResponse(header, ERR_EMPTY_AUDIO, "No audio data")

        # Serialize full message
        handler = SAILProtocolHandler()
        full_data = handler.serialize_message(error)

        # Parse through handler
        parsed = handler.parse_message(full_data)
        self.assertIsInstance(parsed, ErrorResponse)
        self.assertEqual(parsed.error_code, ERR_EMPTY_AUDIO)
        self.assertEqual(parsed.error_message, "No audio data")

        # Test invalid error message
        invalid_data = header.pack() + b"invalid_payload"
        parsed = handler.parse_message(invalid_data)
        self.assertIsInstance(parsed, ErrorResponse)
        self.assertEqual(parsed.error_code, ERR_INVALID_PARAM)
        self.assertIn("Error parsing", parsed.error_message)


if __name__ == "__main__":
    unittest.main()
