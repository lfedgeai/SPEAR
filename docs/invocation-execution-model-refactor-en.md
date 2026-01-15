# Invocation/Execution/Instance Model Refactor (Design)

## Background

The current system mixes multiple concepts under the word “invoke/execution”, which creates confusion across:

- SMS Web Admin API (`/admin/api/executions`)
- Spearlet gRPC `FunctionService.InvokeFunction`
- Spearlet internal scheduling and instance lifecycle
- Runtime-level “function invocation” semantics

In practice, the admin page triggers Spearlet gRPC `InvokeFunction`, but the handler fan-outs into `handle_*_execution()` and delegates to `TaskExecutionManager::submit_execution()`.

Code pointers:

- SMS Web Admin BFF calls Spearlet gRPC `InvokeFunction` in spillback flow: [src/sms/web_admin.rs](../src/sms/web_admin.rs)
- Spearlet gRPC handler routes by execution mode: [src/spearlet/function_service.rs](../src/spearlet/function_service.rs)
- Execution submission currently builds an `ExecutionContext` and enqueues: [src/spearlet/execution/manager.rs](../src/spearlet/execution/manager.rs)
- Runtime (WASM example) executes by `function_name`: [src/spearlet/execution/runtime/wasm.rs](../src/spearlet/execution/runtime/wasm.rs)

## Goals

- Make the runtime layer expose only “invoke function” semantics.
- Clearly define and separate:
  - Function (what is called)
  - Invocation (client request)
  - Execution/Attempt (a concrete run on a node)
  - Instance (stateful runtime container for a task)
- Support reentrancy (concurrent invocations) as a controlled capability (capacity model).
- Prepare for interactive user I/O (terminal-like console) as a first-class invocation mode.
- Keep a practical migration path with minimal breakage.

## Non-goals

- Changing scheduling policy logic in SMS placement.
- Building a brand-new storage backend for execution history.
- A “perfect” exactly-once execution model.

## Definitions (Normative)

### Function

A callable entry inside a deployed artifact/task.

- Identified by `function_name`.
- May run short or long.
- Runtime executes a function with input + context.

### Task

The control-plane unit registered in SMS and used by placement.

- Identified by `task_id`.
- Binds artifact, runtime type, resource limits, scheduling hints.

### Instance

A stateful runtime container created on a Spearlet node to host a task.

- Identified by `instance_id`.
- Owned by Spearlet.
- Can handle one or more concurrent executions depending on capacity/reentrancy.

### Invocation

A user/client request to call a function.

- Identified by `invocation_id`.
- Stable across spillback/retry.
- Used for end-to-end tracing and aggregation.

### Execution (Attempt)

A concrete attempt to run an invocation on a specific node and (usually) a specific instance.

- Identified by `execution_id` (attempt id).
- There may be multiple executions for one invocation due to spillback.

## Invariants

- One execution runs exactly one function.
- One execution is bound to at most one instance.
- One invocation may produce multiple executions.
- Task/Instance lifecycle is an internal responsibility of Spearlet.

## Status Model

### ExecutionStatus (Attempt)

Suggested states:

- `PENDING` → `RUNNING` → `COMPLETED | FAILED | CANCELLED | TIMEOUT`

### InvocationStatus (Aggregate)

Aggregate over attempts:

- `ACCEPTED` (request accepted)
- `RUNNING` (any attempt running)
- `SUCCEEDED` (any attempt completed successfully)
- `FAILED` (all attempts failed and no more retries)
- `CANCELLED`

## Reentrancy & Capacity

Reentrancy is modeled as instance capacity.

- `capacity = 1`: non-reentrant, safest default.
- `capacity > 1`: reentrant, runtime must be concurrency-safe.

This aligns with existing pool/scheduler checks (healthy/ready/capacity).

## API Design

This section describes the target semantics and the migration approach.

### SMS Web Admin API (BFF)

#### New endpoint (recommended)

`POST /admin/api/invocations`

Request body:

- `task_id` (required)
- `function_name` (required)
- `execution_mode` (optional: `sync|async|stream|console`, default `sync`)
- `invocation_id` (optional, auto-generate if missing)
- `execution_id` (optional, auto-generate per attempt)
- `node_uuid` (optional: pin execution to a node, bypass placement)
- `max_candidates` (optional: spillback fanout limit)
- `timeout_ms` (optional)
- `session_id` (optional)
- `input_json` or `input_base64` (optional, one-of)

Response:

- `success`
- `invocation_id`
- `execution_id`
- `node_uuid`
- `message`
- `result` (sync only)

#### Console (terminal-like) interaction

For terminal-like interactive sessions (stdin/stdout/stderr + control messages like resize/signal), the BFF should provide a WebSocket endpoint that bridges browser/CLI I/O to Spearlet gRPC bidirectional streaming.

Recommended endpoint:

- `GET /admin/api/executions/{execution_id}/console` (WebSocket upgrade)

High-level message model:

- Client → BFF:
  - `stdin` bytes (raw)
  - `resize` events (`rows`, `cols`)
  - `signal` (e.g. `INT`, `TERM`)
  - `close`
- BFF → Client:
  - `stdout` bytes
  - `stderr` bytes
  - `status` updates
  - `exit` (final)

#### Breaking change policy

This refactor intentionally allows breaking API changes.

- Replace `POST /admin/api/executions` with `POST /admin/api/invocations`.
- Update all in-repo callers (admin assets, tests, tools) in the same change set.
- Do not keep alias endpoints for compatibility.

### Spearlet gRPC API

#### Target: new RPCs (breaking)

To avoid accumulating legacy semantics (“invoke” that actually means “submit execution / maybe create instance / maybe create task”), replace the current RPC surface with a clean API.

Key principles:

- Runtime-facing API expresses only function invocation.
- Invocation vs execution (attempt) is explicit.
- Control-plane actions (create/delete task) do not live inside the invocation RPC.
- Payload is canonical bytes + content type, not `Any` everywhere.

##### Packaging strategy

- Rewrite `proto/spearlet/function.proto` (or split into `invocation.proto` and `execution.proto`) to define the new services.
- Remove the old `FunctionService` and its messages.
- Update all in-repo clients to the new stubs.

##### v2 proto sketch

```proto
syntax = "proto3";

package spearlet;

import "google/protobuf/timestamp.proto";

enum ExecutionMode {
  EXECUTION_MODE_UNSPECIFIED = 0;
  EXECUTION_MODE_SYNC = 1;
  EXECUTION_MODE_ASYNC = 2;
  EXECUTION_MODE_STREAM = 3;
  EXECUTION_MODE_CONSOLE = 4;
}

enum ExecutionStatus {
  EXECUTION_STATUS_UNSPECIFIED = 0;
  EXECUTION_STATUS_PENDING = 1;
  EXECUTION_STATUS_RUNNING = 2;
  EXECUTION_STATUS_COMPLETED = 3;
  EXECUTION_STATUS_FAILED = 4;
  EXECUTION_STATUS_CANCELLED = 5;
  EXECUTION_STATUS_TIMEOUT = 6;
}

message Payload {
  string content_type = 1;
  bytes data = 2;
}

message Error {
  string code = 1;
  string message = 2;
}

message InvokeRequest {
  string invocation_id = 1;
  string execution_id = 2;

  string task_id = 3;
  string function_name = 4;

  Payload input = 5;
  map<string, string> headers = 6;
  map<string, string> environment = 7;

  uint64 timeout_ms = 8;
  string session_id = 9;
  ExecutionMode mode = 10;

  bool force_new_instance = 11;
  map<string, string> metadata = 12;
}

message InvokeResponse {
  string invocation_id = 1;
  string execution_id = 2;
  string instance_id = 3;

  ExecutionStatus status = 4;
  Payload output = 5;
  Error error = 6;

  google.protobuf.Timestamp started_at = 7;
  google.protobuf.Timestamp completed_at = 8;
}

message InvokeStreamChunk {
  string invocation_id = 1;
  string execution_id = 2;
  string instance_id = 3;

  ExecutionStatus status = 4;
  Payload chunk = 5;
  bool is_final = 6;
  Error error = 7;
  map<string, string> metadata = 8;
}

service InvocationService {
  rpc Invoke(InvokeRequest) returns (InvokeResponse);
  rpc InvokeStream(InvokeRequest) returns (stream InvokeStreamChunk);
  rpc OpenConsole(stream ConsoleClientMessage) returns (stream ConsoleServerMessage);
}

message TerminalSize {
  uint32 rows = 1;
  uint32 cols = 2;
}

enum ConsoleSignal {
  CONSOLE_SIGNAL_UNSPECIFIED = 0;
  CONSOLE_SIGNAL_INT = 1;
  CONSOLE_SIGNAL_TERM = 2;
}

message ConsoleOpen {
  InvokeRequest invoke = 1;
  TerminalSize initial_size = 2;
}

message ConsoleClientMessage {
  oneof msg {
    ConsoleOpen open = 1;
    bytes stdin = 2;
    TerminalSize resize = 3;
    ConsoleSignal signal = 4;
  }
}

message ConsoleExit {
  int32 code = 1;
  string message = 2;
}

message ConsoleServerMessage {
  string invocation_id = 1;
  string execution_id = 2;
  string instance_id = 3;

  oneof msg {
    bytes stdout = 10;
    bytes stderr = 11;
    ExecutionStatus status = 12;
    ConsoleExit exit = 13;
    Error error = 14;
  }
}
```
message GetExecutionRequest {
  string execution_id = 1;
  bool include_output = 2;
}

message Execution {
  string invocation_id = 1;
  string execution_id = 2;
  string task_id = 3;
  string function_name = 4;
  string instance_id = 5;
  ExecutionStatus status = 6;
  Payload output = 7;
  Error error = 8;
  google.protobuf.Timestamp started_at = 9;
  google.protobuf.Timestamp completed_at = 10;
}

message CancelExecutionRequest {
  string execution_id = 1;
  string reason = 2;
}

message CancelExecutionResponse {
  bool success = 1;
  ExecutionStatus final_status = 2;
  string message = 3;
}

message ListExecutionsRequest {
  string task_id = 1;
  string invocation_id = 2;
  uint32 limit = 3;
  string page_token = 4;
}

message ListExecutionsResponse {
  repeated Execution executions = 1;
  string next_page_token = 2;
}

service ExecutionService {
  rpc GetExecution(GetExecutionRequest) returns (Execution);
  rpc CancelExecution(CancelExecutionRequest) returns (CancelExecutionResponse);
  rpc ListExecutions(ListExecutionsRequest) returns (ListExecutionsResponse);
}
```

##### Removal in the old API

The following legacy concepts are removed from the new API:

- `InvocationType` / “create task via invoke” behavior
- `create_if_not_exists`
- `wait`

Task creation remains in SMS TaskService.

### HTTP Gateway

Current Spearlet HTTP gateway includes OpenAPI docs for `/functions/invoke` but handlers are mostly TODO.

Plan:

- Either implement `/functions/invoke` as a thin translation layer to gRPC InvokeFunction.
- Or remove/disable the misleading OpenAPI path until implemented.

## Internal Architecture (Target)

### Spearlet layers

1. **API adapter** (gRPC/HTTP)
2. **Invocation engine**
   - validate request
   - pick task
   - pick/create instance
   - run runtime invoke
3. **Runtime** (WASM/Process/Container)
4. **Execution store** (in-memory initially)

### Core internal types (recommended)

#### InvocationRequest (internal)

Fields:

- `invocation_id: String`
- `execution_id: String`
- `task_id: String`
- `function_name: String`
- `mode: ExecutionMode`
- `timeout_ms: u64`
- `session_id: Option<String>`
- `input: bytes` (canonical)
- `metadata: map<string,string>`

#### ExecutionRecord

- `execution_id`
- `invocation_id`
- `task_id`
- `instance_id` (if bound)
- `node_id`/`node_uuid` (if known)
- `status`
- `timestamps`
- `result` / `error`

### Data structures (in-memory)

Minimal practical store:

- `executions_by_id: execution_id -> ExecutionRecord`
- `executions_by_invocation: invocation_id -> Vec<execution_id>`

This enables:

- Admin to query an execution by id.
- Admin to show all attempts for a single invocation.

## Idempotency & Retries

### Retry policy

- Spillback retry is initiated by SMS Web Admin BFF, not by Spearlet.
- Spearlet treats each `(invocation_id, execution_id)` as a unique attempt.

### Idempotency

Target behavior:

- If a client repeats the same `execution_id`, return the existing record (idempotent).
- If a client repeats the same `invocation_id` with a new `execution_id`, create a new attempt.

This avoids accidental duplicated work when the BFF retries network calls.

## Cancellation

### CancelExecution

Cancels a single attempt (`execution_id`).

Future optional:

- `CancelInvocation(invocation_id)` to cancel all attempts.

## Practical Implementation Plan

This plan assumes we are allowed to break APIs and will update all in-repo callers.

### Phase 1: Rewrite proto & regenerate stubs (breaking)

Changes:

- Replace `FunctionService` with `InvocationService` and `ExecutionService`.
- Replace `InvokeFunctionRequest/Response` with `InvokeRequest/Response`.
- Add required `invocation_id` and `execution_id`.
- Replace `Any`-heavy payloads with canonical `Payload { content_type, bytes }`.

Acceptance:

- Project builds with regenerated proto code.

### Phase 2: Update Spearlet server implementation

Changes:

- Replace [function_service.rs](../src/spearlet/function_service.rs) with new service implementations.
- Update gRPC server registration in [grpc_server.rs](../src/spearlet/grpc_server.rs).
- Ensure execution records store `invocation_id`, `execution_id`, `function_name`, `instance_id`.

Acceptance:

- Invoke/stream/get/cancel/list work end-to-end locally.
- OpenConsole supports interactive stdin/stdout/stderr loop for one runtime.

### Phase 3: Update all in-repo callers

Targets (non-exhaustive, must be searched in codebase):

- SMS Web Admin BFF: [web_admin.rs](../src/sms/web_admin.rs)
- Tests:
  - [placement_spillback_e2e_tests.rs](../tests/placement_spillback_e2e_tests.rs)
  - [spearlet_fetch_task_from_sms_tests.rs](../tests/spearlet_fetch_task_from_sms_tests.rs)
  - any other tests using `proto::spearlet::*`
- Admin static assets under `assets/admin/` (update to new BFF endpoint names and payload shape).

Acceptance:

- `cargo test` passes.

### Phase 4: Remove legacy paths

Changes:

- Delete unused messages/services and dead code paths.
- Remove misleading HTTP gateway OpenAPI entries if not implemented.

## Testing Plan

### Unit tests

- Request validation:
  - reject empty function name (after Phase 2)
  - idempotency on repeated execution id

### Integration tests

- SMS Web Admin spillback flow:
  - multiple candidates
  - first node unavailable, second succeeds
  - verify invocation_id stable, execution_id differs per attempt

- Console interaction:
  - open console, send stdin, observe stdout/stderr
  - resize event does not break the stream
  - cancel/terminate ends with a final exit/status

### Observability checks

- Ensure execution record includes `invocation_id`, `task_id`, `function_name`, `instance_id`.

## Rollout & Rollback

### Rollout

- Land as an atomic change: proto + server + all callers updated together.

### Rollback

- Revert the breaking change commit(s) as a whole.

## Open Questions (Decision Defaults)

- Default function name if not provided during Phase 0: recommend `"__default__"` or a task-defined default.
- Input schema: recommend a canonical bytes payload with optional JSON helper.
