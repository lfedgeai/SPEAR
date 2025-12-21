# Realtime/Streaming Subsystem Design Notes

Realtime voice/streaming differs from standard HTTP requests: it requires dedicated transport (WebSocket/gRPC), session lifecycle, and event-driven incremental outputs.

## 1. Why it needs a separate model

The legacy Go realtime ASR uses a WebSocket session and appends audio buffers; delta events are forwarded back to the task (`legacy/spearlet/stream/rt_asr.go:46-140`). This cannot be represented as a single request/response.

Therefore model realtime explicitly as:

- `Operation::realtime_voice`
- `Feature::supports_bidi_stream`
- `Transport::websocket|grpc`

Implement it as a dedicated stream subsystem, while sharing the same registry/capabilities and routing constraints.

## 2. Suggested lifecycle

At minimum:

- `create_session`
- `append_audio` (or `send_input_event`)
- `commit` (end of an input turn)
- `close_session`

Outputs should be an event stream:

- `delta`
- `completed`
- `error`

## 3. Routing and capability constraints

Routing must require:

- the backend instance supports `realtime_voice`
- transport supports `websocket|grpc`
- session concurrency and duration constraints (`max_session_seconds`)

If no candidate exists:

- return `MissingCapabilities`
- if allowed, downgrade to `speech_to_text` (non-realtime) or reject

## 4. Relationship to hostcalls

Two common shapes:

- reuse a “stream ctrl” hostcall style (closer to legacy `MethodStreamCtrl`)
- design a dedicated `rt_*` hostcall family

In both cases, secrets and URLs remain host-managed.
