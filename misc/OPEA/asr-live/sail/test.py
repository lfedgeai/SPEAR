#!/usr/bin/env python3
import unittest
import struct
import gzip
import json
from sail.proto import (
    Header, BaseMessage, FullClientRequest, AudioOnlyRequest,
    FullServerResponse, ErrorResponse, SAILProtocolHandler,
    PROTOCOL_VERSION, HEADER_SIZE_VALUE,
    MSG_FULL_CLIENT_REQUEST, MSG_AUDIO_ONLY_REQUEST,
    MSG_FULL_SERVER_RESPONSE, MSG_ERROR_RESPONSE,
    SERIALIZATION_NONE, SERIALIZATION_JSON,
    COMPRESSION_NONE, COMPRESSION_GZIP,
    FLAG_NORMAL, FLAG_LAST_AUDIO, FLAG_LAST_RESPONSE,
    ERR_SUCCESS, ERR_INVALID_PARAM
)


class TestHeader(unittest.TestCase):
    def test_header_packing(self):
        header = Header(MSG_FULL_CLIENT_REQUEST, FLAG_NORMAL,
                        SERIALIZATION_JSON, COMPRESSION_GZIP)
        packed = header.pack()

        # Verify length
        self.assertEqual(len(packed), 4)

        # Unpack manually
        byte0, byte1, byte2, byte3 = struct.unpack('!4B', packed)

        # Verify fields
        self.assertEqual(byte0, (PROTOCOL_VERSION << 4) | HEADER_SIZE_VALUE)
        self.assertEqual(byte1, (MSG_FULL_CLIENT_REQUEST << 4) | FLAG_NORMAL)
        self.assertEqual(byte2, (SERIALIZATION_JSON << 4) | COMPRESSION_GZIP)
        self.assertEqual(byte3, 0)

    def test_header_unpacking(self):
        # Create binary header data
        data = struct.pack('!4B',
                           (PROTOCOL_VERSION << 4) | HEADER_SIZE_VALUE,
                           (MSG_AUDIO_ONLY_REQUEST << 4) | FLAG_LAST_AUDIO,
                           (SERIALIZATION_NONE << 4) | COMPRESSION_NONE,
                           0)

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
            Header.unpack(b'\x01')

        # Test invalid version
        invalid_ver = struct.pack('!4B', (2 << 4) | 1, 0, 0, 0)
        with self.assertRaisesRegex(ValueError, "Unsupported protocol version"):
            Header.unpack(invalid_ver)

        # Test invalid header size
        invalid_size = struct.pack('!4B', (1 << 4) | 2, 0, 0, 0)
        with self.assertRaisesRegex(ValueError, "Unsupported header size"):
            Header.unpack(invalid_size)


class TestFullClientRequest(unittest.TestCase):
    def setUp(self):
        self.config = {
            "user": {"uid": "test_user"},
            "audio": {"format": "wav", "rate": 16000},
            "request": {"model_name": "bigmodel"}
        }
        self.header = Header(MSG_FULL_CLIENT_REQUEST, FLAG_NORMAL,
                             SERIALIZATION_JSON, COMPRESSION_NONE)

    def test_json_serialization(self):
        request = FullClientRequest(self.header, self.config)
        payload = request.pack_payload()

        # Should be JSON string without compression
        decoded = json.loads(payload.decode('utf-8'))
        self.assertEqual(decoded, self.config)

    def test_gzip_compression(self):
        # Create header with compression
        comp_header = Header(MSG_FULL_CLIENT_REQUEST, FLAG_NORMAL,
                             SERIALIZATION_JSON, COMPRESSION_GZIP)
        request = FullClientRequest(comp_header, self.config)
        payload = request.pack_payload()

        # Decompress and decode
        decompressed = gzip.decompress(payload)
        decoded = json.loads(decompressed.decode('utf-8'))
        self.assertEqual(decoded, self.config)

    def test_parse_payload(self):
        # Create JSON payload
        payload_data = json.dumps(self.config).encode('utf-8')

        # Parse without compression
        parsed = FullClientRequest.parse_payload(
            payload_data,
            SERIALIZATION_JSON,
            COMPRESSION_NONE
        )
        self.assertEqual(parsed, self.config)

        # Parse with compression
        compressed = gzip.compress(payload_data)
        parsed = FullClientRequest.parse_payload(
            compressed,
            SERIALIZATION_JSON,
            COMPRESSION_GZIP
        )
        self.assertEqual(parsed, self.config)


class TestAudioOnlyRequest(unittest.TestCase):
    def setUp(self):
        self.audio_data = b'\x00\x01\x02\x03' * 100  # 400 bytes of fake audio
        self.header = Header(MSG_AUDIO_ONLY_REQUEST, FLAG_LAST_AUDIO,
                             SERIALIZATION_NONE, COMPRESSION_NONE)

    def test_pack_unpack(self):
        request = AudioOnlyRequest(self.header, self.audio_data)

        # Should return same data without compression
        packed = request.pack_payload()
        self.assertEqual(packed, self.audio_data)

        # Parse should return original data
        parsed = AudioOnlyRequest.parse_payload(
            packed,
            COMPRESSION_NONE
        )
        self.assertEqual(parsed, self.audio_data)

    def test_gzip_compression(self):
        comp_header = Header(MSG_AUDIO_ONLY_REQUEST, FLAG_NORMAL,
                             SERIALIZATION_NONE, COMPRESSION_GZIP)
        request = AudioOnlyRequest(comp_header, self.audio_data)
        packed = request.pack_payload()

        # Should be compressed
        self.assertLess(len(packed), len(self.audio_data))

        # Parse should decompress to original
        parsed = AudioOnlyRequest.parse_payload(
            packed,
            COMPRESSION_GZIP
        )
        self.assertEqual(parsed, self.audio_data)

    def test_is_last_flag(self):
        # With last audio flag
        last_header = Header(MSG_AUDIO_ONLY_REQUEST, FLAG_LAST_AUDIO,
                             SERIALIZATION_NONE, COMPRESSION_NONE)
        last_request = AudioOnlyRequest(last_header, b'')
        self.assertTrue(last_request.is_last)

        # Without last audio flag
        normal_header = Header(MSG_AUDIO_ONLY_REQUEST, FLAG_NORMAL,
                               SERIALIZATION_NONE, COMPRESSION_NONE)
        normal_request = AudioOnlyRequest(normal_header, b'')
        self.assertFalse(normal_request.is_last)


class TestServerResponses(unittest.TestCase):
    def test_full_server_response(self):
        response_data = {
            "result": {"text": "Hello world", "confidence": 95}
        }
        header = Header(MSG_FULL_SERVER_RESPONSE, FLAG_LAST_RESPONSE,
                        SERIALIZATION_JSON, COMPRESSION_GZIP)
        response = FullServerResponse(header, 123, response_data)

        # Test payload packing
        payload = response.pack_payload()
        decompressed = gzip.decompress(payload)
        decoded = json.loads(decompressed.decode('utf-8'))
        self.assertEqual(decoded, response_data)

        # Test payload size calculation
        self.assertEqual(response.get_payload_size(), len(payload))

    def test_error_response(self):
        error_data = {"message": "Invalid parameter"}
        header = Header(MSG_ERROR_RESPONSE, FLAG_NORMAL,
                        SERIALIZATION_JSON, COMPRESSION_NONE)
        response = ErrorResponse(header, ERR_INVALID_PARAM, error_data)

        # Test payload packing
        payload = response.pack_payload()
        decoded = json.loads(payload.decode('utf-8'))
        self.assertEqual(decoded, error_data)

        # Test payload size
        self.assertEqual(response.get_payload_size(), len(payload))


class TestProtocolHandler(unittest.TestCase):
    def setUp(self):
        self.handler = SAILProtocolHandler()
        self.config = {
            "user": {"uid": "test_user"},
            "audio": {"format": "wav", "rate": 16000},
            "request": {"model_name": "bigmodel"}
        }

    def create_client_request(self, serialization=SERIALIZATION_JSON,
                              compression=COMPRESSION_NONE):
        """Helper to create full client request binary"""
        header = Header(MSG_FULL_CLIENT_REQUEST, FLAG_NORMAL,
                        serialization, compression)
        request = FullClientRequest(header, self.config)

        # Build binary message: header + payload_size + payload
        payload = request.pack_payload()
        payload_size = struct.pack('!I', len(payload))
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
        audio_data = b'\x01\x02\x03' * 100
        header = Header(MSG_AUDIO_ONLY_REQUEST, FLAG_NORMAL,
                        SERIALIZATION_NONE, COMPRESSION_GZIP)
        payload = gzip.compress(audio_data)
        payload_size = struct.pack('!I', len(payload))
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
        header = Header(MSG_FULL_SERVER_RESPONSE, FLAG_NORMAL,
                        SERIALIZATION_JSON, COMPRESSION_NONE)
        response = FullServerResponse(header, 1, {"text": "Hello"})
        serialized = self.handler.serialize_message(response)

        # Should have: header (4B) + sequence (4B) + payload_size (4B) + payload
        self.assertEqual(len(serialized), 4 + 4 + 4 +
                         len(json.dumps({"text": "Hello"}).encode()))

        # Test serialization of ErrorResponse
        error_header = Header(MSG_ERROR_RESPONSE, FLAG_NORMAL,
                              SERIALIZATION_JSON, COMPRESSION_NONE)
        error_resp = ErrorResponse(error_header, ERR_INVALID_PARAM,
                                   {"message": "Error"})
        serialized = self.handler.serialize_message(error_resp)
        # Header + error code + payload size
        self.assertGreater(len(serialized), 12)

    def test_unknown_message_type(self):
        # Create invalid message type
        invalid_header = struct.pack('!4B',
                                     (PROTOCOL_VERSION << 4) | HEADER_SIZE_VALUE,
                                     (0b0100 << 4) | 0,  # Invalid type
                                     0, 0)
        invalid_msg = invalid_header + struct.pack('!I', 0)  # Zero payload

        with self.assertRaisesRegex(ValueError, "Unsupported message type"):
            self.handler.parse_message(invalid_msg)


if __name__ == '__main__':
    unittest.main()
