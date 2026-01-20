# MCP 排错指南

本文解释为什么模型可能会返回类似内容：

> “我无法访问外部文件或系统，包括你提到的 MCP filesystem tools”

以及如何在 Spear 中确认 MCP 工具是否真的可用。

## 这段返回代表什么

这段内容是一个普通的模型回复（finish_reason = "stop"），并不是 MCP 执行报错。
多数情况下，这意味着发给 OpenAI 的请求里没有携带任何可调用的 `tools`，模型自然无法调用 MCP。

## 当前实现中 MCP 工具如何注入

当前 MCP 工具的注入与执行只发生在 **cchat host API 的自动 tool-call 循环**里：

- 注入工具：`cchat_inject_mcp_tools`，见 [cchat.rs](../src/spearlet/execution/host_api/cchat.rs)
- 执行工具：`cchat_exec_mcp_tool`，见 [cchat.rs](../src/spearlet/execution/host_api/cchat.rs)
- MCP registry 同步（从 SMS 拉取）：见 [registry_sync.rs](../src/spearlet/mcp/registry_sync.rs)

如果你走的是另一条调用链（不是 cchat auto tool-call），MCP 工具可能根本不会被挂到请求里。

## 必需的 session 参数

MCP 工具注入受 session 参数控制：

- `mcp.enabled`：bool，必须为 `true`
- `mcp.server_ids`：string 数组，必须包含 server id（例如 `"fs"`）
- `mcp.task_tool_allowlist` / `mcp.task_tool_denylist`：task 级工具过滤（如配置）；由 host 从 `Task.config` 注入；WASM 侧不可写

解析逻辑见：`session_policy_from_params`，[policy.rs](../src/spearlet/mcp/policy.rs)

任何一个缺失，都会导致 MCP 工具不注入，模型就会像“没有工具”一样回答。

补充：在当前实现中，如果 task 配置了 `Task.config` 的 MCP 策略，`cchat_create` 会自动写入 `mcp.enabled` / `mcp.server_ids` 等缺省值，你未必需要手工 set_param。

## 常见根因

1. **params 没有开启 MCP**（缺少 `mcp.enabled` / `mcp.server_ids`）
2. **Spearlet 没有成功启动 MCP registry 同步**（未连上 SMS 或 SMS 没提供 registry）
3. **server_id 不匹配**（registry 里找不到对应 server）
4. **server policy 禁止工具**（allowed_tools 为空或过严）
5. **list_tools 超时/失败**，导致拿到的工具列表为空
6. **task policy 拒绝**：task 未启用 MCP 或你试图设置超出 task allowed 的 server_ids（会被 hostcall 拒绝）

## 建议检查点

- 响应里是否包含 `choices[0].message.tool_calls`？
  - 如果没有，一般代表工具未附加（或模型没有选择发起 tool_call）。
- Spearlet 的 registry sync 日志：
  - `MCP registry watch start failed`
  - `MCP registry watch ended`
  - 这些通常指向 SMS 连通性或 MCP registry 服务问题。
- server allowlist：
  - `allowed_tools` 需要包含能匹配工具名的 pattern。
