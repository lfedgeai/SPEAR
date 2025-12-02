# WASM Runtime Usage

## Overview
The Spearlet WASM runtime requires a valid WASM binary at instance creation. If the provided bytes are not a valid WASM module, the runtime returns `InvalidConfiguration: "Invalid WASM module format"` immediately.

## Instantiation Requirements
- Instance configuration must resolve module bytes via `InstanceConfig.runtime_config["module_bytes"]` or by downloading from the task `executable.uri`.
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
- The runtime downloads content from `executable.uri` and validates the module format in `create_wasm_instance`.

### SMS File Scheme and Config Source

- Only `sms+file://<file_id>` is supported for remote download in the WASM runtime.
- The runtime constructs path `"/api/v1/files/<file_id>"` and reads SMS address from `RuntimeConfig.spearlet_config.sms_addr`.
- Download function:

```rust
pub async fn fetch_sms_file(sms_addr: &str, path: &str) -> ExecutionResult<Vec<u8>>
```

- Configuration is injected at runtime initialization: FunctionService passes the full `SpearletConfig` into each `Runtime` via `RuntimeConfig.spearlet_config`, avoiding ad‑hoc environment variable reads.

## Error Behavior
- Invalid WASM bytes: instance creation fails at validation stage to avoid late execution errors.
- Download/validation failures: detailed error recorded, returning `RuntimeError` or `InvalidConfiguration`.

## Best Practices
- Produce valid WASM during build (e.g., `zig cc -target wasm32-wasi`).
- When uploading via SMS file service, provide `checksum_sha256` if possible.
- In integration tests, explicitly provide valid module bytes to verify entry selection and execution.
