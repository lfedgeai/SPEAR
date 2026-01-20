# Task 级 MCP Server 子集选择：设计方案（ZH）

## 背景

在实际业务中，一个 task 往往只需要少数几个 MCP server（例如仅需要 `gitlab` 与 `fs`），而不是把注册中心中所有 MCP server 全量暴露给模型。全量暴露会带来：

- 成本增加：每次注入都需要 `tools/list`，增加延迟与资源消耗
- 可靠性降低：某个 MCP server 不稳定会拖累整体体验
- 安全风险扩大：模型可见工具越多，越难做权限收敛与审计
- 选择困难：模型在大量工具中做决策更容易误用

因此需要一套“按 task 选择 MCP server 子集（subset）”的机制，在不破坏现有架构边界的前提下，做到：**默认最小可用集、可治理、可审计、可扩展**。（按 invocation 的覆盖属于可选后续增强）

## 现状梳理（基于现有代码）

### MCP tools 注入入口（数据面）

当前 Spearlet 在 ChatCompletion 发送前会执行 MCP tools 注入，核心逻辑在：

- 注入：[`src/spearlet/execution/host_api/cchat.rs`](../src/spearlet/execution/host_api/cchat.rs)
  - `cchat_inject_mcp_tools`：读取 `snapshot.params`，解析会话 MCP 策略，然后对指定 server 执行 `tools/list` 并注入工具
- 会话策略解析：[`src/spearlet/mcp/policy.rs`](../src/spearlet/mcp/policy.rs)
  - `session_policy_from_params`：解析 `mcp.enabled` / `mcp.server_ids` / `mcp.tool_allowlist` / `mcp.tool_denylist`

这意味着：**subset 选择已经天然支持**，只缺一个“把 task 意图写入 `snapshot.params`”的控制面到数据面的衔接。

### 当前实现范围（重要）

截至当前代码实现：

- 支持：Task 级策略（来自 `Task.config`）自动注入到 chat session params，并在 hostcall 层做越权约束。
- 不支持：per-invocation（通过 invocation metadata/context_data）覆盖 MCP 子集与 allow/deny。（该能力留作后续再决定是否引入）

### MCP server 注册中心（控制面）

当前 SMS 负责注册表权威存储，并支持从目录加载：

- 目录加载：[`src/sms/service.rs`](../src/sms/service.rs)（`bootstrap_mcp_from_dir`）
- Registry proto：[`proto/sms/mcp_registry.proto`](../proto/sms/mcp_registry.proto)

每个 `McpServerRecord` 自带：

- `allowed_tools`：server 级白名单 pattern
- `budgets`：超时、并发、输出大小
- `approval_policy`：审批策略（如未来扩展）

这为“平台治理层”提供了基础能力。

## 目标与非目标

### 目标

- 支持按 task 指定“默认 MCP server 子集”（default subset）
- 与分层策略兼容：平台治理（SMS registry）→ task 约束 → 会话级选择（由 session params 显式设置；若未设置则使用 task default）
- 失败可控：子集为空时不注入（而不是失败整个任务）；单个 server 不可用时不影响其他 server
- 可审计：记录本次注入最终生效的 server/tool 列表以及被剔除原因

### 非目标（本方案不强制实现）

- 不强制引入全新 DB schema 或复杂 UI（可作为后续增强）
- 不在第一期要求实现 per-tool 审批流闭环（可沿用 `approval_policy` 扩展）
- 不在第一期要求支持 HTTP transport 的引用解析/鉴权体系（可后续补齐）

## 设计原则（业界 best practice）

1. **默认拒绝，显式启用**：没有配置就不注入 MCP tools。
2. **分层收敛**：每一层只能进一步缩小权限集合，不能放大。
3. **稳定命名空间**：对外暴露的 tool 名带命名空间，便于审计与避免冲突（你们已有 `mcp__...__...` 方案）。
4. **可观测**：注入/执行需要暴露指标与日志，包含 server 维度的失败原因。
5. **最小改动落地**：优先复用当前 `snapshot.params` 路径。

## 核心方案：三层策略 + 子集合成算法

### 三层策略

1. **平台层（SMS registry）**
   - 允许哪些 MCP server 被使用
   - 每个 server 的 `allowed_tools` / `budgets` / `approval_policy`
2. **Task 层（Task 默认与上限）**
   - task 默认启用哪些 server（default subset）
   - task 最多允许哪些 server（allowed subset / upper bound）
3. **Session 层（chat session params 的选择）**
   - 本次 chat session 实际启用哪个 subset（通常来自 task default 注入，或由 WASM 显式设置为更小集合）

最终生效集合 = `platform_allowed ∩ task_allowed ∩ session_requested`（session 未提供则使用 task_default）

### 子集合成算法（建议落地为纯函数，便于测试）

输入：

- `registry_servers`: SMS registry 快照（server_id → record）
- `task_policy`: task 默认与约束
- `session_policy`: chat session params 表达的会话选择（可选）

输出：

- `effective_server_ids`: 本次 chat session 实际注入的 server_id 列表
- `effective_tool_allow/deny`: 合并后的工具过滤策略（pattern 列表）
- `decision_log`: 剔除原因（缺 env、无权限、未知 server、server 无 allowed_tools 等）

规则：

1. `platform_allowed_server_ids = registry_servers.keys()`
2. `task_allowed_server_ids`：
   - 如果 task 未显式配置 allowed：默认等同于 task_default（最小授权原则）
3. `requested_server_ids`：
   - 如果 session 显式指定：使用 session 指定
   - 否则：使用 task_default
4. `effective_server_ids = requested ∩ task_allowed ∩ platform_allowed`
5. 对每个 server：
   - 若 `record.allowed_tools` 为空：跳过（server policy deny-all）
   - 执行 `tools/list` 时若失败：本次不注入该 server tools（不影响其他 server）
6. 工具过滤：
   - `effective_allowed = record.allowed_tools ∩ task.tool_allowlist ∩ session.tool_allowlist`
   - `effective_deny` 叠加：任意层 deny 命中则拒绝

## 配置与数据模型（分阶段落地）

本方案建议分两期：

- Phase A：不改 proto，使用 `Task.config` 承载 MCP 选择策略（最快落地）
- Phase B：引入 “MCP Profiles/Bundles” 与 server tags（治理升级）

### Phase A：使用 Task.config 承载 task 级 MCP 子集

`Task` proto 已存在 `config: map<string,string>`（见 [`proto/sms/task.proto`](../proto/sms/task.proto)），建议约定以下 key（value 使用 JSON 字符串）：

- `mcp.enabled`: `"true" | "false"`
- `mcp.default_server_ids`: JSON 数组字符串，例如：`["gitlab","fs"]`
- `mcp.allowed_server_ids`: JSON 数组字符串（可选；未给则等同 default）
- `mcp.tool_allowlist`: JSON 数组字符串（可选）
- `mcp.tool_denylist`: JSON 数组字符串（可选）

#### allowed 与 default 的关系（关键语义）

为避免配置含义混乱，这里将 `allowed_server_ids` 与 `default_server_ids` 明确为两个不同集合：

- `default_server_ids`（Default）：
  - “会话侧未显式覆盖时，本次执行默认启用哪些 MCP servers”
  - 体现“开箱即用的最小集合”
- `allowed_server_ids`（Allow/Upper bound）：
  - “这个 task 最多允许使用哪些 MCP servers”
  - 是硬上限：会话侧显式参数或 task 内部尝试扩大 server 集合都不能超过它

不变量：

- `default_server_ids ⊆ allowed_server_ids`
- 推荐：若 `mcp.enabled=true`，则 `default_server_ids` 至少包含 1 个 server（否则等价于“开启但默认不可用”，除非你们显式要这种灰度形态）

两种推荐配置形态（业界常见）：

1. **最小授权（默认推荐）**：`allowed_server_ids = default_server_ids`
   - 表示 task 只能用这些 server，也默认就用这些 server
2. **允许更大、默认更小（为会话侧收窄预留空间）**：`allowed_server_ids ⊃ default_server_ids`
   - 表示平时默认只注入少量 server；需要时会话侧可以在 allowed 范围内选择更小/不同组合

UI 建议映射（便于治理）：

- “Allowed（上限）”多选框：编辑 `mcp.allowed_server_ids`
- “Default（默认启用）”多选框：编辑 `mcp.default_server_ids`
- UI 交互强制 `Default ⊆ Allowed`：
  - 勾选 Default 会自动勾选 Allowed
  - 取消 Allowed 会同时取消 Default

推荐规则：

- 未配置 `mcp.enabled=true` 时默认禁用 MCP
- `allowed_server_ids` 未配置时，默认为 `default_server_ids`（最小授权）
- 会话侧若请求的 server 不在 `allowed_server_ids` 中，需要被拒绝（并可记录原因）

#### Phase A 的关键改造点（与现有代码对齐）

现有注入逻辑依赖 `ChatSessionState.params`（`snapshot.params`），写入入口是 `cchat_ctl_set_param`。Phase A 需要补齐“自动写入默认 params”的路径：

- 在“创建 chat session 并开始发送消息”之前，将 task.config 中的 MCP 策略写入 `ChatSessionState.params`
- 覆盖规则：会话侧显式参数优先（但必须经过 task_allowed 校验）

落地点（示例）：

- 在创建 WASM import/host API 时，将 task.config 解析为结构化 `McpTaskPolicy` 并绑定到 host API（task 级上下文）。
- 在 `cchat_create` 时自动写入缺省会话参数：
  - `mcp.enabled`
  - `mcp.server_ids`（= task default subset）
  - `mcp.task_tool_allowlist` / `mcp.task_tool_denylist`（如配置）
- 在 `cchat_ctl_set_param` 时对 `mcp.enabled` / `mcp.server_ids` 做越权校验（不可超出 task allowed）。

注：具体“哪里创建 chat session”取决于 wasm/hostcall 的路径；该文档先定义接口与行为，落地时选择最接近 `cchat_create` 的统一入口最合适。

### Phase B：引入 Profiles/Bundles 与 server tags

当 MCP server 数量增多时，“直接写 server_id 列表”会导致 task 配置冗长且难治理。业界通常引入复用单元：

- **Profile/Bundle**：`profile_name -> server_ids[] (+ tool allow/deny)`
- **Tags/Labels**：server record 带 `tags`，task 通过 tags 表达意图（如 `["scm","search"]`），系统映射到 server_id

Phase B 可选实现：

1. 在 SMS 增加一个配置文件（或 DB 表）管理 `mcp_profiles`
2. 在 Web Admin 增加 profile 管理页面（可选）
3. task.config 支持：
   - `mcp.profile = "code-review"`
   - `mcp.profile_overrides`（可选，叠加/收窄）

### 为什么 Phase B 值得做

- 减少 task 配置成本
- 便于集中治理与审计（profile 变更可审批）
- 支持 A/B、灰度、环境隔离（dev/staging/prod 不同 profile）

## 运行时行为与失败策略

### 缺失环境变量 / server 不可用

你们当前实现里，环境变量引用解析失败会导致 `tools/list` 失败，注入为空（见 `client.rs` 解析逻辑与 `cchat_inject_mcp_tools` 的错误吞掉策略）。建议明确为官方语义：

- 单个 server 注入失败：只影响该 server，不影响其他 server
- 最终注入为空：ChatCompletion 仍可继续（模型看不到 MCP tools）

### 子集为空时

- 如果 task 明确要求 MCP（例如 `mcp.enabled=true` 且 default 不为空），但最终 effective 为空：建议仅记录 warning，并继续执行（除非未来提供“强制要求至少一个 server”开关）。

## 安全与合规

- 所有 MCP server 必须通过 SMS registry 治理登记（平台层 allowlist）
- task 层只能收窄 subset，不得扩大
- tools 的暴露永远受 `record.allowed_tools` 约束（server policy）
- 需要对注入结果与执行结果做审计日志（至少包含 server_id、tool_name、拒绝原因）

## 可观测性（推荐）

建议增加如下日志/指标（后续落地实现时补齐）：

- `mcp_injection_total{server_id, status}`：注入尝试计数（success/failed/denied）
- `mcp_list_tools_latency_ms{server_id}`：tools/list 延迟
- `mcp_effective_servers{task_id}`：本次 session 生效的 server 列表（日志字段）
- `mcp_denied_reason{server_id, reason}`：剔除原因分布（unknown_server / not_allowed / env_missing / list_tools_timeout 等）

## 迁移计划

- Phase A：先在少数 task 上配置 `Task.config`，验证 subset 能显著降低注入成本
- Phase B：引入 profile 后逐步替换 `default_server_ids` 为 profile 引用
- 最终：将 “subset 选择”作为平台标准能力，在 Web Admin 与 API 层提供可视化与治理入口

## 附：示例配置

### Task.config（示例）

```json
{
  "mcp.enabled": "true",
  "mcp.default_server_ids": "[\"gitlab\",\"fs\"]",
  "mcp.allowed_server_ids": "[\"gitlab\",\"fs\",\"duckduckgo-search\"]",
  "mcp.tool_allowlist": "[\"*\"]",
  "mcp.tool_denylist": "[\"delete_*\"]"
}
```

### 会话侧覆盖（示例，覆盖为更小子集）

- 若需要在某次会话中显式收窄，可在 chat session params 中设置（通过 `cchat_ctl_set_param`）：
  - `mcp.server_ids=["gitlab"]`
  - hostcall 会校验 `gitlab ∈ task.allowed_server_ids`，否则拒绝（`-EACCES`）
