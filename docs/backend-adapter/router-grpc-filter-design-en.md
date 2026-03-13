# gRPC-based Candidate Filtering for Router (Design, Function/Interface Level)

This document specifies **Option C**: an **external gRPC decision service** that the Router calls **right before selecting** a backend instance, to perform request inspection and candidate filtering/scoring for operations like Chat Completions and ASR.

It is designed to fit the current layered path:

`Normalize -> CanonicalRequestEnvelope -> Router -> BackendAdapter`

See:
- `routing-en.md` for capability filtering and selection policies
- `implementation-plan-en.md` for file/function-level integration points

## 1. Goals / Non-goals

### Goals

1. **Pre-dispatch inspection**: detect/annotate request properties before forwarding (policy, safety, compliance, cost).
2. **Candidate filtering**: drop ineligible backends from a candidate set.
3. **Candidate scoring**: adjust preference (weights / priorities / scores) among remaining candidates.
4. **Strict reliability**: Router must remain available if the external service is down (fail-open by default).
5. **Deterministic contract**: stable proto API, predictable time budget, bounded payload size.

### Non-goals

- Performing the actual upstream invocation (still done by `BackendAdapter`).
- Long-running analysis or streaming inspection (must fit routing latency budget).
- Exposing secrets to the external decision service.

## 2. Placement in the Current Router Flow

Current routing is implemented in:
- `src/spearlet/execution/ai/router/mod.rs` (`Router::route`)
- `src/spearlet/execution/ai/router/registry.rs` (`BackendRegistry::candidates`)

The new gRPC step is inserted **after** hard capability filtering and request routing hints, **before** selection policy:

1. `candidates = registry.candidates(req)` (operation + required_features + transports)
2. Apply routing hints (backend/allowlist/denylist/model binding)
3. **Call external filter/scorer** (this design)
4. `selected = policy.select(req, candidates)`

## 3. gRPC Service Contract (proto, bidirectional stream)

### 3.1 Proto file and package

Proposed new proto file:

- `proto/spearlet/router_filter.proto`
- `package spearlet;`

This keeps the contract close to the Spearlet router use case (even if the server is deployed externally).

### 3.2 Service definition (Filter agent dials Spearlet TCP)

```proto
syntax = "proto3";

package spearlet;

// RouterFilterStreamService: the Filter process acts as a gRPC client and connects to Spearlet over TCP.
// Spearlet acts as a gRPC server and pushes filter requests over the same bidirectional stream.
service RouterFilterStreamService {
  // Open establishes a long-lived bidirectional stream.
  // - Filter side sends: Register/Heartbeat/FilterResponse
  // - Spearlet side sends: FilterRequest/Ping/Reject
  rpc Open(stream StreamClientMessage) returns (stream StreamServerMessage);

  // FetchRequestById allows the agent to fetch request payload by request_id + session_token.
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
  string agent_id = 1;                   // identity for observability and routing
  repeated Operation supported_operations = 2;
  uint32 max_inflight = 3;               // backpressure: max concurrent in-flight requests
  uint32 max_candidates = 4;             // preferred candidate set size
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
  // Correlation id for multiplexing concurrent requests on the same stream.
  string correlation_id = 1;

  string request_id = 2;                 // CanonicalRequestEnvelope.request_id
  Operation operation = 3;               // CanonicalRequestEnvelope.operation

  // Time budget. Router also enforces it locally; filter uses it for self-limiting.
  uint32 decision_timeout_ms = 4;

  // Request metadata (safe, non-secret).
  map<string, string> meta = 5;          // CanonicalRequestEnvelope.meta (stringified)

  // Routing hints that came from the guest and/or host constraints.
  RoutingHints routing = 6;

  // Required features/transports extracted from Normalize.
  Requirements requirements = 7;

  // Operation-specific, size-bounded request signals.
  RequestSignals signals = 8;

  repeated Candidate candidates = 9;     // candidate set after hard filtering
}

message RoutingHints {
  string backend = 1;                    // optional requested backend name
  repeated string allowlist = 2;
  repeated string denylist = 3;
  string requested_model = 4;            // extracted from payload.model if present
}

message Requirements {
  repeated string required_features = 1;
  repeated string required_transports = 2;
}

message RequestSignals {
  // For chat: model, message count, approximate text bytes, tool usage flags, response format flags.
  string model = 1;
  uint32 message_count = 2;
  uint32 approx_text_bytes = 3;
  bool uses_tools = 4;
  bool uses_json_schema = 5;

  // For ASR: codec, sample rate, channel count, duration estimate, language hint.
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
  string kind = 2;                       // backend kind (openai_chat_completion / ...)
  string base_url = 3;                   // optional; can be omitted for privacy
  string model = 4;                      // BackendInstance.model (if bound)
  uint32 weight = 5;                     // BackendInstance.weight
  int32 priority = 6;                    // BackendInstance.priority
  repeated string ops = 7;               // capabilities.ops (string form)
  repeated string features = 8;          // capabilities.features
  repeated string transports = 9;        // capabilities.transports

  // Optional runtime hints (if available).
  CandidateRuntimeHints runtime = 20;
}

message CandidateRuntimeHints {
  // All fields are optional; Router may omit them.
  double ewma_latency_ms = 1;
  double recent_error_rate = 2;          // 0.0 ~ 1.0
  uint32 inflight = 3;
  bool healthy = 4;
}

message FilterResponse {
  string correlation_id = 1;             // matches FilterRequest.correlation_id
  string decision_id = 2;                // for observability
  repeated CandidateDecision decisions = 3;
  FinalAction final_action = 4;
  map<string, string> debug = 5;         // optional debug kvs (size-bounded)
}

message CandidateDecision {
  string name = 1;                       // matches Candidate.name (backend instance name)

  // KEEP means candidate remains usable; DROP means remove from candidate set.
  DecisionAction action = 2;

  // Optional overrides; if unset, Router keeps original values.
  optional uint32 weight_override = 3;
  optional int32 priority_override = 4;

  // Optional soft score; Router may use it only for debug or for a "score-aware" policy.
  optional double score = 5;

  // Human/debug reasons. Router also logs these as structured fields.
  repeated string reason_codes = 6;
}

enum DecisionAction {
  DECISION_ACTION_UNSPECIFIED = 0;
  DECISION_ACTION_KEEP = 1;
  DECISION_ACTION_DROP = 2;
}

message FinalAction {
  // If set, Router should stop and return an error to the caller (fail-closed).
  // Router must map this to CanonicalError and SHOULD mark retryable=false unless explicitly stated.
  bool reject_request = 1;
  string reject_code = 2;                // e.g. "policy_denied"
  string reject_message = 3;

  // If set, Router forces routing to a backend name (still constrained by allowlist/denylist).
  string force_backend = 10;
}
```

### 3.3 Controlled content fetch (request_id + session_token)

By default, Spearlet only sends size-bounded `RequestSignals` to the agent and does not transmit raw request content.

When the policy must inspect content, prefer a controlled fetch flow:

1. The agent registers via the `Open` stream; Spearlet returns `session_token` and its expiry in `RegisterResponse`.
2. While handling a `FilterRequest`, the agent can call `FetchRequestById(request_id, session_token, max_bytes)` (unary) to fetch the payload.
3. Spearlet only returns payload when:
   - `content_fetch_enabled = true`
   - `session_token` is valid and the agent is still connected
   - `request_id` hits a short-TTL in-memory cache
   - the payload size is within `content_fetch_max_bytes` (also capped by request `max_bytes`)

This turns “accessing raw content” into an explicit, bounded, auditable action instead of a default broadcast.

## 4. Router-side Implementation Plan (function / variable level)

### 4.1 New module and main entry points (Rust)

Proposed new module (Router-side stream integration):

- `src/spearlet/execution/ai/router/grpc_filter_stream.rs`

Key public types and functions:

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

### 4.1.1 Transport: TCP (Spearlet exposes a gRPC port)

This design uses TCP (host:port):

- Spearlet runs a gRPC server on a TCP port (typically reusing `spearlet.grpc.addr`).
- The filter process dials that address and maintains long-lived bidirectional stream(s).

Address examples:

- `127.0.0.1:50052` (same-host loopback)
- `spearlet-node-a.internal:50052` (cross-host)

### 4.2 Router::route integration details

Minimal changes to `Router` (no new “big” abstraction required):

```rust
pub struct Router {
    registry: BackendRegistry,
    policy: SelectionPolicy,
    grpc_filter_stream: Option<std::sync::Arc<RouterFilterStreamHub>>,
}
```

New helper function in `ai/router/mod.rs`:

```rust
fn apply_grpc_filter(
    req: &CanonicalRequestEnvelope,
    candidates: &mut Vec<&BackendInstance>,
    hub: &RouterFilterStreamHub,
) -> Result<FilterTrace, CanonicalError>;
```

Key local variables inside `Router::route`:

- `let mut candidates: Vec<&BackendInstance> = self.registry.candidates(req);`
- `let hard_filtered_count: usize = candidates.len();`
- `let decision_budget_ms: u64 = clamp(req.timeout_ms, cfg.decision_timeout_ms);`
- `let filter_res: Result<FilterTrace, CanonicalError> = apply_grpc_filter(...);`
- `let candidate_count_after_filter: usize = candidates.len();`
- `let selected: &BackendInstance = self.policy.select(req, candidates)?;`

Error and fallback policy:

- If `hub.config.fail_open == true`:
  - No available connected agent / stream reset / local wait timeout => **no change** to `candidates`.
  - Router logs `filter_failed=true` and continues to `policy.select`.
- If `hub.config.fail_open == false`:
  - No agent / timeout / protocol error => `CanonicalError { code: "router_filter_unavailable", retryable: true/false }`.

FinalAction handling:

- `reject_request=true` => Router returns `CanonicalError { code = reject_code or "router_filter_rejected", message = reject_message, retryable=false }`
- `force_backend` => Router rewrites candidates to just that backend name, then continues with policy selection.

### 4.3 Response validation rules (must be enforced by Router)

To prevent the external service from expanding power beyond allowed constraints:

1. Any `CandidateDecision.name` not in the input candidates is ignored.
2. `force_backend` must match the current candidate set after hard constraints (backend/allowlist/denylist/model binding).
3. `weight_override` and `priority_override` are clamped to safe bounds:
   - `weight_override` in `[0, 10_000]`
   - `priority_override` in `[-1000, 1000]`
4. If all candidates are dropped:
   - If `final_action.reject_request=true`, Router returns rejection.
   - Else Router returns `no_candidate_backend` (same as current behavior).

## 5. Configuration (TOML)

Proposed config fields under `spearlet.llm`:

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

Mapping to Rust:

- `SpearletConfig.llm.router_grpc_filter_stream: Option<RouterFilterStreamConfig>`

## 6. Operational Guidance (best practices)

### 6.1 Time budget

- For non-streaming requests, recommend `decision_timeout_ms <= 5ms` (local network) or `<= 10ms` (cross-host).
- For streaming (first token / first audio frame), enforce even tighter budgets (1–3ms) and prefer fail-open.

### 6.2 Idempotency and caching

- `FilterCandidatesRequest.request_id` is the idempotency key for retries.
- Router may keep a short-lived cache:
  - key: `(request_id, operation, requested_model, candidate_names_hash)`
  - value: `FilterCandidatesResponse`
  - ttl: `min(500ms, decision_timeout_ms * 100)`

### 6.3 Observability fields

Router logs should include (structured):

- `request_id`, `operation`, `candidate_count_before`, `candidate_count_after`
- `filter_decision_id`, `filter_elapsed_ms`, `filter_failed`
- `dropped_candidates[]` (capped), `selected_backend`

## 7. Security Boundary

- Router must not send secrets (API keys, bearer tokens) to the external service.
- `base_url` is optional and should be omitted by default if it reveals internal topology.
- External service output must be constrained and validated (Section 4.3).

## 8. Throughput and Sharing (multiple WASM instances)

### 8.1 Do WASM instances need a shared client?

With the streaming design:

- Spearlet is the gRPC server (listening on TCP), and the filter process is the gRPC client (dialing in).
- The Router should not create a per-request gRPC connection. It should reuse the already-established stream(s) via a process-level `RouterFilterStreamHub`.

Conclusion:

- **Share** a single `RouterFilterStreamHub` across all WASM instances / Host API entry points in the same Spearlet process.
- Avoid per-instance streams: it increases connection count, context switching, and memory pressure without improving tail latency reliably.

### 8.2 How to increase throughput

Recommended layering:

1. **Multiplex concurrency on a single stream (required)**
   - Allow multiple in-flight `FilterRequest` messages on the same bidirectional stream.
   - Use `correlation_id` to map responses back to awaiting callers (`inflight` map).

2. **Multiple agents / multiple streams (recommended)**
   - Support multiple filter agents connecting to the same `addr`.
   - Dispatch with least-inflight or round-robin across `AgentHandle`s.

3. **Backpressure and limits (required)**
   - `max_inflight_total` prevents Router overload when the filter is slow.
   - `per_agent_max_inflight` avoids saturating a single agent and protects P99.

4. **Payload slimming (strongly recommended)**
   - Cap `max_candidates_sent` (e.g. 32–128).
   - Send only summarized `signals`, never large payloads (ASR audio, long prompts).
   - Cap debug kv count/size.

5. **Timeout and late-response handling (required)**
   - Router enforces `decision_timeout_ms` locally and removes inflight entries after timeout.
   - Late `FilterResponse` messages are discarded and counted for observability.
