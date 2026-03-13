# Router 基于 gRPC 的候选过滤（设计，函数/接口级）

本文档定义 **方案 C**：Router 在选择 backend instance 之前，调用一个**外部 gRPC 决策服务**，对请求进行检测，并对候选集执行过滤/打分（适用于 Chat Completions、ASR 等）。

设计目标是匹配当前分层路径：

`Normalize -> CanonicalRequestEnvelope -> Router -> BackendAdapter`

参考：
- `routing-zh.md`：能力过滤与选择策略（LB/Fallback/Hedge/Mirror）
- `implementation-plan-zh.md`：文件/函数级集成点规划

## 1. 目标 / 非目标

### 目标

1. **发出请求前检测**：在 forward 前识别并标注请求属性（策略、安全、合规、成本）。
2. **候选过滤**：从候选 backend 集合中移除不合规/不适合的实例。
3. **候选打分**：对剩余候选进行偏好调整（weight / priority / score）。
4. **强可靠性**：外部服务不可用时 Router 仍可用（默认 fail-open）。
5. **确定性契约**：稳定的 proto API、可控的时间预算、受限的 payload 规模。

### 非目标

- 不负责真正的上游调用（仍由 `BackendAdapter` 完成）。
- 不做长耗时分析或流式检测（必须满足路由延迟预算）。
- 不向外部决策服务暴露敏感信息（secret）。

## 2. 在当前 Router 流程中的位置

当前路由逻辑位于：
- `src/spearlet/execution/ai/router/mod.rs`（`Router::route`）
- `src/spearlet/execution/ai/router/registry.rs`（`BackendRegistry::candidates`）

新增的 gRPC 步骤插入在：硬过滤与 routing hints 之后、选择策略之前：

1. `candidates = registry.candidates(req)`（operation + required_features + transports）
2. 应用 routing hints（backend/allowlist/denylist/model binding）
3. **调用外部过滤/打分服务**（本文）
4. `selected = policy.select(req, candidates)`

## 3. gRPC 服务契约（proto，基于双向 stream）

### 3.1 Proto 文件与 package

建议新增 proto 文件：

- `proto/spearlet/router_filter.proto`
- `package spearlet;`

让契约贴近 Spearlet Router 的使用场景（即使服务部署在外部）。

### 3.2 Service 定义（Filter 进程主动 dial Spearlet TCP）

```proto
syntax = "proto3";

package spearlet;

// RouterFilterStreamService：Filter 进程作为 gRPC client，主动连接到 Spearlet 的 TCP gRPC 端口；
// Spearlet 作为 gRPC server，通过同一条双向 stream 下发过滤请求并接收决策响应。
service RouterFilterStreamService {
  // Open 建立一条长连接双向 stream。
  // - Filter 侧：发送 Register/Heartbeat/FilterResponse
  // - Spearlet 侧：发送 FilterRequest/Ping/Reject
  rpc Open(stream StreamClientMessage) returns (stream StreamServerMessage);

  // FetchRequestById 允许 agent 基于 request_id + session_token 受控拉取请求 payload。
  rpc FetchRequestById(RequestFetchRequest) returns (RequestFetchResponse);
}

enum Operation {
  OPERATION_UNSPECIFIED = 0;
  OPERATION_CHAT_COMPLETIONS = 1;
  OPERATION_EMBEDDINGS = 2;
  OPERATION_IMAGE_GENERATION = 3;
  OPERATION_SPEECH_TO_TEXT = 4;
  OPERATION_TEXT_TO_SPEECH = 5;
  OPERATION_REALTIME_VOICE = 6;
}

message StreamClientMessage {
  oneof msg {
    RegisterRequest register = 1;
    Heartbeat heartbeat = 2;
    FilterResponse filter_response = 3;
  }
}

message StreamServerMessage {
  oneof msg {
    RegisterResponse register_ok = 1;
    Ping ping = 2;
    Reject reject = 3;
    FilterRequest filter_request = 4;
  }
}

message RegisterRequest {
  // Filter 进程标识（用于观测与路由选择）。
  string agent_id = 1;

  // Filter 能力宣告（可选，用于后续做按 op 分发/版本灰度）。
  repeated Operation supported_operations = 2;

  // Filter 可并发处理的最大 in-flight 请求数（Spearlet 用于背压）。
  uint32 max_inflight = 3;

  // Filter 期望的最大候选数（Spearlet 会做 min 截断）。
  uint32 max_candidates = 4;

  // Filter 支持的协议版本（便于演进）。
  uint32 protocol_version = 5;
}

message RegisterResponse {
  uint32 protocol_version = 1;
  bool accepted = 2;
  string message = 3;
  string session_token = 4;
  int64 token_expire_at_ms = 5;
}

message Heartbeat {
  // 毫秒时间戳（可选）。
  int64 now_ms = 1;
}

message Ping {
  int64 now_ms = 1;
}

message Reject {
  string code = 1;
  string message = 2;
}

message FilterRequest {
  // 用于在同一条 stream 上并发复用的关联 ID。
  // Spearlet 生成，Filter 必须原样回传。
  string correlation_id = 1;

  string request_id = 2;                 // CanonicalRequestEnvelope.request_id
  Operation operation = 3;               // CanonicalRequestEnvelope.operation

  // 决策预算（Spearlet 也会在本地 enforce 超时；Filter 侧用于自我限时）。
  uint32 decision_timeout_ms = 4;

  // 请求元信息（安全、非 secret）。
  map<string, string> meta = 5;          // CanonicalRequestEnvelope.meta（字符串化）

  // 来自 guest 与 host 约束的 routing hints。
  RoutingHints routing = 6;

  // Normalize 层产出的 required features/transports。
  Requirements requirements = 7;

  // 面向 operation 的、受大小限制的 request signals。
  RequestSignals signals = 8;

  repeated Candidate candidates = 9;     // 硬过滤后的候选集
}

message RoutingHints {
  string backend = 1;                    // 可选：指定 backend 名称
  repeated string allowlist = 2;
  repeated string denylist = 3;
  string requested_model = 4;            // 从 payload.model 提取（若存在）
}

message Requirements {
  repeated string required_features = 1;
  repeated string required_transports = 2;
}

message RequestSignals {
  // Chat：model、消息数、文本体积估计、tools 标志、response_format 标志。
  string model = 1;
  uint32 message_count = 2;
  uint32 approx_text_bytes = 3;
  bool uses_tools = 4;
  bool uses_json_schema = 5;

  // ASR：codec、采样率、声道数、时长估计、语言提示。
  string audio_codec = 10;
  uint32 audio_sample_rate_hz = 11;
  uint32 audio_channels = 12;
  uint32 audio_duration_ms = 13;
  string audio_language_hint = 14;
}

message RequestFetchRequest {
  string request_id = 1;
  string session_token = 2;
  uint32 max_bytes = 3;
}

message RequestFetchResponse {
  string request_id = 1;
  string content_type = 2;
  bytes payload = 3;
}

message Candidate {
  string name = 1;                       // BackendInstance.name
  string kind = 2;                       // backend kind（openai_chat_completion / ...）
  string base_url = 3;                   // 可选；出于隐私可默认不发送
  string model = 4;                      // BackendInstance.model（若绑定）
  uint32 weight = 5;                     // BackendInstance.weight
  int32 priority = 6;                    // BackendInstance.priority
  repeated string ops = 7;               // capabilities.ops（字符串形式）
  repeated string features = 8;          // capabilities.features
  repeated string transports = 9;        // capabilities.transports

  // 可选：运行时提示（如可用）。
  CandidateRuntimeHints runtime = 20;
}

message CandidateRuntimeHints {
  // 全字段可选；Router 可省略。
  double ewma_latency_ms = 1;
  double recent_error_rate = 2;          // 0.0 ~ 1.0
  uint32 inflight = 3;
  bool healthy = 4;
}

message FilterResponse {
  // 对应 FilterRequest.correlation_id
  string correlation_id = 1;

  string decision_id = 2;                // 用于可观测
  repeated CandidateDecision decisions = 3;
  FinalAction final_action = 4;
  map<string, string> debug = 5;         // 可选 debug kv（受大小限制）
}

message CandidateDecision {
  string name = 1;                       // 对应 Candidate.name（后端实例名）

  // KEEP 表示保留候选；DROP 表示从候选集中移除。
  DecisionAction action = 2;

  // 可选覆盖；未设置则 Router 保留原值。
  optional uint32 weight_override = 3;
  optional int32 priority_override = 4;

  // 可选软分；Router 可仅用于 debug 或用于“score-aware”策略。
  optional double score = 5;

  // 原因码（可用于日志与排障）。
  repeated string reason_codes = 6;
}

enum DecisionAction {
  DECISION_ACTION_UNSPECIFIED = 0;
  DECISION_ACTION_KEEP = 1;
  DECISION_ACTION_DROP = 2;
}

message FinalAction {
  // 若为 true，Router 直接拒绝请求（fail-closed）。
  // Router 必须映射为 CanonicalError；除非显式指定，否则建议 retryable=false。
  bool reject_request = 1;
  string reject_code = 2;                // 例如 "policy_denied"
  string reject_message = 3;

  // 若设置，Router 强制选择某 backend name（仍受 allowlist/denylist 等硬约束限制）。
  string force_backend = 10;
}
```

### 3.3 受控拉取请求内容（request_id + session_token）

默认情况下，Spearlet 只向 Filter 发送受限的 `RequestSignals`，不透传原始请求内容。

当策略确实需要基于内容做判断时，推荐采用“受控拉取”模式：

1. Filter 通过 `Open` stream 完成注册，Spearlet 在 `RegisterResponse` 中返回 `session_token` 及其过期时间。
2. Filter 在处理 `FilterRequest` 时，如需正文，使用 `FetchRequestById(request_id, session_token, max_bytes)` 发起一次 unary 拉取。
3. Spearlet 仅在满足如下条件时返回 payload：
   - `content_fetch_enabled = true`
   - `session_token` 有效且关联的 agent 仍在连接中
   - `request_id` 命中短 TTL 的内存缓存
   - 返回大小不超过 `content_fetch_max_bytes`（同时受请求侧 `max_bytes` 限制）

该模式的目标是：把“访问正文”从默认广播变成显式、可控、可审计的行为，并通过短 TTL/大小上限降低风险面。

## 4. Router 侧实现规划（函数 / 变量级）

### 4.1 新模块与主要入口（Rust）

建议新增 Router 侧 gRPC stream 接入模块：

- `src/spearlet/execution/ai/router/grpc_filter_stream.rs`

核心类型与函数（规划）：

```rust
pub struct RouterFilterStreamConfig {
    pub enabled: bool,
    pub addr: String,
    pub decision_timeout_ms: u64,
    pub fail_open: bool,
    pub max_candidates_sent: usize,
    pub max_debug_kv: usize,
    pub max_inflight_total: usize,
    pub per_agent_max_inflight: usize,
}

pub struct RouterFilterStreamHub {
    config: RouterFilterStreamConfig,
    agents: tokio::sync::RwLock<std::collections::HashMap<String, AgentHandle>>,
    rr: std::sync::atomic::AtomicU64,
    inflight: tokio::sync::Mutex<std::collections::HashMap<String, tokio::sync::oneshot::Sender<FilterResponse>>>,
}

pub struct AgentHandle {
    pub agent_id: String,
    pub tx: tokio::sync::mpsc::Sender<FilterRequest>,
    pub inflight: std::sync::Arc<tokio::sync::Semaphore>,
    pub supported_operations: std::collections::HashSet<Operation>,
    pub last_heartbeat_ms: std::sync::atomic::AtomicI64,
}

pub struct FilterTrace {
    pub decision_id: Option<String>,
    pub dropped: Vec<String>,
    pub weight_overrides: Vec<(String, u32)>,
    pub priority_overrides: Vec<(String, i32)>,
    pub reason_codes_by_candidate: std::collections::HashMap<String, Vec<String>>,
    pub final_action: Option<FinalActionTrace>,
}

pub struct FinalActionTrace {
    pub reject_request: bool,
    pub reject_code: Option<String>,
    pub force_backend: Option<String>,
}

impl RouterFilterStreamHub {
    pub async fn try_filter_candidates(
        &self,
        req: &CanonicalRequestEnvelope,
        candidates: &[&BackendInstance],
        decision_timeout_ms: u64,
    ) -> Result<(FilterResponse, FilterTrace), CanonicalError>;
}
```

### 4.1.1 传输方式：TCP（Spearlet 对外暴露 gRPC 端口）

本设计采用 TCP（host:port）作为连接方式：

- Spearlet 作为 gRPC server 监听 TCP 端口（通常复用 `spearlet.grpc.addr`）。
- Filter 进程作为 gRPC client 主动 dial 该地址并维持长连接 stream。

地址格式示例：

- `127.0.0.1:50052`（同机 loopback）
- `spearlet-node-a.internal:50052`（跨宿主机）

### 4.2 Router::route 的集成细节

对 `Router` 的最小侵入改动（不强引入新的“大抽象”）：

```rust
pub struct Router {
    registry: BackendRegistry,
    policy: SelectionPolicy,
    grpc_filter_stream: Option<std::sync::Arc<RouterFilterStreamHub>>,
}
```

建议在 `ai/router/mod.rs` 中新增辅助函数：

```rust
fn apply_grpc_filter(
    req: &CanonicalRequestEnvelope,
    candidates: &mut Vec<&BackendInstance>,
    hub: &RouterFilterStreamHub,
) -> Result<FilterTrace, CanonicalError>;
```

在 `Router::route` 内部关键变量（建议命名）：

- `let mut candidates: Vec<&BackendInstance> = self.registry.candidates(req);`
- `let hard_filtered_count: usize = candidates.len();`
- `let decision_budget_ms: u64 = clamp(req.timeout_ms, cfg.decision_timeout_ms);`
- `let filter_res: Result<FilterTrace, CanonicalError> = apply_grpc_filter(...);`
- `let candidate_count_after_filter: usize = candidates.len();`
- `let selected: &BackendInstance = self.policy.select(req, candidates)?;`

错误与降级策略：

- `hub.config.fail_open == true`（推荐默认）：
  - 无可用 agent 连接 / stream 断开 / 等待超时 => **不改变** `candidates`
  - Router 记录结构化日志 `filter_failed=true` 并继续 `policy.select`
- `hub.config.fail_open == false`：
  - 无可用 agent / 超时 / 协议错误 => `CanonicalError { code: "router_filter_unavailable", retryable: true/false }`

FinalAction 处理：

- `reject_request=true` => Router 返回 `CanonicalError { code = reject_code 或 "router_filter_rejected", message = reject_message, retryable=false }`
- `force_backend` => Router 将 candidates 收缩到该 backend name，然后继续选择策略

### 4.3 Router 必须做的响应校验规则

避免外部服务“越权扩张能力”：

1. `CandidateDecision.name` 不在输入候选集合内 => 忽略该决策。
2. `force_backend` 必须属于硬约束后的候选集合（backend/allowlist/denylist/model binding 等仍生效）。
3. `weight_override` / `priority_override` 需限幅：
   - `weight_override` 在 `[0, 10_000]`
   - `priority_override` 在 `[-1000, 1000]`
4. 若全部候选被 DROP：
   - 若 `final_action.reject_request=true` 则按拒绝返回
   - 否则返回 `no_candidate_backend`（与现有行为保持一致）

## 5. 配置（TOML）

建议在 `spearlet.llm` 下新增：

```toml
[spearlet.llm.router_grpc_filter_stream]
enabled = true
addr = "127.0.0.1:50052"
decision_timeout_ms = 5
fail_open = true
max_candidates_sent = 64
max_debug_kv = 32
max_inflight_total = 4096
per_agent_max_inflight = 512
```

Rust 映射（规划）：

- `SpearletConfig.llm.router_grpc_filter_stream: Option<RouterFilterStreamConfig>`

## 6. 工程化建议（best practices）

### 6.1 时间预算

- 非流式：推荐 `decision_timeout_ms <= 5ms`（同机房）或 `<= 10ms`（跨宿主机）
- 流式（首 token / 首音频帧）：预算更紧（1–3ms），并优先 fail-open

### 6.2 幂等与缓存

- `FilterCandidatesRequest.request_id` 用作重试幂等 key
- Router 可做短期缓存：
  - key：`(request_id, operation, requested_model, candidate_names_hash)`
  - value：`FilterCandidatesResponse`
  - ttl：`min(500ms, decision_timeout_ms * 100)`

### 6.3 可观测性字段

Router 结构化日志建议包含：

- `request_id`, `operation`, `candidate_count_before`, `candidate_count_after`
- `filter_decision_id`, `filter_elapsed_ms`, `filter_failed`
- `dropped_candidates[]`（截断），`selected_backend`

## 7. 安全边界

- Router 不得向外部服务发送 secret（API key、token 等）
- `base_url` 默认建议不发送，避免暴露内部拓扑
- 外部服务输出必须受限并校验（见 4.3）

## 8. 吞吐与共享（多 WASM instance 场景）

### 8.1 是否需要共享一个“client”

在 streaming 设计里：

- Spearlet 是 gRPC server（监听 TCP），Filter 进程是 gRPC client（主动连接）。
- Router 侧不再为每个请求创建 gRPC client；它只依赖一个进程级的 `RouterFilterStreamHub` 来复用已有的 stream 连接。

结论：

- **需要共享**：同一进程内所有 WASM instance 的 host API / `DefaultHostApi` / `AiEngine` 应共享同一个 `RouterFilterStreamHub`。
- 不建议每个 instance 单独维护一条 stream：会导致连接数爆炸、context switch 增多、并产生不必要的握手与资源开销。

### 8.2 如何让吞吐更大

建议按以下层次做扩展（从易到难）：

1. **单连接并发复用（必选）**
   - 在同一条双向 stream 上允许并发多个 `FilterRequest`，用 `correlation_id` 做关联。
   - `RouterFilterStreamHub.inflight` 维护 `correlation_id -> oneshot_sender` 的映射；响应回来后完成对应 oneshot。

2. **多连接并行（推荐）**
   - 允许多个 Filter agent 同时连接到 Spearlet（同一个 addr）。
   - `RouterFilterStreamHub.agents` 保存多个 `AgentHandle`，对请求做 `least_inflight` 或 round-robin 分发。

3. **背压与限流（必选）**
   - `max_inflight_total`：全局上限，避免 Router 被外部 filter 堵死。
   - `per_agent_max_inflight`：每个 agent 的并发上限（`Semaphore`），避免单个 agent 过载导致尾延迟飙升。

4. **请求负载瘦身（强烈建议）**
   - 限制 `max_candidates_sent`；把候选集裁到真正需要打分的规模（例如 32~128）。
   - `signals` 只发送摘要，不发送大 payload（ASR 音频/超长 prompt）。
   - 限制 `debug` kv 的数量与长度，避免响应膨胀。

5. **超时与“晚到响应”处理（必选）**
   - Router 侧按 `decision_timeout_ms` 超时返回；超时后要把 inflight 映射删除。
   - Filter 侧晚到的 `FilterResponse` 直接丢弃并计数（便于观测）。
