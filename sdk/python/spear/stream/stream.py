import spear.client as client


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
