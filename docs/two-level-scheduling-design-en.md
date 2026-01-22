# Two-Level Scheduling Design (SMS Control Plane + Spillback)

## Overview

This document describes SPEAR’s two-level scheduling design:

- Level 1 (Cluster-level): SMS acts as the Control Plane and performs node placement decisions.
- Level 2 (Node-level): Spearlet performs in-node instance scheduling, concurrency control, and cold-start management.
- Spillback: if the selected node cannot accept the request (unavailable/overloaded/insufficient capacity/too much queueing), the client quickly retries on another candidate node.

The goal is to remove manual node selection while keeping low latency and a clean evolution path. The design leaves room for SMS horizontal scaling later, but does not implement it in the first phase.

## Current State and Constraints

### Existing building blocks

- SMS node registration and heartbeats: Spearlet registers to SMS and periodically heartbeats; SMS maintains the node list.
  - See: [registration.rs](../src/spearlet/registration.rs)
  - See: [node_service.rs](../src/sms/services/node_service.rs)
- SMS node resource tracking (CPU/memory/disk/load, etc.):
  - See: [resource_service.rs](../src/sms/services/resource_service.rs)
- Spearlet execution management and instance-level scheduling:
  - Entry: TaskExecutionManager::submit_execution (accepts InvokeFunctionRequest, enqueues into the execution loop)
    - See: [manager.rs](../src/spearlet/execution/manager.rs#L232-L340)
  - Instance scheduler: InstanceScheduler (RoundRobin/LeastConnections, etc.)
    - See: [scheduler.rs](../src/spearlet/execution/scheduler.rs)

### Missing pieces

- No cluster-level placement: users must manually pick a node.
- No spillback semantics: no unified retry/redirection behavior when a node cannot accept a request.

## Goals

- Introduce SMS placement without changing Spearlet’s core execution model.
- Define bounded spillback with clear retry budgets.
- Make decisions explainable (reason/score) for debugging and tuning.
- Keep the design compatible with future SMS horizontal scaling (statelessness, sharding, caching, leases).

## Non-goals

- SMS horizontal scaling is not implemented in this phase.
- SMS does not become a data-plane proxy in this phase; clients still call Spearlet directly.
- No strongly consistent global resource accounting.

## High-Level Architecture

### Roles

- SMS (Control Plane):
  - Maintains an approximate, eventually consistent view of nodes and resources.
  - Exposes placement APIs: given invocation requirements/constraints, returns a prioritized candidate list with explanations.
- Spearlet (Data Plane + final local decision):
  - Accepts execution requests and performs in-node scheduling.
  - Rejects quickly with recognizable error codes when overloaded or incapable, triggering spillback.
- Orchestration layer (Client/SDK or Admin BFF):
  - Calls SMS to get placement.
  - Calls Spearlet on candidate nodes (direct or via internal forwarding).
  - Performs spillback (bounded retries).

### Key idea

- Global (SMS) is fast and approximate; it chooses “likely feasible” nodes.
- Local (Spearlet) is accurate and final; it either accepts or rejects quickly.
- Spillback bounds the cost of a “wrong guess” to one or two fast retries.

### Sequence (simplified)

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

## Admin task submission: BFF mode (recommended when Spearlet is not publicly reachable)

When the Admin Page runs in a browser/public network and cannot directly reach Spearlet (private addressing, no public ingress, CORS/TLS/auth constraints), the common best practice is to introduce a BFF (Backend For Frontend).

### Goals

- Admin Page depends on a single stable HTTP entry (the BFF).
- SMS remains control-plane only (placement/metadata) and does not carry invocation payload traffic.
- BFF calls Spearlet over internal gRPC for execution and implements spillback.

### Components and boundaries

- Admin Page (browser): calls BFF via HTTP/JSON.
- Admin BFF (data plane for admin only):
  - External: HTTP (same-origin, simpler CORS).
  - Internal:
    - Calls SMS (placement, or temporarily composes placement from list_nodes + list_resources).
    - Calls Spearlet gRPC FunctionService.InvokeFunction.
- SMS (control plane): returns candidate nodes; does not proxy InvokeFunction payload.
- Spearlet (execution plane): only needs to be reachable from the BFF inside the network.

### Deployment forms (recommend 1 first, then 2)

#### Form 1: extend SMS WebAdminServer to act as the BFF (fastest)

- WebAdminServer is already an HTTP server for the admin UI and can be extended with `/api/admin/*` endpoints.
- Relevant code:
  - Admin server: [web_admin.rs](../src/sms/web_admin.rs)
- Note: this makes the SMS process host “admin data-plane” logic, but still keeps the principle that SMS does not proxy invocation payload. SMS only does placement; the BFF part calls Spearlet internally.

#### Form 2: a standalone admin-bff service (best long-term)

- Run BFF as a separate service/binary.
- Benefits: clean separation, independent rollout/limits/scaling, and avoids coupling control-plane availability to admin data-plane load.
- Keep API and internal modules identical between form 1 and form 2 to enable a smooth migration.

### Why the BFF should call Spearlet via gRPC (not Spearlet HTTP Gateway)

- Spearlet HTTP gateway’s `/functions/execute` is currently a TODO placeholder.
  - See: [http_gateway.rs](../src/spearlet/http_gateway.rs#L496-L514)
- Therefore, BFF should call Spearlet over gRPC: `FunctionService.InvokeFunction`.

### Public BFF APIs (recommended)

#### Submit execution

- `POST /api/admin/executions`

Recommended headers for idempotency and time budgeting:

- `X-Request-Id`: client-generated idempotency key; keep stable across retries
- `X-Total-Timeout-Ms`: end-to-end budget; BFF splits it into placement + per-node timeouts

Request (recommended fields; maps to Spearlet InvokeFunctionRequest):

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

Field notes (scheduling/execution):

- `node_selector`: hard constraints (labels/capabilities), forwarded to SMS placement.
- `spillback.max_attempts`: max nodes to try (including the first). Default 2.
- `spillback.per_node_timeout_ms`: timeout for each node attempt. Keep it small (3-10s).
- `execution_mode + wait`: sync/async semantics.

Note: in the current execution pipeline, `ExecutionContext.payload/headers/context_data` are still marked TODO for extraction. If you expect to pass `input` into runtimes, Spearlet needs follow-up work to extract request payload into `ExecutionContext`.
  - See: [submit_execution](../src/spearlet/execution/manager.rs#L232-L340)

Response (recommended fields):

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

#### Status and cancel

- `GET /api/admin/executions/{execution_id}`
- `POST /api/admin/executions/{execution_id}:cancel`

Note: BFF maintains an `execution_id -> node_uuid` index (MVP can be an in-memory map; later swap to KV/DB).

Recommended additional endpoint:

- `GET /api/admin/executions/{execution_id}/debug`: returns decision_id, candidate list, last error classification.

### BFF internal modules (recommended, method-level)

#### Core objects

```text
AdminBffService
  - placement_client: SmsPlacementClient
  - node_pool: NodeClientPool
  - executor: SpillbackExecutor
  - index: ExecutionIndex
  - policy: SpillbackPolicyDefaults
```

#### ExecutionIndex (execution routing index)

Purpose: status/cancel must route back to the node that actually started the execution.

- Key: `execution_id`
- Value: `{ node_uuid, node_grpc_addr, decision_id, created_at, last_status }`
- TTL guidance:
  - Sync: short TTL (10-30 minutes)
  - Async/long-running: longer TTL or persistence (future KV/DB)

#### NodeClientPool (node connection pool)

Purpose: avoid reconnecting on every spillback attempt.

- `get_function_client(node_grpc_addr) -> FunctionServiceClient<Channel>`
- Health: on connect failures, mark temporarily unhealthy and rebuild.
- Security: optional mTLS; at minimum, enforce an allowlist of internal subnets and only dial addresses returned by SMS.

#### SpillbackExecutor (orchestrator)

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
```

Mapping to existing Spearlet code paths:

- gRPC entry: `FunctionServiceImpl::invoke_function`
  - See: [function_service.rs](../src/spearlet/function_service.rs#L382-L686)
- In-node execution entry: `TaskExecutionManager::submit_execution`
  - See: [manager.rs](../src/spearlet/execution/manager.rs#L232-L340)

### Idempotency, retries, and “might have executed” semantics (engineering critical)

#### Idempotency key (request_id)

- BFF must require or generate `request_id` (e.g. `X-Request-Id`).
- BFF should map `request_id` into `InvokeFunctionRequest.execution_id` so downstream retries can be deduplicated.
  - Spearlet generates an execution id when it is missing; providing one improves idempotency.
  - See: [invoke_function](../src/spearlet/function_service.rs#L382-L420)

#### Retry boundaries

- Only auto-retry failures that are very likely “not submitted” (connect failures/UNAVAILABLE, explicit OVERLOADED).
- For `DEADLINE_EXCEEDED`, separate phases:
  - connect/handshake timeout: retryable
  - timeout while waiting for completion: non-retry by default to avoid double execution

#### Result convergence

- Sync: for “might have executed” timeouts, return `UNKNOWN` with `execution_id`; converge via status.
- Async: return `execution_id` immediately; use status polling or events.

### Rate limiting, backpressure, and resource protection (engineering critical)

- Per-user/tenant limit at BFF entry.
- Global inflight cap (`max_inflight_executions`) to protect internal services.
- Per-node inflight cap (`inflight_per_node`) to avoid overdriving a single node.
- Circuit-break overloaded nodes for a short cooldown to reduce herd effects.
- Enforce request body size limits and whitelist fields; for large inputs, upload to object storage/SMS file service and pass references.

### Security and access control (engineering critical)

- External auth: validate admin identity (cookie/JWT/SSO) and enforce RBAC per task.
- Internal auth: prefer mTLS for BFF→Spearlet/SMS; at minimum, allowlist internal subnets and only dial SMS-returned addresses.
- Audit: record `request_id/decision_id/execution_id/task_id/user` and never log secrets.

### Observability and troubleshooting (engineering)

- Structured logs keyed by `request_id`: candidate list, per-attempt node_uuid, error class, final decision.
- Add metrics: `bff_inflight{node_uuid}`, `bff_circuit_open_total{node_uuid}`.

### Testing and acceptance checklist (engineering)

- Unit: error classification → retry decision.
- Integration:
  - SMS returns 2 candidates, node#1 returns OVERLOADED, verify spillback to node#2.
  - node unreachable, verify bounded latency.
- Load: validate inflight limits and circuit breaker behavior.

### Error classification and spillback decision table

| Failure | Typical signal | Spillback? | Notes |
|---|---|---:|---|
| Node unreachable | connect failure / gRPC UNAVAILABLE | Yes | switch node |
| Timeout | gRPC DEADLINE_EXCEEDED | Yes (bounded) | might have started; be conservative for late-stage timeouts |
| Overload reject | business OVERLOADED / RESOURCE_EXHAUSTED | Yes | explicit reject, switching helps |
| Task missing | TaskNotFound | No (default) | recommended: on-demand fetch from SMS in Spearlet |
| Bad request | INVALID_ARGUMENT | No | retry won’t help |
| App/runtime error | FAILED_PRECONDITION / INTERNAL (business) | No | usually not node-dependent |

### End-to-end budget and deadline propagation

Best practice: BFF owns the “total budget” and propagates deadlines downstream.

- `total_timeout_ms`: from `X-Total-Timeout-Ms` or defaults (e.g. 30s for sync, 5s for async submit)
- `per_node_timeout_ms`: from request spillback or defaults
- `placement_timeout_ms`: keep small (200-500ms)

Constraint:

- `placement_timeout_ms + max_attempts * per_node_timeout_ms <= total_timeout_ms`

### Observability (required identifiers)

- `request_id`: Admin Page → BFF → SMS → Spearlet
- `decision_id`: from placement
- `execution_id`: from Spearlet (or BFF-specified for idempotency)

Suggested metrics:

- `bff_submit_total{status}`, `bff_spillback_attempts_histogram`
- `bff_placement_latency_ms`, `bff_invoke_latency_ms{node_uuid}`
- `bff_error_total{class}`

### Handling “task missing on target node” (critical)

If a Spearlet node does not have the task locally, it fetches the task/artifact from SMS on-demand, materializes them into the local caches, and then executes.

- See: [execute_existing_task_invocation](../src/spearlet/execution/manager.rs#L640-L661)
- Helper used by the on-demand path:
  - [fetch_and_materialize_task_from_sms](../src/spearlet/execution/manager.rs#L895-L936)
  - [ensure_artifact_from_sms](../src/spearlet/execution/manager.rs#L752-L804)
  - [ensure_task_from_sms](../src/spearlet/execution/manager.rs#L861-L893)

This enables placement to send an invocation to any healthy node without requiring pre-warming of task metadata on every node.

### Why this does not turn SMS into a gateway

- SMS only returns candidate nodes (control-plane) and does not proxy invocation payload.
- BFF is an admin-only data-plane entry and can be scaled independently; it avoids making the cluster control plane a bottleneck.

## Control-plane APIs (SMS)

This design prioritizes a “placement API” and avoids data-plane proxying.

### gRPC: PlacementService (proposed)

Add a new PlacementService in SMS proto alongside NodeService/TaskService.

#### PlaceInvocation

Request (illustrative fields):

```text
PlaceInvocationRequest {
  string request_id;               // idempotency key for retries (future)
  string task_id;                  // optional: existing task invocation
  string artifact_id;              // optional: caching/hotness hints
  string runtime_type;             // e.g. wasm/process/k8s
  map<string,string> node_selector;// label/capability constraints
  ResourceRequirements req;        // optional resource requirements
  SpillbackPolicy spillback;       // suggested retry budget
}

ResourceRequirements {
  double cpu_cores;                // optional
  int64 memory_bytes;              // optional
}

SpillbackPolicy {
  uint32 max_attempts;             // e.g. 2-3
  uint32 per_node_timeout_ms;      // e.g. 3000-10000
  bool   allow_requery;            // whether to call placement again if candidates exhausted
}
```

Response (illustrative fields):

```text
PlaceInvocationResponse {
  repeated CandidateNode candidates; // prioritized list
  string decision_id;                // tracing/observability
  string policy;                     // active policy name
}

CandidateNode {
  string node_uuid;
  string grpc_addr;                  // e.g. 10.0.0.1:50052
  string http_addr;                  // e.g. 10.0.0.1:8081 (optional)
  double score;
  string reason;                     // human-readable explanation
  map<string,string> debug;          // machine-readable breakdown (optional)
  string lease_token;                // reserved for future resource leases
  int64 lease_expire_at_unix_ms;     // reserved
}
```

#### ReportInvocationOutcome (recommended for reliability)

Purpose: allow SMS to learn overloaded/unavailable/timeout outcomes to reduce repeated bad placements.

```text
ReportInvocationOutcomeRequest {
  string decision_id;
  string node_uuid;
  string execution_id;
  Outcome outcome;              // SUCCESS / OVERLOADED / UNAVAILABLE / TIMEOUT / ERROR
  int64 latency_ms;
  string error_code;            // optional
}
```

This can be omitted in the MVP, but should be reserved in the design.

### HTTP: /placement (optional)

If HTTP gateway support is needed:

- `POST /api/v1/placement/invocations/place`
- `POST /api/v1/placement/invocations/report-outcome`

### HTTP→gRPC gateway mapping (engineering spec)

This section defines how the SMS HTTP gateway (Axum) should expose the new placement APIs and forward them to the gRPC PlacementService.

#### Routes (HTTP)

- `POST /api/v1/placement/invocations/place` → gRPC `PlacementService.PlaceInvocation`
- `POST /api/v1/placement/invocations/report-outcome` → gRPC `PlacementService.ReportInvocationOutcome`

Optional debug route:

- `GET /placement/nodes`: return current candidates and scores for troubleshooting (no state change)

#### JSON schemas

`POST /api/v1/placement/invocations/place`

Request:

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

Response:

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

Request:

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

Response:

```json
{ "success": true }
```

#### Gateway implementation requirements

- gRPC client wiring: `GatewayState` should include a `placement_client` (similar to `node_client` / `task_client`).
  - See: [GatewayState](../src/sms/gateway.rs)
  - See: [http_gateway.rs](../src/sms/http_gateway.rs)
- Add handlers/routes: introduce `handlers/placement.rs` and wire routes in the centralized router.
  - Pattern reference: [handlers/node.rs](../src/sms/handlers/node.rs)
- Error mapping:
  - gRPC `invalid_argument` → HTTP 400
  - gRPC `unavailable` → HTTP 503
  - gRPC `deadline_exceeded` → HTTP 504
  - otherwise → HTTP 500
- Timeouts: HTTP handlers should apply a short placement timeout (e.g. 200-500ms).
- Observability: always return `decision_id` and log with `request_id`.

Implementation style should follow existing SMS gateway patterns (handlers + routes):

- See: [gateway.rs](../src/sms/gateway.rs)
- See: [handlers/node.rs](../src/sms/handlers/node.rs)

## Data-plane semantics (Spearlet)

### Fast rejection and spillback triggers

To keep spillback cheap, Spearlet should return quickly with recognizable error codes when it clearly cannot accept:

- UNAVAILABLE: node cannot serve.
- OVERLOADED: node is overloaded (e.g. concurrency permit exhausted, queue too long).
- NO_CAPACITY: runtime/instance pool cannot satisfy (e.g. runtime unsupported, instance creation failed).

Candidate hook points (existing code entry points):

- TaskExecutionManager::submit_execution:
  - Today it enqueues the request and waits for the result.
  - Suggested evolution: an admission check before enqueue (semaphore availability / queue length threshold) and return OVERLOADED immediately.
  - See: [manager.rs](../src/spearlet/execution/manager.rs#L232-L340)
- InstanceScheduler:
  - If there is no available instance and instance creation fails, return NO_CAPACITY.
  - See: [scheduler.rs](../src/spearlet/execution/scheduler.rs)

## Spillback design

### Client-driven spillback (recommended for Phase 1)

Core rule: client/SDK performs bounded retries across the candidate list.

Pseudo-code:

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

Suggested defaults:

- max_attempts=2 (one spillback)
- per_node_timeout=3-10s (depends on sync/async)
- allow_requery=false (keep MVP simple)

### Node-driven spillback (optional Phase 2)

If Spearlet can infer “better go elsewhere”, it can return a redirect hint:

- Add `suggested_nodes[]` or `suggest_requery=true` in error payload.
- Client prefers these hints when retrying.

This is closer to Ray-style spillback but requires more cross-node awareness.

## SMS placement implementation plan (function/method-level)

This section proposes module layout and method signatures (Rust-style) for later implementation.

### Proposed modules

- `src/sms/services/placement_service.rs`
- `src/sms/handlers/placement.rs` (if HTTP)
- `src/sms/routes.rs` wiring (if HTTP)
- `src/sms/service.rs` implement gRPC PlacementService

### Key types

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

### Entry point: place_invocation

```rust
impl PlacementService {
    pub async fn place_invocation(
        &self,
        req: PlaceInvocationRequest,
    ) -> Result<PlaceInvocationResponse, Status>;
}
```

Suggested internal decomposition:

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

### Predicates (filters)

- `is_online(node)`: node.status == online.
- `heartbeat_fresh(node)`: node.last_heartbeat within timeout.
- `match_selector(node.metadata, req.node_selector)`: label/capability constraints.
- `resource_not_high_load(res)`: avoid overloaded nodes.

### Scoring

Use a simple weighted model (O(N) is fine for MVP):

- `cpu_idle = 1 - cpu_usage_percent/100`
- `mem_idle = 1 - memory_usage_percent/100`
- `load = 1 - clamp(load_average_1m / load_high_watermark_1m)`
- `penalty = recent_failure_rate * recent_failure_penalty`

Final: `score = w1*cpu_idle + w2*mem_idle + w3*load - penalty`.

Return score breakdown via CandidateNode.reason/debug.

## Leaving room for SMS horizontal scaling

Even though scaling is not implemented now, keep these design properties to avoid rewrites:

- Keep PlacementService stateless or soft-state (cache can be dropped).
- Include `request_id` in requests for idempotency.
- Include `decision_id` in responses for tracing and outcome reporting.
- Reserve `lease_token` for future resource leases.
- Decouple policy from storage/caching:
  - `trait PlacementPolicy { fn filter(...); fn score(...); }`
  - `trait PlacementStateStore { async fn get_node_snapshot(...); }`

## Rollout plan (recommended)

### Phase 1: MVP

- Implement PlaceInvocation (return 2 candidates).
- Client/CLI/SDK or Admin BFF calls placement first.
- If internal connectivity is available: orchestrator calls Spearlet directly.
- If Spearlet is not publicly reachable: Admin Page uses the BFF to trigger execution.
- Spillback: client-driven only, max 2 attempts.

### Phase 2: Reliability

- Implement ReportInvocationOutcome.
- Penalize/circuit-break high-failure nodes.

### Phase 3: Resource leases (optional)

- Issue short TTL leases at placement time to reduce herd effects.

## Engineering decisions to finalize before implementation

- Where the Client/SDK entry lives (CLI, HTTP gateway, or a shared library).
- Minimal field set for placement requests: runtime_type/selector.
- Standardize Spearlet overload error codes (gRPC Status + details or custom error struct).
