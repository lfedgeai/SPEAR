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
    "uri": "sms+file://<file_id>",
    "name": "hello.wasm",
    "args": [],
    "env": {}
  }
}
```
- The runtime downloads content from the artifact `location` during `create_instance`, validates the WASM magic header before loading, and then creates the WASM instance handle.
- When built with the `wasmedge` feature enabled, `start_instance` starts a WASM worker that receives and executes subsequent invocation requests.

### SMS File Scheme and Config Source

- Supported `sms+file` forms:
  - Explicit override: `sms+file://<host:port>/<file_id>`
  - Short form: `sms+file://<file_id>` (runtime uses `SpearletConfig.sms_http_addr` for the HTTP gateway)
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
