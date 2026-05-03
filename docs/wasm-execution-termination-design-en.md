# WASM Terminate/Destroy Guard Design

## Background

The current WASM runtime executes guest code on a dedicated worker thread and exposes synchronous hostcalls via the `spear` import object.

- The runtime sets the execution context using a thread-local `CURRENT_WASM_EXECUTION_ID` before calling into the guest, and clears it afterwards. See `set_current_wasm_execution_id` in `src/spearlet/execution/host_api/core.rs` and its usage in `src/spearlet/execution/runtime/wasm.rs`.
- Most hostcalls return an integer error code (Linux errno-style negative values) via `Result<Vec<WasmValue>, CoreError>`, returning `Ok([i32])` even on failure.

Problem: When an execution must be terminated (timeout, manual terminate) or an instance must be destroyed, the guest may still be blocked inside a hostcall (e.g., `epoll_wait`, `sleep_ms`, or long-running network operations). The runtime-side control plane does not reliably preempt the guest thread; we need a cooperative guard checked at the beginning of every hostcall.

## Goals

- Add a cheap, consistent “should terminate?” check at the start of every WASM hostcall.
- If termination is requested, hostcalls must fail immediately with a well-defined error.
- Carry a human-readable termination message (reason) that can be surfaced for debugging/UX.
- Support two distinct behaviors:
  - Terminate execution (manual terminate/timeout): end only this execution, but allow the instance to run future executions.
  - Destroy instance: terminate current executions, release/destroy the instance, and ensure it is not runnable (future executions must be rejected or immediately terminated).

## Non-goals

- Preempting guest CPU-bound loops that do not call hostcalls.
- Implementing arbitrary interruption of external blocking syscalls inside host code beyond what hostcalls can cooperate with.

## Current Code Touchpoints

- Hostcall registrations: `src/spearlet/execution/runtime/wasm_hostcalls.rs` (`build_spear_import_with_api`).
- Execution context: `src/spearlet/execution/host_api/core.rs` (`CURRENT_WASM_EXECUTION_ID`).
- WASM worker invoke loop: `src/spearlet/execution/runtime/wasm.rs` (sets execution id before `vm.run_func*`).

## End-to-end Flow (UI -> API -> Termination State -> Control Execution/Instance)

This section fills in the concrete “button click -> which API -> how it reaches termination variables -> how it stops execution/instance” flow.

### A. User triggers “Terminate execution”

#### A1. Which API the UI should call

In this design, Terminate execution and Destroy instance are operational/admin controls and **do not require a SPEAR Console button**. They can be triggered from Web Admin or other operational tooling.

- **Web Admin (admin)**: call an SMS admin API (admin token/cookie)
  - `POST /admin/api/executions/{execution_id}/terminate`
  - body: `{ "reason": "admin terminated execution" }`

SMS resolves `execution_id -> node_uuid -> spearlet http_addr` and requests spearlet to terminate the execution.

#### A2. How SMS routes to the correct spearlet (Terminate execution, protobuf/gRPC)

Control-plane should not depend on WebSocket and does not need to go through the HTTP gateway. Prefer protobuf/gRPC end-to-end:

1) Call SMS `ExecutionIndexService.GetExecution(execution_id)` to obtain `execution.node_uuid`.
2) Call SMS `NodeService.GetNode(node_uuid)` to obtain `ip + grpc_port` (prefer `node.port`; fall back to metadata if needed).
3) SMS dials the owning spearlet gRPC endpoint `http://{ip}:{grpc_port}` and calls:
   - `spearlet.ExecutionService.TerminateExecution(execution_id, reason)`.

#### A3. How spearlet writes the “termination variables”

Inside spearlet, implement the terminate handler/service:

1) Parse `execution_id + reason`.
2) Lookup execution state and obtain `instance_id` (from `TaskExecutionManager.executions`; not found -> 404; already completed -> 409).
3) Write execution-scope termination:
   - `exec_termination.mark(execution_id, -ECANCELED, reason)`

#### A4. How it takes effect in hostcalls

- On the next entry into any hostcall, the guard checks `exec_termination`.
- If marked, it immediately returns an error / traps, ensuring termination at the next hostcall boundary.

#### A5. How the runtime converges to “terminated”

- `vm.run_func*` returns an error due to the termination trap.
- Runtime maps it to a terminated error/status (ideally `scope=Execution` plus the message).
- Execution completion is published and SMS indexing eventually shows `terminated/timeout` (timeout is a terminate reason).

### B. User triggers “Destroy instance”

Destroy instance semantics: all currently running executions on that instance must be terminated first, and the instance must not be reusable afterwards (release/destroy resources; to serve again, create a new instance).

#### B1. Which API the UI should call

Recommended as an admin-only action (no SPEAR Console entry required):

- `POST /admin/api/instances/{instance_id}/destroy`
- body: `{ "reason": "admin destroyed instance" }`

#### B2. How SMS routes to the correct spearlet (Destroy instance, protobuf/gRPC)

- Resolve `instance_id -> node_uuid` via SMS instance registry / execution index; if not available, fall back to “list recent executions for instance -> infer node_uuid”.
- Resolve `node_uuid -> spearlet gRPC addr` via `node_client.get_node` (ip + grpc_port).
- SMS dials spearlet gRPC and calls:
  - `spearlet.InstanceService.DestroyInstance(instance_id, reason)` (requires a new protobuf service definition; see implementation plan).

#### B3. How spearlet “terminates all executions first”

Implement destroy-instance in spearlet with a deterministic, idempotent sequence:

1) **Freeze the instance (block new executions)**
   - `instance_termination.mark(instance_id, -ECANCELED, "instance destroyed: {reason}")`
2) **Enumerate and terminate currently running executions (required)**
   - From `TaskExecutionManager`, collect `execution_id` where `status=running && instance_id==...`.
   - For each: `exec_termination.mark(execution_id, -ECANCELED, "instance destroyed: {reason}")`.
3) **Stop instance resources**
   - Call `runtime.stop_instance(instance)` (WASM: `WasmRuntime::stop_instance`).
4) **Reject future executions**
   - With the instance stopped and `instance_termination` still set, any future execution attempts must be rejected or immediately terminated.

#### B4. Important limitation

- A “check only at hostcall entry” is cooperative: if the guest is currently blocked inside a hostcall implementation (e.g. `sleep_ms` doing `thread::sleep`), entry checks cannot preempt it. Blocking hostcalls must become interruptible or periodically check termination to guarantee prompt kill.

## Proposed Design

### 1) Termination State Model

To support both “stop execution” and “kill instance”, we use two keys:

- Execution-scope (key=`execution_id`): affects only the current execution.
- Instance-scope (key=`instance_id`): affects all executions on the instance (current and future).

The termination state should store:

- `terminated: AtomicBool`
- `errno: i32` (recommended: `-libc::ECANCELED`)
- `message: Arc<parking_lot::Mutex<Option<String>>>` (termination reason)
- `scope: enum { Execution, Instance }` (distinguish stop-execution vs kill-instance)
- `ts_ms: u64` (optional; diagnostics)

### 2) Global Registry + Host API Field

We want two properties simultaneously:

1) Runtime-side code (terminate/timeout/destroy paths) can mark termination without holding a direct reference to the `DefaultHostApi` inside the WASM VM.
2) Hostcalls can check termination with minimal coupling.

Implement two registries and also keep handles in `DefaultHostApi`:

- `WASM_EXEC_TERMINATION_REGISTRY: OnceLock<DashMap<String, Arc<TerminationState>>>` keyed by `execution_id`.
- `WASM_INSTANCE_TERMINATION_REGISTRY: OnceLock<DashMap<String, Arc<TerminationState>>>` keyed by `instance_id`.
- Add two fields to `DefaultHostApi` (“the variable on the corresponding field”):
  - `exec_termination: Arc<WasmTerminationRegistry>`
  - `instance_termination: Arc<WasmTerminationRegistry>`

The `WasmTerminationRegistry` is a thin wrapper:

- `mark(execution_id, errno, message)`
- `clear(execution_id)` (to avoid stale flags)
- `check(execution_id) -> Option<TerminationSnapshot>`

### 3) Hostcall Guard Helper

In `wasm_hostcalls.rs`, define a single helper used by all hostcalls:

- Resolve `execution_id` via `current_wasm_execution_id()` (thread-local).
- Check execution-scope termination via `host_data.exec_termination`.
- Check instance-scope termination via `host_data.instance_termination` (requires `host_data.instance_id`).
- If either says `terminated == true`, return immediately / trap.

Semantics:
- Execution-scope termination stops only the current execution and should not affect future executions on the same instance.
- Instance-scope termination (kill instance) must stop the current execution and also ensure future executions are rejected or immediately terminated.

Return strategy (two-tier):

- **Tier A (soft)**: return `Ok([WasmValue::from_i32(-libc::ECANCELED)])` and optionally log the message.
  - Pros: keeps error handling consistent with current hostcall conventions.
  - Cons: guest might ignore the error and keep running.
- **Tier B (hard, recommended for “terminate instance/execution”)**: return `Err(CoreError::Common(UserDefError))` after recording/logging the termination reason.
  - Pros: guarantees the guest execution stops at the next hostcall boundary.
  - Cons: the error payload is not automatically propagated to guest memory; we need to expose it via logs or an explicit hostcall.

Recommendation:

- Default to **Tier B** for termination (to guarantee stopping).
- Still set a stable errno code in the termination state for metrics/logging and for any code paths that prefer soft termination.

### 4) Propagating the Termination Message

Because a trap (`CoreError`) cannot carry arbitrary message bytes to the guest, expose the message via one (or both) of:

- **WASM log ring**: on guard trip, write an error log that includes `execution_id`, `task_id`, `instance_id`, and the termination message. The runtime already has a per-execution log ring.
- **Optional hostcall**: `spear_termination_reason(out_ptr, out_len_ptr) -> i32`
  - Returns `-ENOTCONN` if no execution id, `-EAGAIN` if not terminated, `-ENOSPC` if buffer too small, otherwise writes the message and returns 0.

### 5) Marking Termination

Define explicit mark points and scopes:

- Terminate execution:
  - Terminate endpoint: `exec_termination.mark(execution_id, -ECANCELED, "terminated: ...")`.
  - Timeout: `exec_termination.mark(execution_id, -ECANCELED, "timeout: ...")` (include `timeout_ms`).
- Destroy instance:
  - Destroy-instance / fatal error: `instance_termination.mark(instance_id, -ECANCELED, "instance destroyed: ...")`.
  - Also mark the currently running execution if `execution_id` is known, to guarantee it exits at the next hostcall boundary.

Clearing:

- At the start of each `Invoke` (in the worker loop), call `exec_termination.clear(execution_id)` to remove stale execution flags.
- After execution ends, also call `exec_termination.clear(execution_id)` to avoid memory growth.
- Instance-scope termination is typically not cleared until the instance is rebuilt/replaced.

### 6) Execution-scope vs Instance-scope (required behaviors)

There are two common needs:

- **Execution-scope termination**: terminate only the current execution; allow later executions.
- **Instance-scope termination**: prevent any further execution on the instance (e.g., fatal error, manual stop).

Key difference:

- Execution-scope termination: affects only the current execution; runtime should allow future executions on the same instance.
- Instance-scope termination (destroy instance): affects all executions on the instance; runtime should transition the instance to Stopped/Failed and reject future executions.

### 7) Runtime Behavior on Termination

When `vm.run_func*` returns an error due to the termination trap:

- Map it to an explicit terminated error/status (ideally carrying the scope and message).
- Mark execution status as terminated/failed accordingly and publish completion event.
- If the trigger was instance-scope termination (kill instance), call `stop_instance` and prevent reuse.

## Implementation Plan (High-level)

1) Add termination registry + state types (new module under `src/spearlet/execution/host_api/` or `runtime/`).
2) Add `termination_registry` field into `DefaultHostApi`.
3) Add a guard helper in `wasm_hostcalls.rs` and call it at the top of each hostcall.
4) Wire marking/clearing at execution lifecycle boundaries (invoke start/end; terminate/timeout/destroy paths).
5) Add tests:
   - Unit test: mark terminated; verify hostcall returns termination error immediately.
   - Integration test: run a WASM that repeatedly calls `sleep_ms`; trigger termination and verify execution stops quickly.

## Compatibility Notes

- Guest code that expects Linux errno values should not use WASI libc `errno.h` constants directly; use SDK-provided constants to avoid mismatches.
- Termination checks only take effect at hostcall boundaries; long guest-only compute loops remain non-preemptible.
