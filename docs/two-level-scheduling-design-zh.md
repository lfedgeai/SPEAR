# 双层调度（Two-Level Scheduling）方案设计（SMS 控制面 + Spillback）

## 概述

本文档描述 SPEAR 的双层调度方案：

- 第一层（集群级 / Cluster-level）：由 SMS 作为控制面（Control Plane）负责“选节点（placement）”。
- 第二层（节点内 / Node-level）：由 Spearlet 在本机完成“选实例（instance scheduling）+ 并发控制 + 冷启动管理”。
- Spillback：当所选节点无法接住请求（不可用/过载/资源不满足/排队过长）时，快速重定向到其他节点。

目标是让用户不再需要手选节点，同时保持低延迟与可演进性，并在设计上为 SMS 水平扩展预留空间（但本阶段不实现水平扩展）。

## 现状与约束

### 已存在能力（与本方案直接相关）

- SMS 节点注册与心跳：Spearlet 向 SMS 注册与心跳上报，SMS 维护 Node 列表。
  - 参考：[registration.rs](../src/spearlet/registration.rs)
  - 参考：[node_service.rs](../src/sms/services/node_service.rs)
- SMS 节点资源信息（CPU/内存/磁盘/负载等）管理与查询：
  - 参考：[resource_service.rs](../src/sms/services/resource_service.rs)
- Spearlet 节点内执行管理与实例级调度：
  - 执行入口：TaskExecutionManager::submit_execution（接收 InvokeFunctionRequest 并进入异步执行循环）
    - 参考：[manager.rs](../src/spearlet/execution/manager.rs#L232-L340)
  - 节点内实例调度器：InstanceScheduler（RoundRobin/LeastConnections 等）
    - 参考：[scheduler.rs](../src/spearlet/execution/scheduler.rs)

### 当前缺口

- 缺少“集群级选址”：调用时需要用户选节点。
- 缺少“spillback 语义”：当节点接不住请求时没有统一的重选机制。

## 设计目标（Goals）

- 在不改变 Spearlet 核心执行模型的前提下，引入 SMS 控制面的 placement 能力。
- 提供明确的 spillback 机制与重试预算，保证失败快速恢复且不会无限重试。
- 调度决策可解释：返回 reason/score，便于排障与评估策略。
- 设计上支持未来 SMS 水平扩展：调度服务尽量无状态、可分片、可缓存、可引入 lease。

## 非目标（Non-goals）

- 本阶段不实现 SMS 的水平扩展与分片部署。
- 本阶段不将 SMS 变成数据面（不做请求代理转发），默认仍然是客户端直连 Spearlet。
- 本阶段不引入强一致的全局资源账本。

## 总体架构

### 组件角色

- SMS（控制面）：
  - 维护节点与资源近似视图。
  - 提供 placement API：输入“本次 invocation 的需求/约束”，输出“候选节点列表（按优先级排序）+ 决策解释”。
- Spearlet（数据面 + 本地最终裁决）：
  - 接收执行请求并进行节点内实例调度与资源控制。
  - 当本机过载/无法接收时，返回可识别的错误码，触发 spillback。
- 编排层（Client/SDK 或 Admin BFF）：
  - 调用 SMS 进行 placement。
  - 按候选节点顺序调用 Spearlet（直连或内网转发）。
  - 失败时进行 spillback（换节点重试）。

### 关键思想

- 全局（SMS）视图允许“最终一致/近似”，只负责快速筛选出“高概率可行”的节点。
- 本地（Spearlet）视图准确且是最终裁决：能接就接，不能接就快速拒绝。
- spillback 把“全局近似导致的选错”成本控制在一次快速重试内。

### 时序（简化）

```
Client/Orchestrator     SMS (Placement)                     Spearlet (Node)
  |  PlaceInvocation()      |                                   |
  |------------------------>|                                   |
  |   candidates[]          |                                   |
  |<------------------------|                                   |
  |  InvokeFunction(node#1) |---------------------------------->|  submit_execution()
  |                         |                                   |  node-level schedule
  |                         |                                   |  (accept or reject)
  |  result / overloaded    |<----------------------------------|
  |  spillback to node#2    |---------------------------------->|  ...
  |  result                 |<----------------------------------|
```

## Admin 提交任务：BFF 模式（推荐用于 Spearlet 不可外网访问）

当 Admin Page 位于外网/浏览器环境，无法直连 Spearlet（内网地址、无公网入口、跨域/证书/鉴权复杂）时，业界 best practice 是引入 BFF（Backend For Frontend）作为“前端专用后端”。

### 目标

- Admin Page 对外只依赖一个稳定入口（BFF）。
- SMS 仍然只做控制面（placement/元数据），不承载 invoke 数据面流量。
- BFF 在内网调用 Spearlet gRPC 执行，并实现 spillback。

### 组件与网络边界

- Admin Page（浏览器）：只访问 BFF 的 HTTP/JSON。
- Admin BFF（数据面，仅面向 admin）：
  - 对外：HTTP（同域、简化 CORS）。
  - 对内：
    - 调 SMS（placement 或临时用 list_nodes + list_resources 组装 placement）。
    - 调 Spearlet gRPC FunctionService.InvokeFunction。
- SMS（控制面）：继续维护节点/资源视图，不代理转发 InvokeFunction。
- Spearlet（执行面）：只需内网可达（BFF 能连）。

### 部署形态（两种，推荐先 1 后 2）

#### 形态 1：复用 SMS WebAdminServer 作为 BFF（最快落地）

- WebAdminServer 已经是“给管理 UI 提供 HTTP”的服务端组件，可直接扩展 `/api/admin/*` 作为 BFF API。
- 对应实现位置：
  - Web 管理服务： [web_admin.rs](../src/sms/web_admin.rs)
- 注意：这会让 SMS 进程承载“admin 数据面”，但仍可以保持“SMS 不代理 invoke payload”的原则（SMS 只做 placement；BFF 负责内网调用 Spearlet）。

#### 形态 2：独立 admin-bff 服务（二进制）（最佳长期形态）

- 将 BFF 从 SMS 拆出为独立服务：`admin-bff`。
- 好处：职责清晰、可独立限流/扩缩容/发布；不会把控制面故障与 admin 数据面耦合。
- 设计上两种形态的 API 与内部模块保持一致，便于平滑迁移。

### 为什么 BFF 需要走 Spearlet gRPC（而不是 Spearlet HTTP Gateway）

- 当前 Spearlet HTTP gateway 的 `/functions/execute` 仍是 TODO，占位实现。
  - 参考：[http_gateway.rs](../src/spearlet/http_gateway.rs#L496-L514)
- 因此 BFF 的执行面调用建议直接走 Spearlet gRPC：`FunctionService.InvokeFunction`。

### BFF 对外 API（建议）

#### 提交执行

- `POST /api/admin/executions`

建议支持幂等与重试控制：

- 请求头：`X-Request-Id`（客户端生成；相同请求重试时保持不变）
- 请求头：`X-Total-Timeout-Ms`（端到端预算，BFF 用于切分 per-node timeout 与总超时）

请求（建议字段，映射到 spea​​rlet InvokeFunctionRequest）：

```json
{
  "task_id": "task-xxx",
  "artifact_spec": {
    "artifact_id": "artifact-xxx",
    "version": "v1",
    "artifact_type": "wasm",
    "metadata": {}
  },
  "execution_mode": "sync|async",
  "wait": true,
  "input": {"any": "json"},
  "node_selector": {"gpu": "true"},
  "spillback": {"max_attempts": 2, "per_node_timeout_ms": 5000}
}
```

字段说明（与调度/执行相关）：

- `node_selector`：调度硬约束（标签/能力）。BFF 透传给 SMS placement。
- `spillback.max_attempts`：最多尝试的节点数（含第一次）。建议默认 2。
- `spillback.per_node_timeout_ms`：每个节点尝试的超时。建议短（例如 3~10s），避免卡死。
- `execution_mode + wait`：同步/异步语义。

注意：当前执行链路中 `ExecutionContext.payload/headers/context_data` 仍标注 TODO 提取，若你期望把 `input` 传给运行时，需要后续在 Spearlet 侧补齐“从 InvokeFunctionRequest 提取 payload”的落地。
  - 参考：[submit_execution](../src/spearlet/execution/manager.rs#L232-L340)

响应（建议字段）：

```json
{
  "execution_id": "exec-xxx",
  "decision_id": "dec-xxx",
  "selected_node_uuid": "node-1",
  "attempts": 1,
  "status": "completed|running|failed",
  "result_base64": "...",
  "error": null
}
```

#### 查询状态与取消

- `GET /api/admin/executions/{execution_id}`
- `POST /api/admin/executions/{execution_id}:cancel`

说明：BFF 需要维护 `execution_id -> node_uuid` 的索引（最小实现可用内存 Map，后续可替换成 KV/DB）。

建议额外提供：

- `GET /api/admin/executions/{execution_id}/debug`：返回 decision_id、节点列表、最后一次错误与分类（便于排障）。

### BFF 内部模块设计（建议，方法级）

#### 核心对象

```text
AdminBffService
  - placement_client: SmsPlacementClient
  - node_pool: NodeClientPool
  - executor: SpillbackExecutor
  - index: ExecutionIndex
  - policy: SpillbackPolicyDefaults
```

#### ExecutionIndex（执行路由索引）

用途：后续 status/cancel 必须路由回“实际执行节点”。

- Key: `execution_id`
- Value: `{ node_uuid, node_grpc_addr, decision_id, created_at, last_status }`
- 生命周期：
  - Sync 执行：可短 TTL（例如 10~30 分钟）
  - Async/LongRunning：需要更长 TTL 或持久化（后续可落到 SMS/KV/DB）

#### NodeClientPool（节点连接池）

用途：避免每次 spillback 都重新建连。

- `get_function_client(node_grpc_addr) -> FunctionServiceClient<Channel>`
- 连接健康：连接失败时标记短期不可用并触发重建。
- 安全：可选启用 mTLS；至少做地址 allowlist（只允许 SMS 返回的内网网段/注册节点）。

#### SpillbackExecutor（调度执行器）

关键方法：

```text
submit_execution(req) -> resp
  request_id = header.X-Request-Id or generate
  decision = placement_client.place(req, request_id)
  return invoke_with_spillback(req, decision)

invoke_with_spillback(req, decision)
  attempts = 0
  for node in decision.candidates[0..max_attempts]:
    attempts += 1
    r = invoke_once(node, req, timeout=per_node_timeout)
    if r.success:
      index.put(r.execution_id, node, decision.decision_id)
      placement_client.report_outcome(SUCCESS)
      return r
    if is_spillbackable(r.error):
      placement_client.report_outcome(map_error(r.error))
      continue
    placement_client.report_outcome(ERROR)
    return r
  return last_error

invoke_once(node, req)
  client = node_pool.get_function_client(node.grpc_addr)
  return client.invoke_function(InvokeFunctionRequest{...})
```

### 幂等、重试与“可能已执行”语义（工程化必选）

#### 幂等键（request_id）

- BFF 入口必须要求或生成 `request_id`（例如 `X-Request-Id`）。
- BFF 建议将 `request_id` 映射到 Spearlet 的 `InvokeFunctionRequest.execution_id`，使得“同一请求重试”在 Spearlet 侧可去重。
  - 参考：Spearlet 会在 `execution_id` 为空时自行生成，因此 BFF 传入能提升幂等性。
  - 参考：[invoke_function](../src/spearlet/function_service.rs#L382-L420)

#### 重试边界

- 只对“确定未开始执行”的失败做自动重试（例如连接失败/UNAVAILABLE、明确的 OVERLOADED）。
- 对 `DEADLINE_EXCEEDED` 需要区分阶段：
  - 连接/握手阶段超时：可重试（更像“未提交成功”）。
  - 已提交后等待结果超时：默认不重试，避免重复执行。

#### 结果一致性

- Sync 模式：若发生“可能已执行”的超时，BFF 返回 `UNKNOWN` 并带上 `execution_id`，前端需走 status 查询收敛。
- Async 模式：BFF 返回 `execution_id` 即可，结果通过 status 轮询或事件流获取。

### 限流、背压与资源保护（工程化必选）

#### BFF 入口限流

- 按 admin 用户/租户限流：`rate_limit(user)`。
- 全局并发限制：`max_inflight_executions`（避免 admin 打爆内网执行面）。

#### BFF → Spearlet 背压

- 对每个 node 维护 `inflight_per_node` 上限，超过则优先 spillback 或快速失败。
- 对 OVERLOADED 节点做短期熔断（cooldown），避免羊群效应。

#### 体积限制

- 限制请求体大小与字段白名单，避免把“大 payload”经由 BFF 传入执行面。
- 若需要大输入，推荐先上传到对象存储（或 SMS 文件服务），BFF 只传引用。

### 安全与鉴权（工程化必选）

- 外部鉴权：BFF 对 admin 登录态进行校验（cookie/JWT/SSO），并做 RBAC（谁能提交哪些 task）。
- 内部鉴权：BFF 调 Spearlet/SMS 建议使用 mTLS 或至少内网 allowlist。
- 审计：记录 `request_id/decision_id/execution_id/task_id/user`，不记录 secrets。

### 观测与排障（补充到工程化）

- 日志：按 `request_id` 结构化输出 placement 候选、每次尝试的 node_uuid、错误分类、最终决策。
- 指标：除了前文指标外，建议增加 `bff_inflight{node_uuid}`、`bff_circuit_open_total{node_uuid}`。

### 测试与验收（工程化落地清单）

- 单测：错误分类与 spillback 决策表（输入 gRPC Status/错误码，输出是否重试）。
- 集成测：
  - mock SMS placement 返回两节点，第一节点返回 OVERLOADED，验证 BFF spillback 到第二节点。
  - mock 节点不可达，验证 BFF 不会长时间阻塞。
- 压测：验证 BFF 并发上限、每节点 inflight 限制与熔断逻辑有效。

### 错误分类与 spillback 决策表（推荐默认）

| 失败类型 | 典型信号 | 是否 spillback | 备注 |
|---|---|---:|---|
| 节点不可达 | TCP connect 失败 / gRPC UNAVAILABLE | 是 | 优先换节点 |
| 超时 | gRPC DEADLINE_EXCEEDED | 是（有限次数） | 可能已开始执行；建议仅对“连接前/握手阶段超时”更激进 |
| 过载拒绝 | 业务 OVERLOADED / RESOURCE_EXHAUSTED | 是 | 目标节点主动拒绝，换节点收益高 |
| 任务不存在 | TaskNotFound | 否 | Spearlet 已支持按需从 SMS 拉取任务并 materialize 后再执行 |
| 参数错误 | INVALID_ARGUMENT | 否 | 重试无意义 |
| 函数内部错误 | FAILED_PRECONDITION / INTERNAL（业务） | 否 | 通常不应换节点 |

### 端到端时延预算与超时传播

best practice：BFF 负责“总预算切分”，并把 deadline 向下游传播。

- `total_timeout_ms`：来自 `X-Total-Timeout-Ms` 或默认（例如 sync 30s、async 5s）
- `per_node_timeout_ms`：来自请求 spillback 或默认
- `placement_timeout_ms`：建议很短（例如 200~500ms）

建议约束：

- `placement_timeout_ms + max_attempts * per_node_timeout_ms <= total_timeout_ms`
- 若不足，BFF 需收缩 per_node_timeout 或减少尝试次数。

### 可观测性（必须字段）

- `request_id`：贯穿 Admin Page → BFF → SMS → Spearlet
- `decision_id`：placement 生成，关联候选列表
- `execution_id`：Spearlet 生成或由 BFF 指定（用于幂等与重试）

指标建议：

- `bff_submit_total{status}`、`bff_spillback_attempts_histogram`
- `bff_placement_latency_ms`、`bff_invoke_latency_ms{node_uuid}`
- `bff_error_total{class}`

 

## API 设计（控制面：SMS）

本方案优先提供“选址 API”，不代理转发执行请求。

### gRPC：PlacementService（建议新增）

建议在 SMS proto 中新增 PlacementService（可与现有 NodeService/TaskService 并列）。

#### PlaceInvocation

请求（示意字段）：

```text
PlaceInvocationRequest {
  string request_id;               // 幂等键，便于重试去重（未来扩展）
  string task_id;                  // 可选：现有任务调用
  string artifact_id;              // 可选：用于缓存/冷热策略
  string runtime_type;             // 如 wasm/process/k8s
  map<string,string> node_selector;// 标签/能力约束（如 gpu=true, arch=x86_64）
  ResourceRequirements req;        // 资源需求（初期可为空）
  SpillbackPolicy spillback;       // 重试预算建议
}

ResourceRequirements {
  double cpu_cores;                // 可选
  int64 memory_bytes;              // 可选
}

SpillbackPolicy {
  uint32 max_attempts;             // 例如 2~3
  uint32 per_node_timeout_ms;      // 例如 3000~10000
  bool   allow_requery;            // 候选耗尽后是否允许再请求一次 placement
}
```

响应（示意字段）：

```text
PlaceInvocationResponse {
  repeated CandidateNode candidates; // 按优先级排序
  string decision_id;                // 可观测/追踪
  string policy;                     // 当前策略名
}

CandidateNode {
  string node_uuid;
  string grpc_addr;                  // e.g. 10.0.0.1:50052
  string http_addr;                  // e.g. 10.0.0.1:8081 (可选)
  double score;
  string reason;                     // 人可读解释
  map<string,string> debug;          // 机器可读解释（可选）
  string lease_token;                // 预留：未来做资源租约/预占
  int64 lease_expire_at_unix_ms;     // 预留
}
```

#### ReportInvocationOutcome（建议新增，服务于 spillback 与降权）

目的：让 SMS 获知“节点拒绝/失败/超时”等结果，用于短期降权与 outlier 避免。

```text
ReportInvocationOutcomeRequest {
  string decision_id;
  string node_uuid;
  string execution_id;
  Outcome outcome;              // SUCCESS / OVERLOADED / UNAVAILABLE / TIMEOUT / ERROR
  int64 latency_ms;
  string error_code;            // 可选
}
```

初期可以不实现该接口，但建议在设计上保留，为未来提升稳定性与减少羊群效应提供入口。

### HTTP：/placement（可选，用于调试与非 gRPC 客户端）

如果需要 HTTP 网关支持，可新增：

- `POST /api/v1/placement/invocations/place`
- `POST /api/v1/placement/invocations/report-outcome`

### HTTP→gRPC 网关映射（工程化规范）

本节定义 SMS HTTP gateway（Axum）如何把新增的 placement 能力暴露为 HTTP，并转发到 gRPC PlacementService。

#### 路由（HTTP）

- `POST /api/v1/placement/invocations/place` → gRPC `PlacementService.PlaceInvocation`
- `POST /api/v1/placement/invocations/report-outcome` → gRPC `PlacementService.ReportInvocationOutcome`

建议同时支持调试用途的查询接口（可选）：

- `GET /placement/nodes`：返回当前候选节点与评分（不改变系统状态，仅用于排障）

#### 请求/响应（JSON）

`POST /api/v1/placement/invocations/place`

请求：

```json
{
  "request_id": "req-xxx",
  "task_id": "task-xxx",
  "artifact_id": "artifact-xxx",
  "runtime_type": "wasm|process|kubernetes",
  "node_selector": {"gpu": "true"},
  "req": {"cpu_cores": 1.0, "memory_bytes": 1073741824},
  "spillback": {"max_attempts": 2, "per_node_timeout_ms": 5000, "allow_requery": false}
}
```

响应：

```json
{
  "decision_id": "dec-xxx",
  "policy": "weighted_score_v1",
  "candidates": [
    {
      "node_uuid": "node-1",
      "grpc_addr": "10.0.0.1:50052",
      "http_addr": "10.0.0.1:8081",
      "score": 0.82,
      "reason": "cpu_idle=0.7 mem_idle=0.9 load=0.8",
      "debug": {"cpu_usage_percent": "30", "mem_usage_percent": "10"}
    }
  ]
}
```

`POST /api/v1/placement/invocations/report-outcome`

请求：

```json
{
  "decision_id": "dec-xxx",
  "node_uuid": "node-1",
  "execution_id": "exec-xxx",
  "outcome": "SUCCESS|OVERLOADED|UNAVAILABLE|TIMEOUT|ERROR",
  "latency_ms": 1234,
  "error_code": ""
}
```

响应：

```json
{ "success": true }
```

#### 网关实现要求（工程化）

- gRPC client 注入：HTTP gateway 的 `GatewayState` 需要新增 `placement_client`（类似现有 `node_client/task_client`）。
  - 参考：[GatewayState](../src/sms/gateway.rs)
  - 参考：[http_gateway.rs](../src/sms/http_gateway.rs)
- routes/handlers：新增 `handlers/placement.rs` 与对应 routes wiring。
  - 参考现有模式：[handlers/node.rs](../src/sms/handlers/node.rs)
- 错误映射：
  - gRPC `Status::invalid_argument` → HTTP 400
  - gRPC `Status::unavailable` → HTTP 503
  - gRPC `Status::deadline_exceeded` → HTTP 504
  - 其他 → HTTP 500
- 超时：HTTP handler 必须设置 placement 的短超时（例如 200~500ms），防止网关被慢请求拖垮。
- 可观测性：响应必须回传 `decision_id`；日志必须包含 `request_id`。

实现方式参考现有 SMS HTTP gateway（handlers + routes）：

 - 参考：[gateway.rs](../src/sms/gateway.rs)
 - 参考：[handlers/node.rs](../src/sms/handlers/node.rs)

## 节点侧语义（数据面：Spearlet）

### “快速拒绝”与 spillback 触发

为让 spillback 成本可控，Spearlet 在“明显接不住”时应尽快返回可识别的错误码：

- UNAVAILABLE：节点不可达/服务不可用。
- OVERLOADED：节点过载（例如并发信号量耗尽、队列过长）。
- NO_CAPACITY：实例池/运行时无法满足（例如 runtime 不支持、实例创建失败）。

候选触发点（现有代码可落点）：

- TaskExecutionManager::submit_execution：
  - 目前会进入 request_sender 队列并等待结果。
  - 设计建议：未来可以在入队前做 admission check（例如 semaphore 当前可用量、队列长度阈值），若不满足直接返回 OVERLOADED。
  - 参考：[manager.rs](../src/spearlet/execution/manager.rs#L232-L340)
- InstanceScheduler：
  - 若无可用实例且创建新实例失败，可返回 NO_CAPACITY。
  - 参考：[scheduler.rs](../src/spearlet/execution/scheduler.rs)

## Spillback 机制设计

### 客户端驱动 spillback（第一阶段推荐）

核心原则：由 Client/SDK 对候选节点进行有界重试。

伪代码（说明语义，不绑定具体语言）：

```text
decision = SMS.PlaceInvocation(req)
for i, node in decision.candidates:
  result = try InvokeFunction(node, timeout=per_node_timeout)
  if result.success:
    SMS.ReportInvocationOutcome(SUCCESS)
    return result
  if result.error in {OVERLOADED, UNAVAILABLE, TIMEOUT}:
    SMS.ReportInvocationOutcome(result.error)
    continue
  else:
    SMS.ReportInvocationOutcome(ERROR)
    return result

if allow_requery:
  decision2 = SMS.PlaceInvocation(req with attempt_hint)
  repeat...
return last_error
```

建议默认：

- max_attempts=2（最多换 1 次节点）
- per_node_timeout=3~10s（取决于同步/异步模式）
- allow_requery=false（第一阶段尽量简单）

### 节点驱动 spillback（第二阶段可选增强）

当 Spearlet 能判断“更适合去别处”时，可返回重定向建议：

- 在错误 payload 中携带 `suggested_nodes[]` 或 `suggest_requery=true`。
- Client 收到后优先按建议顺序重试。

该模式能更接近 Ray 的 spillback 语义，但需要 Spearlet 感知更多集群信息（或与 SMS 交互）。

## SMS 调度器实现设计（函数/方法级别）

本节定义建议新增/扩展的代码结构与方法签名（Rust 风格伪接口），用于后续落地。

### 建议新增文件与模块

- `src/sms/services/placement_service.rs`
- `src/sms/handlers/placement.rs`（如需 HTTP）
- `src/sms/routes.rs` 增加 placement 路由（如需 HTTP）
- `src/sms/service.rs` 增加 PlacementService 的 gRPC 实现（如走 gRPC）

### 关键类型

```rust
pub struct PlacementService {
    node_service: Arc<RwLock<NodeService>>,
    resource_service: Arc<ResourceService>,
    config: PlacementConfig,
    stats: Arc<parking_lot::RwLock<PlacementStats>>,
}

pub struct PlacementConfig {
    pub heartbeat_timeout_s: u64,
    pub max_candidates: usize,
    pub cpu_high_watermark: f64,
    pub mem_high_watermark: f64,
    pub load_high_watermark_1m: f64,
    pub scoring_weights: ScoringWeights,
}

pub struct ScoringWeights {
    pub cpu_idle: f64,
    pub mem_idle: f64,
    pub load: f64,
    pub recent_failure_penalty: f64,
}
```

### 核心入口：place_invocation

```rust
impl PlacementService {
    pub async fn place_invocation(
        &self,
        req: PlaceInvocationRequest,
    ) -> Result<PlaceInvocationResponse, Status>;
}
```

内部建议拆分方法（便于扩展与测试）：

```rust
impl PlacementService {
    async fn list_healthy_nodes(&self) -> Vec<Node>;

    async fn join_resources(
        &self,
        nodes: Vec<Node>,
    ) -> Vec<(Node, Option<NodeResourceInfo>)>;

    fn filter_candidates(
        &self,
        req: &PlaceInvocationRequest,
        nodes: Vec<(Node, Option<NodeResourceInfo>)>,
    ) -> Vec<(Node, Option<NodeResourceInfo>)>;

    fn score_candidate(
        &self,
        req: &PlaceInvocationRequest,
        node: &Node,
        res: Option<&NodeResourceInfo>,
    ) -> (f64, CandidateDebug);

    fn choose_top_k(
        &self,
        scored: Vec<ScoredNode>,
        k: usize,
    ) -> Vec<CandidateNode>;
}
```

### 过滤规则（predicates）建议

- `is_online(node)`：node.status == online。
- `heartbeat_fresh(node)`：node.last_heartbeat 在 timeout 内。
- `match_selector(node.metadata, req.node_selector)`：标签/能力硬约束。
- `resource_not_high_load(res)`：避免高负载节点（可复用 ResourceService 的判定思路）。

### 打分规则（scoring）建议

建议采用加权线性模型，且保证 O(N) 简单实现即可：

- `cpu_idle = 1 - cpu_usage_percent/100`
- `mem_idle = 1 - memory_usage_percent/100`
- `load = 1 - clamp(load_average_1m / load_high_watermark_1m)`
- `penalty = recent_failure_rate * recent_failure_penalty`

最终：`score = w1*cpu_idle + w2*mem_idle + w3*load - penalty`。

并在 CandidateNode.reason/debug 中返回：

- 过滤命中原因（如 selector/gpu）。
- 关键指标（cpu%、mem%、load）。
- score 分解项（便于调参）。

## 为 SMS 水平扩展预留的空间（设计约束）

虽然第一阶段不实现水平扩展，但建议在接口与内部结构上保持以下特性，避免未来大改：

- PlacementService 尽量“无状态”或“软状态”（缓存可丢）。
- PlaceInvocationRequest 带 `request_id`（幂等键）。
- 响应带 `decision_id`（便于跨实例追踪与 report）。
- 预留 `lease_token`（未来做资源预占/减少羊群效应）。
- 将“策略”与“存储/缓存”解耦：
  - `trait PlacementPolicy { fn filter(...); fn score(...); }`
  - `trait PlacementStateStore { async fn get_node_snapshot(...); }`

## 迁移与落地计划（建议）

### Phase 1：最小可用（MVP）

- SMS 提供 PlaceInvocation（返回 2 个候选节点）。
- Client/CLI/SDK 或 Admin BFF 在调用前先请求 placement。
- 内网可直连：编排层直连 Spearlet。
- Spearlet 不可外网访问：Admin Page 通过 BFF 触发执行。
- Spillback：仅客户端驱动，2 次尝试。

### Phase 2：可靠性增强

- 实现 ReportInvocationOutcome。
- SMS 对失败率高的节点短期降权/熔断。

### Phase 3：资源租约（可选）

- 在 placement 时发放短 TTL lease，减少并发下的羊群效应。

## 需要明确的工程决策（后续落地前）

- Client/SDK 入口在哪里（CLI、HTTP gateway、还是独立库）。
- placement 请求的字段对齐：runtime_type/selector 的最小集合。
- Spearlet “快速拒绝”错误码的标准化（gRPC Status + details 或自定义错误结构）。
