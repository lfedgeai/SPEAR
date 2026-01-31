# Task-level MCP server subset selection: Design (EN)

## Background

In real deployments, a task usually needs only a small subset of MCP servers (e.g. `gitlab` + `fs`), not the full MCP registry. Exposing everything to the model tends to:

- Increase cost and latency: injection needs `tools/list` per enabled server
- Reduce reliability: one flaky server degrades the whole experience
- Expand security risk: more visible tools increase the attack surface
- Hurt tool choice quality: the model is more likely to pick the wrong tool among many

We need a task-level MCP subset selection mechanism that stays aligned with the current architecture boundaries while being **default-minimal, governable, auditable, and extensible**. (Invocation-level override can be added later if needed.)

## Current state (based on existing code)

### MCP tool injection entrypoint (data plane)

Spearlet injects MCP tools before sending Chat Completions:

- Injection: [`src/spearlet/execution/host_api/cchat.rs`](../src/spearlet/execution/host_api/cchat.rs)
  - `cchat_inject_mcp_tools`: reads `snapshot.mcp` (the in-memory `McpSessionParams`), calls `tools/list` for selected servers, and injects tools
- Policy helpers: [`src/spearlet/mcp/policy.rs`](../src/spearlet/mcp/policy.rs)
  - `filter_and_namespace_openai_tools`: applies allow/deny filters and generates namespaced OpenAI tool defs
  - `parse_namespaced_mcp_tool_name`: routes tool calls back to `(server_id, tool_name)` (accepts both `mcp__...__...` and `mcp.<server_id>.<tool_name>`)

This means subset selection is already supported at runtime; what’s missing is a clean way to populate the chat session’s `McpSessionParams` from task intent.

### Current implementation scope (important)

As of the current code:

- Supported: task-level policy from `Task.config` is auto-applied to chat session params, with hostcall-level enforcement.
- Not supported: per-invocation override via invocation metadata/context_data (e.g. `mcp.server_ids` in metadata). This is intentionally deferred.

### MCP registry (control plane)

SMS owns the registry and can bootstrap it from a directory:

- Directory bootstrap: [`src/sms/service.rs`](../src/sms/service.rs) (`bootstrap_mcp_from_dir`)
- Registry proto: [`proto/sms/mcp_registry.proto`](../proto/sms/mcp_registry.proto)

Each `McpServerRecord` includes:

- `allowed_tools`: server-level allowlist patterns
- `budgets`: timeout/concurrency/output limits
- `approval_policy`: approval policy (extensible)

This is the basis for platform governance.

## Goals and non-goals

### Goals

- Support task-level default MCP server subset (default subset)
- Preserve a layered policy model: platform governance → task constraint → session selection (explicit session params; if absent, use task default)
- Failure-tolerant: empty subset means “inject none” (not “fail the task”); one server failure doesn’t affect others
- Auditable: record the effective server/tool set and rejection reasons

### Non-goals (not required in the first iteration)

- No mandatory new DB schema or complex UI
- No requirement to fully implement per-tool approval flows end-to-end (can extend `approval_policy` later)
- No requirement to fully standardize secret/auth resolution for HTTP transports (can be added later)

## Design principles (industry best practices)

1. Default deny, explicit opt-in
2. Layered narrowing: each layer can only reduce permissions, never expand them
3. Stable namespacing for tools to avoid collisions and improve auditability (you already use `mcp__...__...`)
4. Observability-first: injection/execution should emit structured logs/metrics
5. Minimize changes: reuse the existing `cchat_ctl_set_param` + `cchat_create` control path

## Core design: three-layer policy + subset composition

### Three layers

1. **Platform layer (SMS registry)**
   - Which MCP servers are eligible
   - Per-server `allowed_tools`, `budgets`, `approval_policy`
2. **Task layer (task default + upper bound)**
   - Task default enabled servers (default subset)
   - Task maximum allowed servers (allowed subset / upper bound)
3. **Session layer (chat session params selection)**
   - Requested subset for this chat session (usually task defaults, or explicitly narrowed by the WASM guest)

Effective set = `platform_allowed ∩ task_allowed ∩ session_requested` (if session request is absent, use task_default).

### Subset composition algorithm (recommend as a pure function)

Inputs:

- `registry_servers`: registry snapshot (server_id → record)
- `task_policy`: task defaults and constraints
- `session_policy`: session request from chat session params (optional)

Outputs:

- `effective_server_ids`: servers effectively injected in this chat session
- `effective_tool_allow/deny`: merged tool filters
- `decision_log`: exclusion reasons (missing env, not allowed, unknown server, deny-all server policy, etc.)

Rules:

1. `platform_allowed_server_ids = registry_servers.keys()`
2. `task_allowed_server_ids`:
   - if not explicitly provided, default to `task_default_server_ids` (least privilege)
3. `requested_server_ids`:
   - if session params provide it: use it
   - else: use `task_default_server_ids`
4. `effective_server_ids = requested ∩ task_allowed ∩ platform_allowed`
5. Per-server:
   - if `record.allowed_tools` is empty: skip (deny-all)
   - if `tools/list` fails: inject none for this server (do not affect other servers)
6. Tool filtering:
   - `effective_allowed = record.allowed_tools ∩ task.tool_allowlist ∩ session.tool_allowlist`
   - denylist is a final veto at any layer

## Configuration and data model (phased rollout)

We recommend two phases:

- Phase A: no proto changes; use `Task.config` for MCP selection policy (fastest)
- Phase B: add Profiles/Bundles and server tags (governance upgrade)

### Phase A: encode task-level subset policy in Task.config

`Task` already has `config: map<string,string>` (see [`proto/sms/task.proto`](../proto/sms/task.proto)). Define these keys (values as JSON strings):

- `mcp.enabled`: `"true" | "false"`
- `mcp.default_server_ids`: JSON array string, e.g. `["gitlab","fs"]`
- `mcp.allowed_server_ids`: JSON array string (optional; if absent, equals default)
- `mcp.tool_allowlist`: JSON array string (optional)
- `mcp.tool_denylist`: JSON array string (optional)

#### Relationship between allow and default (key semantics)

Treat these as two different sets with a strict relationship:

- `default_server_ids` (Default):
  - the servers enabled by default when the session does not explicitly override
  - meant to be the smallest “works out of the box” set
- `allowed_server_ids` (Allow / upper bound):
  - the maximum set the task is allowed to ever use
  - a hard ceiling: session params (and task code) can only narrow within this set

Invariants:

- `default_server_ids ⊆ allowed_server_ids`
- Recommended: if `mcp.enabled=true`, `default_server_ids` should be non-empty (otherwise “enabled but no default”)

Two common patterns:

1. **Least privilege (recommended default)**: `allowed_server_ids = default_server_ids`
2. **Larger allow, smaller default**: `allowed_server_ids ⊃ default_server_ids` (reserve room for session-level narrowing)

Recommended semantics:

- MCP is disabled unless `mcp.enabled=true`
- If `allowed_server_ids` is not set, treat it as `default_server_ids` (least privilege)
- If session params request servers outside `allowed_server_ids`, reject the request (and optionally record a reason)

#### Phase A integration points (aligned with the existing code)

The injection path depends on `ChatSessionState.mcp` (`snapshot.mcp`), typically populated via `cchat_ctl_set_param`. Phase A needs an automatic bridge:

- Before the first chat send, parse task.config into a structured `McpTaskPolicy`
- Compute the effective subset from task policy only (invocation-level override is deferred)
- Populate chat session defaults:
  - `cchat_create` applies task defaults via `cchat_apply_task_mcp_defaults`
  - Guest can further narrow via `cchat_ctl_set_param` (but cannot expand beyond task policy)

Implementation detail: task config parsing and enforcement is implemented in [`src/spearlet/mcp/task_subset.rs`](../src/spearlet/mcp/task_subset.rs) and enforced in [`src/spearlet/execution/host_api/cchat.rs`](../src/spearlet/execution/host_api/cchat.rs).

### Phase B: Profiles/Bundles and server tags

As MCP server counts grow, hardcoding server_id lists in each task becomes hard to maintain. Common production patterns:

- **Profile/Bundle**: `profile_name -> server_ids[] (+ allow/deny)`
- **Tags/Labels**: servers have tags; tasks request by tags (e.g. `["scm","search"]`) which maps to server_ids

Phase B options:

1. Add a profiles config in SMS (file-based or DB-backed)
2. (Optional) add Web Admin UI to manage profiles
3. Extend task config:
   - `mcp.profile = "code-review"`
   - `mcp.profile_overrides` to further narrow

Why Phase B is worth it:

- Lower task configuration overhead
- Central governance and auditability (profile changes can be reviewed)
- Environment isolation (dev/staging/prod profiles)

## Runtime behavior and failure policy

### Missing environment variables / server unavailable

If env references can’t be resolved, `tools/list` fails and injection for that server becomes empty. Recommended as an explicit contract:

- A single server injection failure only affects that server
- If the final injected set is empty, the chat still proceeds (the model just sees no MCP tools)

### Empty subset

- If `mcp.enabled=true` but the effective subset is empty, emit a warning and continue (unless you add a future “require at least one server” flag).

## Security and compliance

- All MCP servers must be registered and governed via SMS registry (platform allowlist)
- Task layer can only narrow, never expand
- Tool exposure is always constrained by `record.allowed_tools`
- Audit logs should include server_id, tool_name, and denial reasons

## Observability (recommended)

Suggested logs/metrics (to implement during rollout):

- `mcp_injection_total{server_id,status}`: injection attempts (success/failed/denied)
- `mcp_list_tools_latency_ms{server_id}`: list latency
- `mcp_effective_servers{task_id}`: effective server list per session (as structured log fields)
- `mcp_denied_reason{server_id,reason}`: reason distribution (unknown_server/not_allowed/env_missing/list_timeout/etc.)

## Migration plan

- Phase A: enable for a small set of tasks using `Task.config`, validate latency/cost improvements
- Phase B: introduce profiles and gradually replace `default_server_ids` with `profile` references
- Final: standardize subset selection as a platform capability with optional UI and governance workflows

## Appendix: examples

### Task.config example

```json
{
  "mcp.enabled": "true",
  "mcp.default_server_ids": "[\"gitlab\",\"fs\"]",
  "mcp.allowed_server_ids": "[\"gitlab\",\"fs\",\"duckduckgo-search\"]",
  "mcp.tool_allowlist": "[\"*\"]",
  "mcp.tool_denylist": "[\"delete_*\"]"
}
```

### Session override example (narrow to a smaller subset)

- Request `gitlab` only via chat session params (using `cchat_ctl_set_param`):
  - `mcp.server_ids=["gitlab"]`
  - The host validates `gitlab ∈ task.allowed_server_ids`; otherwise it rejects with `-EACCES`.
