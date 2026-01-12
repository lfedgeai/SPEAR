# CChat Function Call（Tool Calling）能力设计

## 背景

Spear 的 WASM hostcall 提供了 Chat Completion 的会话式 API，其中 `cchat_write_fn(fd, fn_offset, fn_json)` 用于把 WASM 侧的函数（tool）注册到会话。

当前能力缺口是：

- Chat completion 请求虽然可以携带 tools，但宿主侧不会自动根据模型返回的 function/tool call 去调用 WASM 函数。
- 因此无法实现业界常见的「模型提出工具调用 → 运行工具 → 把工具结果回填给模型 → 继续推理直到不再需要工具调用」的闭环。

相关现状与约定：

- Hostcall 文档描述了 `cchat_write_fn`、`cchat_send` 的 flags 里包含「自动 function call」的规划（bit 1）。见 [chat-completion-zh.md](file:///Users/bytedance/Documents/GitHub/bge/spear/docs/api/spear-hostcall/chat-completion-zh.md)。
- `cchat_write_fn` 在宿主侧会持久化 `fn_offset` 与 `fn_json`（tool schema）。代码入口见 [wasm_hostcalls.rs](file:///Users/bytedance/Documents/GitHub/bge/spear/src/spearlet/execution/runtime/wasm_hostcalls.rs)。

本设计基于现有 hostcall/legacy 约定，并对齐 OpenAI 兼容接口的最佳实践。

## 目标

- 在 `cchat_send`（Chat completion）时，把 `cchat_write_fn` 注册的 tool schema 作为 `tools` 传给上游模型。
- 当模型返回 tool/function call 时：
  - 在宿主侧按 tool name 找到对应 `fn_offset`。
  - 调用 WASM 侧函数（通过 table + funcref 机制），把参数传入，把返回值拿回。
  - 将工具结果追加为 `role=tool` 消息继续调用 chat completion。
  - 循环直到模型不再返回新的 tool call。
- 具备可观测性（日志/指标）、可控性（迭代次数/输出大小限制）、以及可回滚的开关。

## 非目标

- 不在本阶段实现 streaming 下的 tool calling（可作为后续扩展）。
- 不在本阶段实现跨会话的工具沙箱/权限系统（但会预留安全边界与限制点）。

## 术语

- **Tool / Function**：模型可调用的外部能力；在本系统中由 WASM 侧函数实现。
- **fn_offset**：WASM 函数指针/偏移。对 wasm32 来说，函数指针本质是 table index（funcref 表索引）。
- **tool schema**：传给模型的 JSON schema，通常包含 name/description/parameters。

## 现状梳理（Legacy）

### 1) tool 注册

WASM 侧通过 `cchat_write_fn(fd, fn_offset, fn_json)` 注册工具。

- `fn_offset`：WASM 函数在表中的索引（要求 wasm 导出 table，或有默认 `__indirect_function_table`）。
- `fn_json`：工具 schema JSON 字符串。为了对齐 OpenAI，推荐 schema 形如：

```json
{
  "type": "function",
  "function": {
    "name": "tool_call",
    "description": "...",
    "parameters": {"type":"object", "properties": {}}
  }
}
```

### 2) chat completion

`cchat_send` 会将会话 messages 与 tools 组装成请求，交给 AI backend（例如 OpenAI chat/completions）。目前 backend 适配层会返回原始 JSON payload（canonical envelope）。见 [openai_chat_completion.rs](file:///Users/bytedance/Documents/GitHub/bge/spear/src/spearlet/execution/ai/backends/openai_chat_completion.rs)。

### 3) 缺口

即便模型返回 `tool_calls` / `function_call`，宿主也不会自动解析并调用对应 `fn_offset`。

## 业界最佳实践（对齐点）

以 OpenAI 兼容接口为例：

1. 请求携带 `tools`（一组 function schema），可选设置 `tool_choice`。
2. 模型返回 assistant message，并包含 `tool_calls` 数组：

```json
{
  "role": "assistant",
  "tool_calls": [
    {
      "id": "call_abc",
      "type": "function",
      "function": {"name": "tool_call", "arguments": "{...json...}"}
    }
  ]
}
```

3. 客户端执行工具，把结果作为 `role=tool` 的消息回填，并带上 `tool_call_id`：

```json
{"role":"tool","tool_call_id":"call_abc","content":"{...result...}"}
```

4. 继续调用 chat completion，直到不再出现 tool_calls。

本设计将严格遵循这一消息结构，以保证模型行为稳定和可迁移。

## 设计总览

### 分层

- **WASM hostcall 层（编排层）**：实现 tool-calling loop（因为这里能同时拿到 `host_data` 与 `instance`）。
- **HostApi/会话存储层（状态层）**：负责会话 fd 的生命周期、messages/tools 的持久化。
- **AI 后端适配层（上游调用层）**：负责将 canonical request 变成上游请求，返回 canonical response。

### 新增/调整的数据结构

为了让编排层能正确回填工具调用的消息，需要扩展 chat completion 的内部 IR：

1. **ChatMessage 扩展**

- 现状：只有 `role` + `content`。
- 需要增加（可选字段）：
  - `tool_call_id: Option<String>`（role=tool 时使用）
  - `tool_calls: Option<Vec<ToolCall>>`（role=assistant 时使用）
  - `name: Option<String>`（兼容部分 provider）

2. **ToolCall 结构**

- `id: String`
- `name: String`
- `arguments_json: String`（保留原始 arguments 字符串，避免序列化差异）

3. **ToolRegistry（每会话）**

- `Vec<ToolEntry>` 或 `HashMap<String, ToolEntry>`：
  - `name: String`
  - `fn_offset: i32`
  - `schema_json: String`

`name` 从 `fn_json` 中解析得到（优先 `function.name`）。解析失败则该 tool 不参与自动调用。

### WASM Tool ABI

WASM tool 函数建议统一签名：

```text
tool(args_ptr: i32, args_len: i32, out_ptr: i32, out_len_ptr: i32) -> i32
```

- `args_ptr/args_len`：指向 UTF-8 JSON arguments（与上游 `tool_calls[].function.arguments` 一致）。
- `out_ptr/out_len_ptr`：输出缓冲区与长度指针。
- 返回值：
  - `0`：成功，tool 已写入 `out_ptr`，并在 `*out_len_ptr` 写入实际长度。
  - `-ENOSPC`：输出缓冲区不足，tool 在 `*out_len_ptr` 写入所需长度，宿主可扩容后重试。
  - 其它负数：错误码。

该模式与 `cchat_recv` 的「缓冲区不足→返回 ENOSPC 并告知所需长度」一致，便于复用宿主侧读写逻辑。

### `fn_offset` 到函数的解析与调用

宿主侧按以下规则解析并调用：

1. 找导出的 table：优先 `__indirect_function_table`，其次 `table`，否则取第一个导出 table。
2. `table.get_data(fn_offset)` 得到 funcref。
3. 将 funcref 转为 `Function`，校验签名为 `(i32,i32,i32,i32)->i32`。
4. 通过 `Executor::call_func` 调用。

注意：为了稳定性，生产实现必须为 args/out 在 guest memory 中分配/管理内存。

## 核心流程（Auto Tool Calling Loop）

### 触发条件

- `cchat_send(fd, flags)` 中 flags 包含 `AUTO_TOOL_CALL`（建议沿用文档规划：bit 1）。
- 启用 `AUTO_TOOL_CALL` 时必须同时启用“最大工具调用次数”限制，用于约束本次 `cchat_send` 触发的 tool 调用总次数（跨多轮 completion 累计）。

### 算法

给定会话 `fd`：

1. 从 session 快照生成 ChatCompletions 请求：`messages` + `tools`（来自 ToolRegistry）。
2. 调用 AI backend。
3. 解析 response：
   - 若无 tool_calls：将 assistant content 写回会话并结束。
   - 若存在 tool_calls：
     1) 将 assistant message（含 tool_calls）追加到会话 messages。
     2) 对每个 tool_call：
        - 在 ToolRegistry 里按 name 找到 `fn_offset`。
        - 调用 WASM tool，得到 tool_result 字符串。
        - 将 `role=tool, tool_call_id=..., content=tool_result` 追加到会话 messages。
     3) 回到步骤 1（继续下一轮 completion）。

### 终止与限制

为避免无限循环/资源耗尽：

- `max_iterations`：默认 8（可配置）。
- `max_tool_output_bytes`：默认 64KiB（可配置）。
- `max_total_tool_calls`：默认 32（可配置）。用于限制 `AUTO_TOOL_CALL` 触发的 tool 调用总次数（每执行一次 WASM tool 计 1 次）。
- 超限时返回可重试/不可重试错误，并在会话中写入可诊断信息。

### 多 tool_calls 处理

同一轮 assistant message 可能包含多个 tool_calls。

- 建议顺序执行（与 OpenAI 文档一致，且易于复用 guest memory）。
- 执行结果按 tool_calls 顺序逐个追加 `role=tool`。

## 错误处理策略

### unknown tool

- 如果模型请求了未注册 name：
  - 作为 tool 消息回填一个结构化错误（建议 JSON），并继续让模型自我修正。
  - 或在严格模式下直接返回错误（可配置）。

### tool 执行失败

- 返回值非 0：
  - 回填 `role=tool` 的错误内容，包含 rc 与简要说明。
- 内存分配失败/越界：
  - 直接终止本次调用，返回 `ExecutionError`。

### 上游返回不兼容格式

- 对 OpenAI 兼容格式：优先解析 `choices[0].message.tool_calls`，并兼容旧字段 `function_call`。
- 解析失败：当作无 tool_calls 处理，但会记录 debug 日志。

## 可观测性

- 在 auto tool call loop 中输出结构化 debug 日志：
  - iteration、tool_name、tool_call_id、rc、输出长度、上游 request_id。
- 指标建议：
  - tool_call_iterations_total
  - tool_calls_total（按 tool name 分组）
  - tool_call_failures_total（按 rc 分组）
  - tool_output_bytes_total

## 配置与开关

- `AUTO_TOOL_CALL` flag：默认关闭，逐步灰度。
- 全局配置：max_iterations / max_total_tool_calls / max_tool_output_bytes / strict_unknown_tool 等。

## 兼容性与迁移

- 未开启 `AUTO_TOOL_CALL` 时行为不变。
- 旧 tool schema（仅 function 部分）可以在宿主侧做兼容包装：
  - 若 `fn_json` 顶层无 `type/function`，但包含 `name/parameters`，可包装为 OpenAI tools 结构。
- IR 扩展会影响 backend adapter 组装 messages 的方式，需要提供向后兼容序列化。

## 测试计划

- 单元测试：
  - 构造包含 `tool_calls` 的 mock OpenAI response，验证 loop 能正确调用 tool 并追加 tool message。
  - 覆盖 unknown tool / tool rc!=0 / ENOSPC 扩容重试。
- 集成测试：
  - 基于 wasm sample（导出 table + tool），跑一轮 end-to-end。
- 回归测试：
  - 未开启 AUTO_TOOL_CALL 的 chat completion 行为不变。

## 安全注意事项

- 不记录 tool arguments / tool output 的完整内容（默认只记录长度与摘要），避免泄露敏感信息。
- 对 tool output 做大小限制与 UTF-8 校验。
- 迭代次数/总调用次数限制，防止 prompt 注入导致的无限调用。
