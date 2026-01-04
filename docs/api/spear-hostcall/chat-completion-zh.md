# Spear Hostcall API: Chat Completion 部分

## 概述

这个部分描述了 Spear 项目中 WASM hostcall 的 Chat Completion API 设计。该 API 采用 syscall-like 抽象，基于文件描述符 (fd) 进行操作，支持增量构建 OpenAI Chat Completion 请求，包括消息添加、函数工具注册（带 offset 用于 WASM 调用）、参数设置、发送请求和接收响应。设计参考 Linux syscall（如 create, write, send, recv, ctl, close），以实现高效、状态化的 API 调用。

API 支持自动工具调用：当响应包含 tool_calls 时，host 可以根据注册的 fn_offset 回调 WASM guest 的函数，处理参数并整合结果。

## 返回内容

- `cchat_recv` 返回模型响应 JSON（建议返回 OpenAI 原始响应结构，包含 `choices`、`usage` 等字段）。
- `cchat_ctl(..., CTL_GET_METRICS, ...)` 用于在启用 metrics 时读取使用量等指标（建议返回 `usage` 子对象 JSON）。

## 函数签名

所有函数均为 extern "C"，参数兼容 WASM（i32, *const u8 等）。错误返回负值（e.g., -1 invalid fd）。

## 错误码

- `0`：成功
- `-1`：fd 无效
- `-2`：指针无效或内存访问失败
- `-3`：输出缓冲区不足（会把需要的长度写回 `*len`）
- `-4`：`cchat_ctl` 的 `cmd` 不支持
- `-5`：内部错误

### 版本与破坏性变更说明

为对齐通用 fd/epoll 子系统（支持 `-errno`、通用 `spear_fd_ctl`、以及 response fd 的可 poll 能力），允许对 `cchat_*` 做破坏性变更。

工程化规范见：

- [fd-epoll-subsystem-zh.md](file:///Users/bytedance/Documents/GitHub/bge/spear/docs/api/spear-hostcall/fd-epoll-subsystem-zh.md)

推荐的演进方向：

1. **错误码统一为 `-errno`**
   - `-1..-5` 固定码可废弃或仅保留兼容层
2. **通用控制入口**
   - 将 nonblock/flags/status/metrics 等通用能力收敛到 `spear_fd_ctl`
3. **更强的异步语义（可选）**
   - `cchat_send` 可变为异步：立即返回 response_fd，后台生成响应；当 response 可读时通过 epoll 的 `EPOLLIN` 通知

破坏性变更的同步更新要求（必须）：

- 文档：本文件与 `chat-completion-en.md`
- C SDK 与样例：
  - `sdk/c/include/spear.h`
  - `samples/wasm-c/chat_completion.c`
- Rust 测试：
  - `src/spearlet/execution/runtime/wasm.rs`（WAT 导入符号测试）
  - `src/spearlet/execution/host_api.rs`（cchat pipeline 单测）
  - `tests/wasm_openai_e2e_tests.rs`（如依赖具体语义）

### 1. cchat_create() -> i32
- **描述**：创建新的 chat completion 会话，返回 fd。
- **参数**：无。
- **返回**：fd (>0) 或错误码 (<0)。

### 2. cchat_write_msg(fd: i32, role: *const u8, role_len: usize, content: *const u8, content_len: usize) -> i32
- **描述**：向 messages 列表写入一条消息。
- **参数**：
  - fd: 会话描述符。
  - role: 角色字符串 (e.g., "user")。
  - role_len: 角色长度。
  - content: 内容字符串。
  - content_len: 内容长度。
- **返回**：0 (成功) 或错误码。

### 3. cchat_write_fn(fd: i32, fn_offset: i32, fn_json: *const u8, json_len: usize) -> i32
- **描述**：向 tools 列表写入一个函数/工具定义，支持 WASM 回调。
- **参数**：
  - fd: 会话描述符。
  - fn_offset: WASM 函数表偏移/索引，用于回调。
  - fn_json: 函数描述 JSON (e.g., {"name": "get_weather", "parameters": {...}})。
  - json_len: JSON 长度。
- **返回**：0 (成功) 或错误码。
- **注意**：host 在 tool_calls 时，使用 offset 调用 guest 函数。

## 工具回调 ABI（建议）

为便于 host 通过 `fn_offset` 直接调用 WASM guest，建议 guest 提供统一的调用 ABI，例如：

```c
int tool_call(int args_ptr, int args_len, int out_ptr, int out_len_ptr);
```

- `args_ptr/args_len` 指向 UTF-8 的 `arguments` JSON 字符串。
- guest 将 tool 执行结果写入 `out_ptr`，并写回 `out_len_ptr`。
- 返回值为 0 表示成功，负值表示失败。

### 4. cchat_ctl(fd: i32, cmd: i32, arg_ptr: *mut u8, arg_len: *mut usize) -> i32
- **描述**：对 fd 进行控制操作（类似于 fcntl）。
- **参数**：
  - fd: 会话或响应描述符。
  - cmd: 命令码 (e.g., CTL_SET_PARAM = 1, CTL_GET_METRICS = 2)。
  - arg_ptr: 输入/输出缓冲区。
  - arg_len: 输入长度/输出实际长度。
- **返回**：set 操作返回 0；get 操作返回字节数 或错误码。
- **支持 cmd**：
  - CTL_SET_PARAM (1): 设置参数 (arg: JSON 如 {"key":"model", "value":"gpt-4"})。
  - CTL_GET_METRICS (2): 获取 metrics (arg: 输出缓冲区)。

### 5. cchat_send(fd: i32, flags: i32) -> i32
- **描述**：发送请求，返回 response_fd。
- **参数**：
  - fd: 会话描述符。
  - flags: 位标志 (bit 0: enable metrics, bit 1: enable auto tool call)。
- **返回**：response_fd (>0) 或错误码。
- **注意**：如果启用 auto tool call，host 自动处理 tool_calls，回调 guest 函数，并可能循环发送。

### 6. cchat_recv(response_fd: i32, out_buf: *mut u8, buf_len: *mut usize) -> i32
- **描述**：从 response_fd 接收响应结果 (JSON)。
- **参数**：
  - response_fd: 响应描述符。
  - out_buf: 输出缓冲区。
  - buf_len: 输入 max len / 输出 actual len。
- **返回**：接收字节数 或错误码。

### 7. cchat_close(fd: i32) -> i32
- **描述**：关闭 fd 或 response_fd，释放资源。
- **参数**：fd。
- **返回**：0 (成功) 或错误码。

## 使用示例

### C 示例
```c
#define CTL_SET_PARAM 1
#define CTL_GET_METRICS 2

int fd = cchat_create();

char param_json[] = "{\"key\":\"model\", \"value\":\"gpt-4\"}";
size_t param_len = strlen(param_json);
cchat_ctl(fd, CTL_SET_PARAM, param_json, &param_len);

cchat_write_msg(fd, "user", strlen("user"), "Hello", strlen("Hello"));

char fn_json[] = "{\"name\":\"tool\", \"parameters\":{...}}";
cchat_write_fn(fd, 5 /* offset */, fn_json, strlen(fn_json));

int resp_fd = cchat_send(fd, 3 /* metrics + auto_call */);

char buf[4096];
size_t len = 4096;
cchat_recv(resp_fd, buf, &len); // buf 包含响应

cchat_close(fd);
cchat_close(resp_fd);
```

## 实现注意
- **内存管理**：WASM 侧负责分配 out_buf，host 写入。
- **工具调用**：guest 需导出函数表，host 通过 offset 调用。
- **错误处理**：负返回值为错误码，未来可添加 get_error 函数。
- **扩展**：支持 streaming 通过 ctl 设置 "stream": true。

文档版本：v1.0 (基于 2025-12-16 设计)。
