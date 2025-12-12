from abc import ABC, abstractmethod

import spear.client as client
from spear.stream.stream_ctrl import close_stream, create_stream

from spear.proto.stream import OperationType


def send_operation_event(
    stream_id, function_name, operation_type, data, last_event=False
):
    """
    Send an operation event to the specified stream.

    Args:
        stream_id (int): The ID of the stream.
        function_name (str): The name of the function to call.
        operation_type (OperationType): The type of operation to perform.
        data (bytes): The data to send with the operation event.
    """
    client.global_agent().send_operation_event(
        stream_id, function_name, operation_type, data, last_event
    )


def send_notification_event(
    stream_id, function_name, notification_event_type, data, last_event=False
):
    """
    Send a notification event to the specified stream.

    Args:
        stream_id (int): The ID of the stream.
        function_name (str): The name of the function to call.
        notification_event_type (NotificationEventType): The type of notification event.
        data (bytes): The data to send with the notification event.
    """
    client.global_agent().send_notification_event(
        stream_id, function_name, notification_event_type, data, last_event
    )


def send_rawdata_event(stream_id, data, last_event=False):
    """
    Send raw data to the specified stream.
    Args:
        stream_id (int): The ID of the stream.
        data (bytes): The raw data to send.
    """
    client.global_agent().send_rawdata_event(stream_id, data, last_event)


class AbstractStreamHandler(ABC):
    """
    Base class for handling streams.
    This class can be extended to implement specific stream handling logic.
    """

    def __init__(self):
        """
        Initialize the stream handler.
        This method sets up the initial state of the stream handler.
        """
        self._stream_id = None
        self._stream_cls = None

    def open_stream(self, stream_cls: str):
        """
        Open a stream with the specified class.
        This method can be used to create a new stream for handling requests.

        Args:
            stream_cls (str): The class of the stream to open.
        """
        self._stream_cls = stream_cls
        self._stream_id = create_stream(stream_cls, self.handle_stream)

    def close_stream(self):
        """
        Close the stream.
        This method can be used to close the stream when it is no longer needed.
        """
        if self._stream_id is not None:
            close_stream(self._stream_id)
            self._stream_id = None
            self._stream_cls = None

    @property
    def stream_id(self):
        """
        Get the stream ID.
        This property returns the ID of the stream created for handling requests.
        """
        return self._stream_id

    @property
    def stream_cls(self):
        """
        Get the stream class.
        This property returns the class of the stream being handled.
        """
        return self._stream_cls

    @abstractmethod
    def operation(self, ctx: client.OperationStreamRequestContext):
        """
        Handle operation events from the stream.
        This method should be implemented by subclasses to define how to handle operations.
        """
        raise NotImplementedError("Subclasses should implement this method")

    @abstractmethod
    def notification(self, ctx: client.NotificationStreamRequestContext):
        """
        Handle notification events from the stream.
        This method should be implemented by subclasses to define how to handle notifications.
        """
        raise NotImplementedError("Subclasses should implement this method")

    @abstractmethod
    def raw(self, ctx: client.RawStreamRequestContext):
        """
        Handle raw data events from the stream.
        This method should be implemented by subclasses to define how to handle raw data.
        """
        raise NotImplementedError("Subclasses should implement this method")

    def send_operation_event(
        self, operation_type: OperationType, data: bytes, last_event: bool = False
    ):
        """
        Send an operation event to the stream.
        This method can be used to send operation events to the stream being handled.

        Args:
            operation_type (OperationType): The type of operation to perform.
            data (bytes): The data to send with the operation event.
            last_event (bool): Whether this is the last event in the operation.
        """
        send_operation_event(
            self._stream_id, self._stream_cls, operation_type, data, last_event
        )

    def handle_stream(self, ctx: client.RawStreamRequestContext):
        """
        Handle stream requests.
        """
        assert isinstance(
            ctx, client.RawStreamRequestContext
        ), "Expected RawStreamRequestContext for real-time ASR handling"
        # logger.info("Handling real-time ASR request: %s", ctx)
        if ctx.is_operation_event:
            assert isinstance(
                ctx, client.OperationStreamRequestContext
            ), "Expected OperationStreamRequestContext for operation handling"
            self.operation(ctx)
        elif ctx.is_notification_event:
            assert isinstance(
                ctx, client.NotificationStreamRequestContext
            ), "Expected NotificationStreamRequestContext for notification handling"
            self.notification(ctx)
        elif ctx.is_raw:
            assert isinstance(
                ctx, client.RawStreamRequestContext
            ), "Expected RawStreamRequestContext for raw data handling"
            self.raw(ctx)
