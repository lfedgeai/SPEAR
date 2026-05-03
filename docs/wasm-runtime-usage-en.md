# WASM Runtime Usage

## Overview
The Spearlet WASM runtime requires a valid WASM binary at instance creation. If the module bytes are invalid (e.g. missing the WASM magic header), the runtime returns `InvalidConfiguration` and refuses to create the instance.

## Lifecycle Notes
- `create_instance`: downloads module bytes, validates the WASM magic header, loads the module, and creates the instance handle
- `start_instance`: moves the instance into a serving state; when built with `wasmedge`, starts the WASM worker
- `stop_instance`: sends a Stop control request and waits for an ack to ensure the worker exits

## Instantiation Requirements
- Instance configuration must resolve module bytes by downloading from `InstanceConfig.artifact.location` (derived from the task `executable.uri` during artifact/task materialization).
- Module bytes must start with the WASM magic number `00 61 73 6d` (`\0asm`).
- Empty or invalid content causes `create_instance` to fail.

## Task Registration Integration
- Provide executable descriptor in task registration:
```json
{
  "name": "hello-wasm",
  "priority": "normal",
  "endpoint": "",
  "version": "1.0.0",
  "capabilities": ["wasm"],
  "metadata": {},
  "config": {},
  "executable": {
    "type": "wasm",
    "uri": "smsfile://<file_id>",
    "name": "hello.wasm",
    "args": [],
    "env": {}
  }
}
```
- The runtime downloads content from the artifact `location` during `create_instance`, validates the WASM magic header before loading, and then creates the WASM instance handle.
- When built with the `wasmedge` feature enabled, `start_instance` starts a WASM worker that receives and executes subsequent invocation requests.

### SMS File Scheme and Config Source

- Supported `smsfile` forms:
  - Explicit override: `smsfile://<host:port>/<file_id>`
  - Short form: `smsfile://<file_id>` (runtime uses `SpearletConfig.sms_http_addr` for the HTTP gateway)
- The runtime constructs path `"/api/v1/files/<file_id>"`.
- Download function:

```rust
pub async fn fetch_sms_file(sms_http_addr: &str, path: &str) -> ExecutionResult<Vec<u8>>
```

* Configuration is injected at runtime initialization: FunctionService passes the full `SpearletConfig` into each `Runtime` via `RuntimeConfig.spearlet_config`; `sms_http_addr` provides the HTTP gateway address for downloads.

## Error Behavior
- Invalid WASM bytes: instance creation fails at validation stage to avoid late execution errors.
- Download/validation failures: detailed error recorded, returning `RuntimeError` or `InvalidConfiguration`.

## Best Practices
- Produce valid WASM during build:
  - C: `zig cc -target wasm32-wasi`
  - Rust: `cargo build --release --target wasm32-wasip1`
- When uploading via SMS file service, provide `checksum_sha256` if possible.
- In integration tests, explicitly provide valid module bytes to verify entry selection and execution.

## User Stream (WebSocket <-> WASM)

Spearlet supports a bidirectional streaming bridge from external clients to a WASM instance using a fd/epoll model.

- WebSocket endpoint: `GET /api/v1/executions/{execution_id}/streams/ws`
- If the execution is not directly reachable, use the SMS gateway:
  - `POST /api/v1/executions/{execution_id}/streams/session`
  - `GET /api/v1/executions/{execution_id}/streams/ws?token=...`
- Payload: SSF v1 binary frames (`SPST` magic). See: [wasm-user-stream-bridge-en.md](./api/spear-hostcall/wasm-user-stream-bridge-en.md)

### WASM hostcalls

- `user_stream_open(stream_id: i32, direction: i32) -> i32`
- `user_stream_read(fd: i32, out_ptr: i32, out_len_ptr: i32) -> i32`
- `user_stream_write(fd: i32, buf_ptr: i32, buf_len: i32) -> i32`
- `user_stream_close(fd: i32) -> i32`
- `user_stream_ctl_open() -> i32`
- `user_stream_ctl_read(fd: i32, out_ptr: i32, out_len_ptr: i32) -> i32`

Direction values:

- `1`: inbound (client -> guest)
- `2`: outbound (guest -> client)
- `3`: bidirectional

Recommended pattern:

- Use `spear_epoll_create/spear_epoll_ctl/spear_epoll_wait` to wait on `EPOLLIN/EPOLLOUT`.
- Treat `-EAGAIN` as backpressure and retry when epoll indicates readiness.

Stream discovery pattern:

- Create a control fd via `user_stream_ctl_open()` and register it to epoll with `EPOLLIN`.
- When it becomes readable, call `user_stream_ctl_read()` to fetch a fixed 8-byte event:
  - `u32 stream_id` (little-endian)
  - `u32 kind` (little-endian), currently:
    - `1`: stream connected
    - `2`: session closed
- After receiving `stream_id`, call `user_stream_open(stream_id, direction)` to bind a data fd for that stream.
- Close the control fd with `user_stream_close(fd)` when done.
