# Spear Hostcall API: Chat Completion

## Overview

This document describes the Chat Completion API design for Spear WASM hostcalls. The API follows a syscall-like abstraction using file descriptors (fd) and supports incremental request construction for OpenAI Chat Completion, including message writes, tool/function registration (with a WASM function offset for callbacks), parameter control, request send, and response receive.

The API supports automatic tool invocation: when the model response contains `tool_calls`, the host can call back into the WASM guest using the registered `fn_offset`, execute the corresponding function, and merge results into the conversation.

## Function Signatures

All functions are `extern "C"` and WASM-friendly (e.g., `i32`, `*const u8`). Errors return negative values (e.g., `-1` for invalid fd).

## Error Codes

- `0`: Success
- `-1`: Invalid fd
- `-2`: Invalid pointer or memory access failure
- `-3`: Output buffer too small (writes required length back to `*len`)
- `-4`: Invalid `cmd` in `cchat_ctl`
- `-5`: Internal error

### Versioning and breaking changes

To align with the general fd/epoll subsystem (shared `-errno` conventions, generic `spear_fd_ctl`, and making response fds pollable), breaking changes to `cchat_*` are acceptable.

Engineering spec:

- [fd-epoll-subsystem-en.md](./fd-epoll-subsystem-en.md)

Recommended evolution:

1. **Unify errors under `-errno`**
   - deprecate fixed `-1..-5` or keep only as a compatibility shim
2. **Generic control entrypoint**
   - converge generic controls (nonblock/flags/status/metrics) to `spear_fd_ctl`
3. **Stronger async semantics (optional)**
   - allow `cchat_send` to return a response fd immediately and produce output in background; use epoll `EPOLLIN` when readable

Breaking-change update requirements (must):

- Docs: this file and `chat-completion-zh.md`
- C SDK and sample:
  - `sdk/c/include/spear.h`
  - `samples/wasm-c/chat_completion.c`
- Rust tests:
  - `src/spearlet/execution/runtime/wasm.rs` (WAT import symbol tests)
  - `src/spearlet/execution/host_api.rs` (cchat pipeline unit tests)
  - `tests/wasm_openai_e2e_tests.rs` (if relying on specific semantics)

### 1. `cchat_create() -> i32`
- **Description**: Create a new chat completion session and return its fd.
- **Args**: None.
- **Returns**: fd (>0) or error code (<0).

### 2. `cchat_write_msg(fd: i32, role: *const u8, role_len: usize, content: *const u8, content_len: usize) -> i32`
- **Description**: Write one message into the session message list.
- **Args**:
  - `fd`: Session descriptor.
  - `role`: Role string (e.g., "user").
  - `role_len`: Role length.
  - `content`: Content string.
  - `content_len`: Content length.
- **Returns**: `0` on success or an error code.

### 3. `cchat_write_fn(fd: i32, fn_offset: i32, fn_json: *const u8, json_len: usize) -> i32`
- **Description**: Register one tool/function definition and its WASM callback offset.
- **Args**:
  - `fd`: Session descriptor.
  - `fn_offset`: WASM function table offset/index used for callbacks.
  - `fn_json`: Tool/function schema JSON (e.g., `{ "name": "get_weather", "parameters": { ... } }`).
  - `json_len`: JSON length.
- **Returns**: `0` on success or an error code.
- **Note**: When `tool_calls` appear, the host calls into the guest by `fn_offset`. A typical guest ABI can be: `fn(args_ptr: i32, args_len: i32, out_ptr: i32, out_len_ptr: i32) -> i32`.

### 4. `cchat_ctl(fd: i32, cmd: i32, arg_ptr: *mut u8, arg_len: *mut usize) -> i32`
- **Description**: Control operations on a session/response descriptor (fcntl-like).
- **Args**:
  - `fd`: Session fd or response fd.
  - `cmd`: Command code (e.g., `CTL_SET_PARAM = 1`, `CTL_GET_METRICS = 2`).
  - `arg_ptr`: In/out buffer.
  - `arg_len`: In length / out actual length.
- **Returns**: `0` for set operations; bytes written for get operations; or an error code.
- **Supported commands**:
  - `CTL_SET_PARAM (1)`: Set a parameter with JSON like `{ "key": "model", "value": "gpt-4" }`.
  - `CTL_GET_METRICS (2)`: Read metrics (usage) into `arg_ptr`.

### 5. `cchat_send(fd: i32, flags: i32) -> i32`
- **Description**: Send the request and return `response_fd`.
- **Args**:
  - `fd`: Session descriptor.
  - `flags`: Bit flags (`bit 0`: enable metrics, `bit 1`: enable auto tool call).
- **Returns**: `response_fd` (>0) or an error code.
- **Note**: With auto tool call enabled, the host processes `tool_calls`, calls into the guest, and may loop send/recv until the final assistant message is produced.

### 6. `cchat_recv(response_fd: i32, out_buf: *mut u8, buf_len: *mut usize) -> i32`
- **Description**: Receive the response JSON from `response_fd`.
- **Args**:
  - `response_fd`: Response descriptor.
  - `out_buf`: Output buffer.
  - `buf_len`: In max len / out actual len.
- **Returns**: Bytes received or an error code.

### 7. `cchat_close(fd: i32) -> i32`
- **Description**: Close a session/response descriptor.
- **Args**: `fd`.
- **Returns**: `0` on success or an error code.

## Example

### C
```c
#define CTL_SET_PARAM 1
#define CTL_GET_METRICS 2

// Enable auto tool calling: bit 1
#define AUTO_TOOL_CALL (1 << 1)

// Provide a writable arena for tool arguments + tool output
static uint8_t TOOL_ARENA[128 * 1024];

int32_t tool_call(int32_t args_ptr, int32_t args_len, int32_t out_ptr, int32_t out_len_ptr) {
  uint32_t cap = *(uint32_t *)(uintptr_t)out_len_ptr;
  const char *result = "{\"ok\":true}";
  uint32_t need = (uint32_t)strlen(result);
  if (cap < need) {
    *(uint32_t *)(uintptr_t)out_len_ptr = need;
    return -ENOSPC;
  }
  memcpy((void *)(uintptr_t)out_ptr, result, need);
  *(uint32_t *)(uintptr_t)out_len_ptr = need;
  return 0;
}

int fd = cchat_create();

char param_json[] = "{\"key\":\"model\", \"value\":\"gpt-4\"}";
size_t param_len = strlen(param_json);
cchat_ctl(fd, CTL_SET_PARAM, param_json, &param_len);

cchat_write_msg(fd, "user", strlen("user"), "Hello", strlen("Hello"));

char fn_json[] = "{\"name\":\"tool\", \"parameters\":{...}}";
cchat_write_fn(fd, 5 /* offset */, fn_json, strlen(fn_json));

// Required for AUTO_TOOL_CALL
// - tool_arena_ptr/tool_arena_len: guest memory region for tool I/O
// - max_total_tool_calls: hard cap per cchat_send
char arena_ptr_json[128];
snprintf(arena_ptr_json, sizeof(arena_ptr_json),
         "{\"key\":\"tool_arena_ptr\",\"value\":%u}",
         (unsigned)(uintptr_t)TOOL_ARENA);
size_t arena_ptr_len = strlen(arena_ptr_json);
cchat_ctl(fd, CTL_SET_PARAM, arena_ptr_json, &arena_ptr_len);

char arena_len_json[128];
snprintf(arena_len_json, sizeof(arena_len_json),
         "{\"key\":\"tool_arena_len\",\"value\":%u}",
         (unsigned)sizeof(TOOL_ARENA));
size_t arena_len_len = strlen(arena_len_json);
cchat_ctl(fd, CTL_SET_PARAM, arena_len_json, &arena_len_len);

char max_calls_json[] = "{\"key\":\"max_total_tool_calls\",\"value\":8}";
size_t max_calls_len = strlen(max_calls_json);
cchat_ctl(fd, CTL_SET_PARAM, max_calls_json, &max_calls_len);

int resp_fd = cchat_send(fd, 3 /* metrics + auto_call */);

char buf[4096];
size_t len = 4096;
cchat_recv(resp_fd, buf, &len);

cchat_close(fd);
cchat_close(resp_fd);
```

## Implementation Notes

- Memory: the WASM side allocates `out_buf`, and the host writes into it.
- Tool callbacks: the guest should export a function table; the host calls by `fn_offset`.
- Errors: negative return values indicate failures; an optional `get_error` can be added later.
- Extensions: streaming can be enabled by setting a param like `"stream": true` via `cchat_ctl`.

Version: v1.0 (designed on 2025-12-16).
