# Samples

This directory contains buildable WASM samples (C) and their build outputs.

## Layout

- `wasm-c/`: sample sources (C)
- `build/`: build outputs (`.wasm`)

## Build

Run from the repo root:

```bash
make samples
```

The build uses `zig` (`zig cc -target wasm32-wasi`) if available; otherwise it falls back to `clang` + `WASI_SYSROOT`.

## Samples

- `hello.c`: minimal sample
- `chat_completion.c`: basic Chat Completion call
- `chat_completion_tool_sum.c`: WASM custom tool + AUTO_TOOL_CALL loop
- `mic_rtasr.c`: mic + realtime ASR sample
- `mcp_fs.c`: MCP filesystem (stdio) tool injection + execution sample

## MCP sample (mcp_fs)

This sample demonstrates:

1) enabling MCP via `cchat_ctl_set_param` session params (`mcp.enabled=true`, `mcp.server_ids=["fs"]`, etc.)
2) runtime MCP tool injection into `tools`
3) using `AUTO_TOOL_CALL` so the runtime executes MCP tool calls automatically

### Prerequisites

- SMS loads MCP server configs (this repo includes `config/sms/mcp.d/fs.toml`).
  - You must set it explicitly via `--mcp-dir ./config/sms/mcp.d` or `SMS_MCP_DIR=./config/sms/mcp.d`.
- `npx` is available on the host (the fs server is started via stdio using `@modelcontextprotocol/server-filesystem`)

If SMS does not load MCP configs, Spearlet will see an empty MCP registry, no MCP tools will be injected into `tools`, and the model may respond as if MCP tools do not exist.

### What to look for

- Source: `./wasm-c/mcp_fs.c`
- Output: `./build/mcp_fs.wasm`

If the final response includes tool calls/tool outputs (and ends with `MCP_OK`), MCP injection and execution are working.
