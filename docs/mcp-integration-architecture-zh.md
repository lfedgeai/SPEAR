# MCP 集成架构（当前实现）

## 概述

本文基于本仓库当前代码，说明 Spear 中 MCP（Model Context Protocol）集成的现状：

- Spear 侧保存所有“允许连接”的 MCP Server 注册信息（含策略与凭证引用）。
- 在 Agent 层，将 MCP tools 以标准 `tools` 形式注入 Chat Completion，使 Agent 无需感知 MCP。
- MCP tools 会以标准 `tools` 形式注入 Chat Completion，并由 host 侧 auto tool-call loop 执行。

说明：当前实现没有单独的 `mcp_*` hostcall 接口；MCP 访问目前只通过 `cchat` 集成。

## 目标

- 将外部 MCP Server 的配置、策略、凭证统一沉淀在 Spear。
- 让 MCP tools 可被 Chat Completion 的 tool calling 直接使用，Agent 无感知。
- 默认安全（默认拒绝、allowlist、命名空间、预算限制、可审计）。
- 端到端支持 stdio transport（本地子进程）。
  - Registry schema 也允许 `streamable_http`，但 Spearlet 侧当前只支持 stdio 执行。

## 非目标

- 不在第一阶段实现“对外的通用 MCP 网关”（供 Spear 外部第三方客户端连接）。
- 不在第一阶段实现全部 MCP 能力（resources/prompts 可分期落地）。
- 不实现独立的 `mcp_open/list_tools/call_tool/close` hostcall API（后续工作）。
- 不在 Spearlet 侧执行 Streamable HTTP transport（后续工作）。
- 不允许绕过注册中心的任意子进程启动与任意网络连接。

## 可复用的 Spear 现有基础

- Chat Completion hostcall 已采用 fd 抽象（`cchat_create/write_msg/write_fn/ctl/send/recv/close`）。
- Spear 已支持 host 侧自动工具调用：当模型返回 `tool_calls` 时，host 执行工具，追加 `role=tool` 消息，并循环发送直到不再需要工具。

参考：

- Chat Completion hostcall 文档：[chat-completion-zh.md](./api/spear-hostcall/chat-completion-zh.md)
- 现有 auto tool-call loop 实现：[cchat.rs](../src/spearlet/execution/host_api/cchat.rs)

## 术语

- **MCP Server**：对外提供 `tools/list` 与 `tools/call` 的外部进程/服务。
- **Spear MCP 注册中心**：Spear 管理的 MCP Server 列表与治理策略。
- **Tool 注入**：将 MCP tools 转换为 OpenAI 兼容的 `tools`，参与 Chat Completion。
- **命名空间工具名**：对外暴露给模型的稳定 tool name。
  - 注入形态（当前）：`mcp__<base64(server_id)>__<base64(tool_name)>`
  - 路由兼容形态：`mcp.<server_id>.<tool_name>`

## 高层架构

### 组件

1. **MCP Registry（SMS，控制面）**
   - SMS 维护一份内存态的 MCP server registry。
   - SMS 支持从目录扫描 `*.toml`/`*.json` 启动加载（bootstrap）。
   - Spearlet 通过 gRPC 从 SMS 拉取 registry，并在本地缓存快照。

2. **Registry 同步（Spearlet，数据面）**
   - Spearlet 使用 watch+poll 的方式保持 registry 快照最新。

3. **Stdio MCP client（Spearlet，数据面）**
   - 每次 `tools/list` 与 `tools/call` 会按配置通过 stdio spawn MCP server 子进程，并使用 `rmcp` 进行通信。
   - 当前限制：仅支持 stdio（尽管 registry schema 允许 `streamable_http`）。

4. **Chat Completion 注入与执行（Spearlet，会话内）**
   - `cchat` host API 在 auto tool-call loop 内注入 MCP tools（`tools/list`）并执行 MCP tool call（`tools/call`）。

### 使用模式（当前）

- MCP tools 的注入与执行仅通过 `cchat` 的 auto tool-call loop 完成。

## MCP Server 注册中心

注册中心的权威存储在 SMS；Spearlet 不应持久化注册数据，只做缓存。

### 注册记录建议字段

推荐字段（满足生产治理的最小集合）：

- `server_id`（string，唯一且稳定）
- `display_name`（string）
- `transport`（`stdio` | `streamable_http` | `http_sse_legacy`）
- `stdio`（可选）：
  - `command`（string）
  - `args`（string[]）
  - `env`（map<string,string> 或引用）
  - `cwd`（string）
- `http`（可选）：
  - `url`（string）
  - `headers`（map<string,string> 或引用）
  - `auth_ref`（Spear 密钥/凭证引用）
- `tool_namespace`（string，默认 `mcp.<server_id>`）
- `allowed_tools`（pattern 列表，默认全拒绝）
- `approval_policy`（按工具或按 server：`never` | `always` | `policy`）
- `budgets`：
  - `tool_timeout_ms`
  - `max_concurrency`
  - `max_tool_output_bytes`

### 注册中心操作

控制面推荐以 CLI/gRPC/HTTP 管理接口提供：

- 注册 / 更新 / 删除 MCP Server
- 列出 MCP Server
- 健康状态与最近错误（可观测）

数据面访问方式：

- Spearlet 从 SMS 拉取并缓存注册中心记录（TTL + revision/version 校验）。
- Spearlet 支持惰性连接（首次使用再连），也可配置预热。

## Tool 命名与冲突规避

业界最佳实践是避免“平铺工具名空间”。Spear 对模型暴露的 MCP 工具使用稳定命名空间：

- 对模型暴露的工具名（注入后）：`mcp__<base64(server_id)>__<base64(tool_name)>`
- 内部路由：反解析为 `(server_id, tool_name)`；同时兼容 `mcp.<server_id>.<tool_name>` 形态。

这能显著降低重名冲突风险，并让审计与策略判断更直观。

## 在 Chat Completion 中注入 MCP tools

### 会话级启用哪些 MCP Server

推荐策略：

- 默认不启用任何 MCP Server。
- 通过 chat session params 显式启用（复用现有 `cchat_ctl_set_param` 链路）：
  - `mcp.enabled`: boolean
  - `mcp.server_ids`: string[]
  - 可选：`mcp.tool_allowlist`: string[] patterns（会话级进一步收敛）

当前实现说明：

- Task 级默认值可能会在创建 chat session 时，从 `Task.config` 自动写入到 session params。
- 暂不支持通过 invocation metadata/context_data 做 per-invocation 覆盖（例如在 invocation metadata 里传 `mcp.server_ids`）。

这样可以保持 WASM API 面稳定，并且让 MCP 的启用更可控、更可审计。

### 让用户选择 MCP 工具（best practice）

业界通常不会把“给模型一大堆工具自由挑选”当成用户选择方案，而是用分层 allowlist + 作用域收敛来实现“用户选择哪些 MCP 工具可被调用”。

- 第 1 层（平台/管理员）：只有通过审核的 MCP Server 才能进入注册中心，默认拒绝。
- 第 2 层（租户/用户）：用户启用集成（按 server 维度），通常默认只开只读工具子集。
- 第 3 层（会话/请求）：本次运行只启用少量 server_ids，并用 pattern 进一步收敛工具范围。

推荐会话参数（通过现有 `cchat_ctl_set_param` 写入）：

- `mcp.enabled`: boolean
- `mcp.server_ids`: string[]（启用 MCP 时建议必填）
- `mcp.task_tool_allowlist`: string[] patterns（task 级；host 注入；WASM 不可写）
- `mcp.task_tool_denylist`: string[] patterns（task 级；host 注入；WASM 不可写）
- `mcp.tool_allowlist`: string[] patterns（可选）
- `mcp.tool_denylist`: string[] patterns（可选）

推荐的 tool calling 策略（作为请求参数透传给上游模型）：

- `tool_choice = "none"`：用户显式禁止本次请求调用任何工具。
- `tool_choice = "auto"`：默认；模型只能在“已过滤后的工具集合”里选择。
- `tool_choice = {"type":"function","function":{"name":"mcp__...__..."}}`：用户点选了具体工具，强制模型使用该工具（tool name 需与本次注入的名称一致）。

产品交互建议：

- 优先按“集成/能力包”（server 维度）呈现，再在其内区分只读与写入工具。
- 控制每次注入给模型的工具数量；写入类工具建议走额外审批或显式用户操作。

### 生成上游请求

在构造 Chat Completions 请求时，Spear 生成：

- `tools = wasm_tools + mcp_tools`
  - wasm_tools：通过 `cchat_write_fn` 注册的工具
  - mcp_tools：通过 MCP `tools/list` 拉取并经过过滤：
    - 注册中心 `allowed_tools`
    - 会话级 allowlist
    - 全局治理策略

补充（当前代码）：

- `mcp.*` 会话参数属于 host 内部控制参数，会被写入会话参数 map，但不会透传到上游模型请求 body。

### 执行 tool calls

复用现有的自动 tool-call 闭环：

1. 带注入后的 `tools` 发送 Chat Completion。
2. 若模型返回 `tool_calls`：
   - 逐个调用：
     - 命中 WASM 工具：根据 `fn_offset` 调用 WASM。
     - 命中 `mcp__...__...`（或兼容形态 `mcp.<server_id>.<tool_name>`）：调用 MCP `tools/call`。
   - 将结果以 `role=tool` 且携带正确 `tool_call_id` 的消息追加回会话。
3. 循环直到模型不再请求工具或触发预算上限。

预算与安全限制应对 WASM/MCP 两类工具一视同仁：

- `max_iterations`
- `max_total_tool_calls`
- `max_tool_output_bytes`
- 单工具超时

## MCP hostcall（可编程 API）

本节为后续扩展预留。当前实现并未暴露 `mcp_*` hostcall 接口；MCP 访问目前只通过 `cchat` 集成。

## 安全与治理

推荐 best practice：

- **默认拒绝**：未显式启用则不可用。
- **工具 allowlist**：注册中心 `allowed_tools` 为第一道门，会话级可进一步收敛。
- **命名空间**：避免冲突并便于审计。
- **凭证间接引用**：敏感信息进 Spear secret store，通过 `auth_ref` 引用。
- **审批机制**：敏感工具按策略触发审批（人工或程序化）。
- **网络管控**：对 Streamable HTTP 的出网做限制。
- **stdio 规范**：stdio 协议流不得混入日志；日志应走 stderr。

## 可观测性

最小推荐指标：

- Server 维度：连接状态、重连次数、最近错误、平均延迟
- Tool 维度：调用次数、错误率、超时率、p50/p95 延迟、输出字节数
- Session 维度：总工具调用次数、迭代次数、预算触发次数

审计日志（按策略启用）：

- `request_id`、`session_id`、`server_id`、`tool_name`、时间戳、状态
- 参数脱敏（可配置）

## 故障处理

- Server 不可用：
  - tool call 返回结构化错误 JSON，并以 `role=tool` 追加回会话。
- 输出超限：
  - 截断并明确标记，同时携带结构化错误字段。
- 模型陷入工具循环：
  - 严格执行 `max_iterations` 与 `max_total_tool_calls`。

## 建议的落地节奏

1. 第 1 阶段：注册中心 + tool 注入 + 在 Chat Completion loop 中执行 MCP tools。
2. 第 2 阶段：补齐 `mcp_open/list_tools/call_tool/close` hostcall。
3. 第 3 阶段：按需支持 resources/prompts。
4. 第 4 阶段：大型部署可选引入 MCP 网关模式。

## 工程化设计细节

本节面向“准备开始实现”的工程化落地，给出推荐的代码结构、数据结构、关键流程、并发与预算控制、错误模型、测试与可观测等细节。

### 代码结构（当前）

当前实现主要分布在以下位置（code map）：

- SMS
  - 从目录 bootstrap MCP configs：[`src/apps/sms/main.rs`](../src/apps/sms/main.rs) 与 [`src/sms/service.rs`](../src/sms/service.rs)（`bootstrap_mcp_from_dir`）
  - MCP registry gRPC service：[`src/sms/service.rs`](../src/sms/service.rs)（`McpRegistryService`）
  - Web Admin MCP 接口：[`src/sms/web_admin.rs`](../src/sms/web_admin.rs)
- Spearlet
  - Registry 同步（watch + 定期 refresh）：[`src/spearlet/mcp/registry_sync.rs`](../src/spearlet/mcp/registry_sync.rs)
  - Stdio MCP client 封装（rmcp）：[`src/spearlet/mcp/client.rs`](../src/spearlet/mcp/client.rs)
  - Tool 命名/过滤/路由解析：[`src/spearlet/mcp/policy.rs`](../src/spearlet/mcp/policy.rs)
  - Task 级子集策略解析：[`src/spearlet/mcp/task_subset.rs`](../src/spearlet/mcp/task_subset.rs)
  - cchat 注入与执行：[`src/spearlet/execution/host_api/cchat.rs`](../src/spearlet/execution/host_api/cchat.rs)

### 配置与注册中心存储

#### 1) SMS 配置入口（registry file）

由于注册中心权威存储在 SMS，推荐由 SMS 负责加载 MCP server 注册文件并写入（upsert）注册中心。

SMS 启动时加载 registry file，并将内容 upsert 到注册中心。

#### 1.1) 从可配置目录加载 MCP Server 配置（推荐）

如果你希望“把 MCP 配置放在一个目录里，SMS 自动发现所有支持的 MCP server 配置信息”，这也是业界非常常见的治理方式（按文件拆分、便于 code review、便于灰度与回滚）。

设计建议：

- 仅在 SMS 的 config 中提供一个目录路径（或 CLI flag）。
- SMS 启动时扫描该目录下的配置文件，并将每个文件解析出的 server record upsert 到注册中心（以 `server_id` 为主键）。
- 可选 reload：SIGHUP、定时轮询、或 Web Admin/CLI 触发重新扫描。
- Spearlet 不从磁盘加载注册信息，仅从 SMS 拉取。

推荐配置项命名（示例）：

- SMS：
  - CLI：`--mcp-dir <DIR>`
  - ENV：`SMS_MCP_DIR=<DIR>`
  - 配置文件：`[mcp]\ndir = "..."`（sms 配置）

目录扫描规则建议：

- 只读取 `*.toml`、`*.json`（实现可先支持一种）。
- 不递归或可配置是否递归（默认不递归）。
- 忽略隐藏文件与临时文件（如 `.*`、`~`、`.swp`）。
- 默认拒绝跟随 symlink（防止目录穿越与意外引用）。

文件格式建议：两种都可以，优先推荐 “单文件单 server” 的 schema（更利于拆分与审计）。

示例（TOML，单文件单 server）：

```toml
server_id = "fs"
display_name = "Filesystem"
transport = "stdio"
tool_namespace = ""
allowed_tools = ["read_*", "search_*"]

[stdio]
command = "npx"
args = ["-y", "@modelcontextprotocol/server-filesystem", "./"]

[budgets]
tool_timeout_ms = 30000
max_concurrency = 8
max_tool_output_bytes = 65536
```

环境变量与引用：

- `stdio.env` 支持在 value 中使用环境变量引用：
  - 必填：`${ENV:VAR_NAME}`
  - 带默认值：`${ENV:VAR_NAME:-default_value}`
- `stdio.env_from`（仅目录加载的配置支持）是“透传这些环境变量”的语法糖：
  - `env_from = ["API_TOKEN"]` 等价于 `env.API_TOKEN = "${ENV:API_TOKEN}"`
- 当环境变量缺失时的行为：
  - 如果必填引用（无默认值）无法解析，Spearlet 会认为 `tools/list` 失败，并在该 chat session 中不注入这个 MCP server 的任何 tools（等价于“未 load”）。
  - 如果提供了默认值，则使用默认值继续注入。
- GitLab MCP 配置示例：[config/sms/mcp.d/gitlab.toml](../config/sms/mcp.d/gitlab.toml)

合并策略建议：

- 默认 upsert（同 `server_id` 覆盖）。
- 冲突检测（可选）：同一个 `server_id` 被多个文件声明时，按文件名排序最后生效，且输出告警与来源文件。
- 可选 `--dry-run`（仅校验与展示 diff）。
- 可选 `--strict`（存在未知字段/校验失败则拒绝加载）。

#### 2) Cluster 模式（SMS 注册中心）

推荐由 SMS 提供注册中心的 CRUD 与版本号（或 revision），Spearlet 缓存读取：

- 使用 gRPC `McpRegistryService`（proto）
  - `ListMcpServers`（list + revision）
  - `WatchMcpServers`（server-side streaming）
  - `UpsertMcpServer` / `DeleteMcpServer`（管理员权限）

Spearlet data-plane 只需要只读能力即可。

#### 2.0) Spearlet 拉取注册中心数据的约定（建议）

Spearlet 应通过“带 revision 的 list API”从 SMS 拉取注册数据。

- RPC：`McpRegistryService.ListMcpServers`
- 响应：

```json
{
  "revision": 123,
  "servers": [
    {"server_id":"fs","transport":"stdio","allowed_tools":["read_*"]}
  ]
}
```

缓存规则建议：

- 按 `revision` + TTL 缓存。
- revision 未变化则跳过工具注入列表重建。

#### 2.0.1) Spearlet 观察 SMS 更新（best practice）

要保证 SMS 侧 registry 更新后 Spearlet 能及时感知并更新缓存，业界通常采用“推+拉双通道”：

- 推（主通道）：gRPC server-side streaming 的 watch 变更流，通知 Spearlet 有更新。
- 拉（兜底）：定期调用 `ListMcpServers`，保证最终一致。

原因是：watch 长连接可能断开/丢事件；纯轮询要么延迟高，要么 QPS 成本高。

推荐接口形态：

- Watch RPC（gRPC server-side streaming）：
  - `McpRegistryService.WatchMcpServers(WatchMcpServersRequest{ since_revision }) -> stream`
  - 事件 payload 保持轻量（不要推全量配置）：

```json
{"revision": 124, "upserts": ["fs"], "deletes": ["jira"]}
```

- Poll RPC（现有 list）：
  - `McpRegistryService.ListMcpServers` 返回 `{revision, servers}`。

Spearlet 缓存更新流程建议：

1. 启动时全量拉取 list，保存 `revision` 与快照。
2. 建立 watch 流，从 `since_revision=revision` 开始订阅。
3. 收到 watch 事件：
   - 更新本地 `target_revision`。
   - 触发一次 refresh（全量拉取或增量拉取均可）。
4. 同时运行低频 poll（带 jitter，例如 30s~120s）：
   - 若 SMS `revision` > 本地 `revision`，则 refresh。

可靠性与安全建议：

- watch 重连使用指数退避并加 jitter。
- watch 返回 `failed_precondition`（since_revision 过旧）或 stream 异常结束时，触发一次全量 `ListMcpServers` resync，并从最新 revision 重新建立 watch。
- 缓存更新采用“构建新快照后一次性替换”（atomic swap）。
- refresh 失败时继续用旧快照对外服务，并将 registry cache 标记为 degraded。
- revision 变化时驱动依赖失效：
  - tool 注入集合重建
  - server transport 配置变化则重连/重启对应 MCP 连接
  - tools/list 缓存按 `server_id` 失效

### Rust MCP client 库选择

在 Rust 侧，best practice 是：如果官方/成熟 SDK 能满足需求，优先使用 SDK，而不是手写 JSON-RPC + transport。

- 优先方案：使用官方 Rust MCP SDK 的 `rmcp`：https://github.com/modelcontextprotocol/rust-sdk
  - 优点：协议类型更完整、stdio 子进程 transport 等基础能力更成熟，减少自研协议细节 bug。
  - 代价：新增依赖面；在 Streamable HTTP 等 transport 上可能仍需自定义 glue（视 SDK 支持情况）。
- 备选方案：基于现有依赖（`tokio`、`serde_json`、`reqwest`）实现 Spear 自用的最小 MCP client 子集（仅 `tools/list` + `tools/call`），适用于：
  - 功能范围受控、只需要 tools 能力。
  - 需要更强的 IO/日志/资源上限控制。

如果采用 `rmcp`，建议在 `Cargo.toml` 中 pin 版本与 features，并将所有 MCP 逻辑隔离在 `src/spearlet/mcp/` 边界内。

### 实现说明

- Proto 与 SMS gRPC 服务定义：[mcp_registry.proto](../proto/sms/mcp_registry.proto)
- SMS 服务实现：[service.rs](../src/sms/service.rs)
- Spearlet registry 同步（watch+poll 缓存）：[registry_sync.rs](../src/spearlet/mcp/registry_sync.rs)
- Chat Completion MCP tool 注入与执行：[cchat.rs](../src/spearlet/execution/host_api/cchat.rs)
- MCP client 封装（当前仅支持 stdio）：[client.rs](../src/spearlet/mcp/client.rs)

会话级参数（每个 chat session）：

- `mcp.enabled`: bool
- `mcp.server_ids`: string[]
- `mcp.tool_allowlist`: string[] patterns
- `mcp.tool_denylist`: string[] patterns

#### 2.1) Web Admin 页面：注册与治理入口

为便于运维与自助接入，推荐在 SMS 的 Web Admin 增加一个独立 Tab（例如 `MCP Servers`），用于管理外部 MCP Server 注册信息。

页面能力建议：

- 列表：server_id、transport、状态（Connected/Degraded/Down）、最近错误、工具数量、更新时间
- 创建/编辑：表单编辑 stdio/http 配置、allowed_tools、budgets、审批策略
- 删除：需要二次确认
- 连接测试（可选）：触发一次 `tools/list` 并显示结果摘要
- 工具预览（可选）：展示经过 allowlist 过滤后的 tools 列表（含命名空间后的名称）
- 从文件导入（可选）：上传 registry 文件，服务端校验后 upsert

当前实现中，后端接口沿用 `/admin/api` 体系（在 sms Web Admin gateway 内实现）：

- `GET /admin/api/mcp/servers`
- `GET /admin/api/mcp/servers/{server_id}`
- `POST /admin/api/mcp/servers`（upsert）
- `DELETE /admin/api/mcp/servers/{server_id}`（删除）

### 函数/方法级细节（建议）

本小节给出更贴近 Rust 工程实现的函数边界建议，用于降低落地时的歧义。

#### SMS：registry store 与 service

核心存储 trait：

```rust
pub trait McpRegistryStore {
    fn revision(&self) -> u64;
    fn list_servers(&self) -> Result<Vec<McpServerRecord>, RegistryError>;
    fn get_server(&self, server_id: &str) -> Result<Option<McpServerRecord>, RegistryError>;
    fn upsert_server(&self, record: McpServerRecord) -> Result<u64, RegistryError>;
    fn delete_server(&self, server_id: &str) -> Result<u64, RegistryError>;
}
```

业务层：

```rust
pub struct SmsMcpRegistryService {
    store: Arc<dyn McpRegistryStore + Send + Sync>,
}

impl SmsMcpRegistryService {
    pub fn list(&self) -> Result<(u64, Vec<McpServerRecord>), RegistryError>;
    pub fn get(&self, server_id: &str) -> Result<Option<McpServerRecord>, RegistryError>;
    pub fn upsert(&self, record: McpServerRecord) -> Result<u64, RegistryError>;
    pub fn delete(&self, server_id: &str) -> Result<u64, RegistryError>;
    pub fn import_from_file(&self, path: &str, mode: ImportMode) -> Result<u64, RegistryError>;
    pub fn validate(&self, record: &McpServerRecord) -> Result<(), RegistryError>;
}
```

gRPC handler（形态建议，函数名不做强约束）：

```rust
async fn list_mcp_servers(...) -> Result<ListMcpServersResponse, Status>;
async fn watch_mcp_servers(...) -> Result<tonic::Response<impl Stream<Item = Result<WatchMcpServersResponse, Status>>>, Status>;
async fn upsert_mcp_server(...) -> Result<UpsertMcpServerResponse, Status>;
async fn delete_mcp_server(...) -> Result<DeleteMcpServerResponse, Status>;
```

#### Spearlet：registry client 与 cache

Registry client：

```rust
pub struct McpRegistryClient {
    sms_base_url: String,
    http: reqwest::Client,
}

impl McpRegistryClient {
    pub async fn list_servers(&self) -> Result<(u64, Vec<McpServerRecord>), RegistryError>;
}
```

缓存：

```rust
pub struct McpRegistryCache {
    revision: u64,
    servers: Vec<McpServerRecord>,
    expires_at_ms: u64,
}

impl McpRegistryCache {
    pub async fn get_or_refresh(
        &mut self,
        client: &McpRegistryClient,
        ttl_ms: u64,
    ) -> Result<(u64, Vec<McpServerRecord>), RegistryError>;
}
```

### 核心数据结构（建议）

#### Registry

- `McpServerRecord`
  - `server_id`, `transport`, `stdio/http` 配置
  - `tool_namespace`（默认 `mcp.<server_id>`）
  - `allowed_tools`（pattern）
  - `approval_policy`
  - `budgets`

#### Client & Tool

- `McpToolDescriptor`
  - `name`, `description`, `input_schema`
  - `server_id`
  - `namespaced_name`（对模型暴露用）

- `McpCallRequest`
  - `server_id`, `tool_name`
  - `arguments`（JSON object）
  - `timeout_ms`, `max_output_bytes`

### MCP client：连接、发现、调用

#### 连接池（pool）

建议按 `server_id` 维护一条“可复用连接”的状态机：

- stdio：保持子进程常驻（可配置 idle 超时回收），断开自动重启
- HTTP：保持会话信息（如有），断开自动重连/退避

每个 server_id 维度实现：

- `Semaphore` 控制并发（`max_concurrency`）
- `timeout` 包裹每次 `tools/call`
- 记录健康状态：`Connected/Degraded/Down` + last_error

#### tools/list 缓存

best practice：不要每次 `cchat_send` 都去 list_tools。

- 缓存 key：`(server_id, policy_hash)`
- TTL：例如 30s~5min（可配置）
- 失败缓存：短 TTL（例如 1~5s）避免雪崩

### Chat Completion：用户选择、注入、路由、执行

#### 会话参数（session params）

通过 `cchat_ctl_set_param` 写入，建议支持：

- `mcp.enabled`: bool
- `mcp.server_ids`: string[]
- `mcp.task_tool_allowlist`: string[] patterns（task 级；host 注入；WASM 不可写）
- `mcp.task_tool_denylist`: string[] patterns（task 级；host 注入；WASM 不可写）
- `mcp.tool_allowlist`: string[] patterns
- `mcp.tool_denylist`: string[] patterns
- `tool_choice`: `none | auto | {"type":"function",...}`（直接透传上游模型）

#### 注入算法（建议）

输入：`session_params + registry_records + cached_tools`。

1. 若 `mcp.enabled != true` 或 `mcp.server_ids` 为空：不注入 MCP tools。
2. 对每个 server_id：
   - 读取 registry record
   - list_tools（走缓存）
   - 将工具名映射为注入后的 OpenAI tool name：`mcp__<base64(server_id)>__<base64(tool_name)>`
   - 用 registry allowlist + session allowlist/denylist 过滤
3. 将过滤后的 MCP tools 追加到 `tools`（与 WASM tools 合并）。

#### 路由与执行

在现有 auto tool-call loop 中加入一个统一分发器：

- `tool_name` 如果命中 WASM 工具：走 `fn_offset`
- `tool_name` 如果匹配 `mcp__...__...`（或兼容形态 `mcp.<server_id>.<tool_name>`）：
  - 解析出 `server_id/tool_name`
  - 解析 `arguments` 为 JSON object（若解析失败返回结构化错误）
  - 调用 MCP `tools/call`

输出建议统一为 JSON 字符串（成功或失败），再以 `role=tool` 追加到 messages。

### MCP hostcall：fd 模型与 ABI（后续工作）

#### fd 类型

当前代码尚未实现 `mcp_*` hostcall。若后续引入，可考虑为 MCP 引入新的 `FdKind::McpSession`（或复用 `FdKind::Generic` + tag），内部状态保存：

- `server_id`
- 可选：连接句柄引用（连接池 key）
- 选配：会话级 policy（allowlist/denylist/预算覆盖）

#### ABI（建议与现有 cchat 保持一致）

所有 API 使用 `(ptr,len)` 输入字符串或 JSON，并通过 `(out_ptr, out_len_ptr)` 输出：

- `mcp_open(server_id_ptr, server_id_len) -> mcp_fd`
- `mcp_list_tools(mcp_fd, out_ptr, out_len_ptr) -> rc`
- `mcp_call_tool(mcp_fd, tool_name_ptr, tool_name_len, args_ptr, args_len, out_ptr, out_len_ptr) -> rc`
- `mcp_close(mcp_fd) -> rc`

错误码建议统一 `-errno`，与 fd 子系统风格一致。

### 错误模型（建议）

对“工具执行失败”不要直接让整次 Chat Completion 失败，建议返回结构化 tool 输出并继续让模型决定下一步：

```json
{"error": {"code": "mcp_unavailable", "message": "...", "retryable": true}}
```

常见错误码分类：

- `mcp_unavailable`：连接失败/服务不可用
- `mcp_timeout`：单次调用超时
- `mcp_invalid_arguments`：arguments JSON 无法解析或不符合 schema
- `mcp_policy_denied`：被 allowlist/denylist/审批策略拒绝
- `mcp_output_too_large`：输出超过上限（可截断并标记）

### 并发、预算与资源控制

建议同时实现三类预算：

- 会话预算：`max_iterations`、`max_total_tool_calls`、`max_tool_output_bytes`
- server 预算：`max_concurrency`、`tool_timeout_ms`
- 全局预算：总并发上限、每个 Spearlet 的子进程数量上限

stdio 子进程建议加入：

- 最大子进程数限制（避免被大量会话击穿）
- 空闲回收（idle timeout）

### 可观测性与审计（落点）

建议在 Spearlet 打点：

- `mcp_server_connect_total{server_id}`
- `mcp_tool_call_total{server_id,tool}`
- `mcp_tool_call_error_total{server_id,tool,code}`
- `mcp_tool_call_latency_ms{server_id,tool}`
- `mcp_tool_call_output_bytes{server_id,tool}`

审计日志建议包含：`request_id/session_id/server_id/tool_name/status`，并支持参数脱敏。

### 测试计划（建议最小集）

- 单元测试
  - tool 名称路由解析（`mcp__...__...`，以及兼容形态 `mcp.<server_id>.<tool_name>`）
  - allowlist/denylist pattern 匹配
  - tool 注入过滤算法
- 集成测试（tokio）
  - stdio：用一个“假 MCP server”子进程模拟 `tools/list` 与 `tools/call`
  - Streamable HTTP：若后续实现该 transport，再补齐本地 mock
  - cchat auto tool-call loop：注入 MCP tools，确保循环能 append `role=tool`
- 回归测试
  - MCP 不启用时行为不变
  - tool_choice 为 `none/auto/force` 的兼容性
