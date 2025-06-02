#!/usr/bin/env python3
import gzip
import json
import struct

# Constants for protocol version
PROTOCOL_VERSION = 1
HEADER_SIZE_VALUE = 1  # Header size in units of 4 bytes (1*4=4 bytes)

# Message types
MSG_FULL_CLIENT_REQUEST = 0b0001
MSG_AUDIO_ONLY_REQUEST = 0b0010
MSG_FULL_SERVER_RESPONSE = 0b1001
MSG_ERROR_RESPONSE = 0b1111

# Message type specific flags
FLAG_NORMAL = 0b0000
FLAG_LAST_AUDIO = 0b0010
FLAG_LAST_RESPONSE = 0b0011

# Serialization methods
SERIALIZATION_NONE = 0b0000
SERIALIZATION_JSON = 0b0001

# Compression methods
COMPRESSION_NONE = 0b0000
COMPRESSION_GZIP = 0b0001

# Error codes (partial list)
ERR_SUCCESS = 20000000
ERR_INVALID_PARAM = 45000001
ERR_EMPTY_AUDIO = 45000002
ERR_TIMEOUT = 45000081
ERR_AUDIO_FORMAT = 45000151
ERR_SERVER_BUSY = 55000031


class Header:
    """Represents the 4-byte header of SAIL ASR protocol messages"""

    def __init__(self, message_type, flags, serialization, compression):
        self.version = PROTOCOL_VERSION
        self.header_size = HEADER_SIZE_VALUE
        self.message_type = message_type
        self.flags = flags
        self.serialization = serialization
        self.compression = compression

    def pack(self):
        """Serialize header into 4-byte binary format"""
        byte0 = (self.version << 4) | self.header_size
        byte1 = (self.message_type << 4) | self.flags
        byte2 = (self.serialization << 4) | self.compression
        byte3 = 0  # Reserved byte
        return struct.pack('!4B', byte0, byte1, byte2, byte3)

    @classmethod
    def unpack(cls, data):
        """Parse 4-byte binary data into Header object"""
        if len(data) < 4:
            raise ValueError("Header data too short")

        byte0, byte1, byte2, byte3 = struct.unpack('!4B', data[:4])
        version = (byte0 >> 4) & 0x0F
        header_size = byte0 & 0x0F

        if version != PROTOCOL_VERSION:
            raise ValueError(f"Unsupported protocol version: {version}")
        if header_size != HEADER_SIZE_VALUE:
            raise ValueError(f"Unsupported header size: {header_size}")

        message_type = (byte1 >> 4) & 0x0F
        flags = byte1 & 0x0F
        serialization = (byte2 >> 4) & 0x0F
        compression = byte2 & 0x0F

        return cls(message_type, flags, serialization, compression)


class BaseMessage:
    """Base class for all SAIL ASR messages"""

    def __init__(self, header):
        self.header = header

    def get_payload_size(self):
        """Calculate payload size for serialization (to be implemented by subclasses)"""
        raise NotImplementedError

    def pack_payload(self):
        """Serialize payload (to be implemented by subclasses)"""
        raise NotImplementedError


class FullClientRequest(BaseMessage):
    """Represents initial client request with configuration parameters"""

    def __init__(self, header, payload_data):
        super().__init__(header)
        self.payload_data = payload_data

    @classmethod
    def parse_payload(cls, payload_bytes, serialization, compression):
        """Parse payload from binary data based on serialization and compression"""
        # Decompress if needed
        if compression == COMPRESSION_GZIP:
            payload_bytes = gzip.decompress(payload_bytes)

        # Deserialize based on format
        if serialization == SERIALIZATION_JSON:
            return json.loads(payload_bytes.decode('utf-8'))
        else:
            # For unsupported serialization, return raw bytes
            return payload_bytes

    def get_payload_size(self):
        """Calculate payload size for serialization"""
        payload = self.pack_payload()
        return len(payload)

    def pack_payload(self):
        """Serialize payload into binary format"""
        # Serialize based on format
        if self.header.serialization == SERIALIZATION_JSON:
            payload_bytes = json.dumps(self.payload_data).encode('utf-8')
        else:
            payload_bytes = self.payload_data  # Assume already bytes

        # Compress if needed
        if self.header.compression == COMPRESSION_GZIP:
            payload_bytes = gzip.compress(payload_bytes)

        return payload_bytes


class AudioOnlyRequest(BaseMessage):
    """Represents audio data packets from client"""

    def __init__(self, header, audio_data):
        super().__init__(header)
        self.audio_data = audio_data
        self.is_last = (header.flags == FLAG_LAST_AUDIO)

    @classmethod
    def parse_payload(cls, payload_bytes, compression):
        """Parse audio payload from binary data"""
        # Decompress if needed
        if compression == COMPRESSION_GZIP:
            return gzip.decompress(payload_bytes)
        return payload_bytes

    def get_payload_size(self):
        return len(self.audio_data)

    def pack_payload(self):
        """Serialize audio payload"""
        # Compress if needed
        if self.header.compression == COMPRESSION_GZIP:
            return gzip.compress(self.audio_data)
        return self.audio_data


class FullServerResponse(BaseMessage):
    """Represents server responses with recognition results"""

    def __init__(self, header, sequence, response_data):
        super().__init__(header)
        self.sequence = sequence
        self.response_data = response_data

    def get_payload_size(self):
        payload = self.pack_payload()
        return len(payload)

    def pack_payload(self):
        """Serialize response payload into binary format"""
        # Serialize based on format
        if self.header.serialization == SERIALIZATION_JSON:
            payload_bytes = json.dumps(self.response_data).encode('utf-8')
        else:
            payload_bytes = self.response_data  # Assume already bytes

        # Compress if needed
        if self.header.compression == COMPRESSION_GZIP:
            payload_bytes = gzip.compress(payload_bytes)

        return payload_bytes


class ErrorResponse(BaseMessage):
    """Represents error responses from server"""

    def __init__(self, header, error_code, error_data):
        super().__init__(header)
        self.error_code = error_code
        self.error_data = error_data

    def get_payload_size(self):
        payload = self.pack_payload()
        return len(payload)

    def pack_payload(self):
        """Serialize error payload into binary format"""
        # Serialize based on format
        if self.header.serialization == SERIALIZATION_JSON:
            return json.dumps(self.error_data).encode('utf-8')
        return self.error_data  # Assume already bytes


class SAILProtocolHandler:
    """Main class for handling SAIL ASR protocol parsing and serialization"""

    def __init__(self):
        self.serialization = SERIALIZATION_JSON
        self.compression = COMPRESSION_NONE

    def parse_message(self, data):
        """Parse incoming binary data into appropriate message object"""
        # Parse header (first 4 bytes)
        header = Header.unpack(data[:4])
        payload_bytes = data[4+4:]  # Skip header and payload size

        # Handle different message types
        if header.message_type == MSG_FULL_CLIENT_REQUEST:
            # Remember serialization and compression for responses
            self.serialization = header.serialization
            self.compression = header.compression

            # Parse payload
            payload = FullClientRequest.parse_payload(
                payload_bytes,
                header.serialization,
                header.compression
            )
            return FullClientRequest(header, payload)

        elif header.message_type == MSG_AUDIO_ONLY_REQUEST:
            # Parse audio payload
            audio_data = AudioOnlyRequest.parse_payload(
                payload_bytes,
                header.compression
            )
            return AudioOnlyRequest(header, audio_data)

        else:
            raise ValueError(
                f"Unsupported message type: {header.message_type}")

    def create_full_response(self, sequence, result_data, is_last=False):
        """Create FullServerResponse message"""
        flags = FLAG_LAST_RESPONSE if is_last else FLAG_NORMAL
        header = Header(
            MSG_FULL_SERVER_RESPONSE,
            flags,
            self.serialization,
            self.compression
        )
        return FullServerResponse(header, sequence, result_data)

    def create_error_response(self, error_code, error_message):
        """Create ErrorResponse message"""
        header = Header(
            MSG_ERROR_RESPONSE,
            FLAG_NORMAL,
            SERIALIZATION_JSON,
            COMPRESSION_NONE
        )
        error_data = {
            "code": error_code,
            "message": error_message
        }
        return ErrorResponse(header, error_code, error_data)

    def serialize_message(self, message):
        """Convert message object to binary data for sending"""
        # Pack header
        data = message.header.pack()

        # Handle different message types
        if isinstance(message, (FullClientRequest, AudioOnlyRequest)):
            # Pack payload size and payload
            payload = message.pack_payload()
            payload_size = len(payload)
            data += struct.pack('!I', payload_size)
            data += payload

        elif isinstance(message, FullServerResponse):
            # Pack sequence number, payload size, and payload
            data += struct.pack('!I', message.sequence)
            payload = message.pack_payload()
            payload_size = len(payload)
            data += struct.pack('!I', payload_size)
            data += payload

        elif isinstance(message, ErrorResponse):
            # Pack error code, payload size, and payload
            data += struct.pack('!I', message.error_code)
            payload = message.pack_payload()
            payload_size = len(payload)
            data += struct.pack('!I', payload_size)
            data += payload

        else:
            raise TypeError("Unsupported message type for serialization")

        return data
