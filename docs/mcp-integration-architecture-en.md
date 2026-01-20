# MCP Integration Architecture (Registry + Tool Injection + Hostcalls)

## Overview

This document proposes how to integrate external MCP (Model Context Protocol) servers into Spear, aligned with common industry practices:

- Spear stores a registry of MCP servers that are allowed to connect.
- At the agent layer, MCP tools are exposed to Chat Completions as standard `tools` so the agent can be unaware of MCP.
- Additionally, a dedicated set of MCP hostcalls is provided so WASM workloads can call MCP tools programmatically.

The design is intended to reuse Spear’s existing fd-based hostcall model and its existing “auto tool call loop” for Chat Completion.

## Goals

- Centralize external MCP server configuration, policy, and credentials in Spear.
- Make MCP tools available to Chat Completion tool-calling without requiring agent-side MCP awareness.
- Provide an fd-based MCP hostcall API for explicit tool calls from WASM.
- Ensure safety by default (deny-by-default, allowlists, namespacing, budgets, auditability).
- Support multiple transports (stdio for local subprocess; Streamable HTTP for remote).

## Non-goals

- Implementing a full MCP gateway for third-party clients outside Spear.
- Implementing every MCP capability category on day one (resources/prompts can be phased in).
- Allowing arbitrary, user-provided subprocess spawning without policy controls.

## Current Spear foundations to reuse

- Chat Completion hostcalls are fd-based (`cchat_create/write_msg/write_fn/ctl/send/recv/close`).
- Spear already supports auto tool-calling on the host side: the host loops on model `tool_calls`, executes tools, appends `role=tool` messages, and continues until no more tool calls.

References:

- Chat Completion hostcall design: [chat-completion-en.md](./api/spear-hostcall/chat-completion-en.md)
- Existing auto tool-call loop implementation: [cchat.rs](../src/spearlet/execution/host_api/cchat.rs)

## Terminology

- **MCP Server**: an external process/service exposing `tools/list` and `tools/call`.
- **Spear MCP Registry**: Spear-managed list of allowed MCP servers and policies.
- **Tool injection**: converting MCP tools into OpenAI-compatible `tools` entries used by Chat Completions.
- **Namespaced tool name**: a stable name that avoids collisions, e.g. `mcp.<server_id>.<tool_name>`.

## High-level architecture

### Components

1. **MCP Registry Service (control plane)**
   - Implemented in SMS as the single source of truth.
   - Stores MCP server registrations, policy, and credential references.
   - Spearlets fetch and cache registry data from SMS (revision-based).

2. **MCP Client Pool (data plane, per Spearlet)**
   - Maintains connections to MCP servers.
   - Provides:
     - tools discovery (with caching)
     - tool execution with timeouts, concurrency limits, output caps

3. **Chat Completion Tool Bridge (data plane, per chat session)**
   - Determines which MCP servers are enabled for the current chat session.
   - Injects MCP tools into the upstream chat request.
   - Routes tool calls returned by the model to either WASM tools or MCP tools.

4. **MCP Hostcall Surface (data plane, for WASM workloads)**
   - Exposes an fd-based API to connect/list/call MCP tools explicitly.

### Two usage modes

- **Agent-unaware mode (recommended default)**
  - The agent only uses Chat Completions tool calling.
  - Spear injects MCP tools and executes them automatically.

- **Programmable hostcall mode**
  - WASM code calls MCP tools directly via `mcp_*` hostcalls.

## MCP Server Registry

SMS is the authoritative registry owner. Spearlet should not persist registry state beyond caching.

### Registry record

Recommended fields (minimal set for production governance):

- `server_id` (string, unique, stable)
- `display_name` (string)
- `transport` (`stdio` | `streamable_http` | `http_sse_legacy`)
- `stdio` (optional):
  - `command` (string)
  - `args` (string[])
  - `env` (map<string,string> or references)
  - `cwd` (string)
- `http` (optional):
  - `url` (string)
  - `headers` (map<string,string> or references)
  - `auth_ref` (reference to Spear secret store)
- `tool_namespace` (string, default `mcp.<server_id>`)
- `allowed_tools` (patterns, default deny-all)
- `approval_policy` (per tool or per server: `never` | `always` | `policy`) 
- `budgets`:
  - `tool_timeout_ms`
  - `max_concurrency`
  - `max_tool_output_bytes`

### Registry operations

Control-plane operations (recommended via CLI/gRPC/HTTP admin API):

- Register / update / delete MCP server
- List MCP servers
- Health status & last error (observability)

Data-plane access patterns:

- Spearlet fetches from SMS and caches registry records (TTL + revision/version checks).
- Spearlet connects lazily (on first tool usage) unless pre-warming is configured.

## Tool naming and collision avoidance

Best practice is to avoid flat namespaces. Spear should expose MCP tools to the model using a deterministic namespace.

- External tool name presented to the model: `mcp.<server_id>.<tool_name>`
- Internal routing: parse prefix, map to `(server_id, tool_name)`

This makes audit logs and policy enforcement straightforward and avoids tool name conflicts across servers.

## Tool injection into Chat Completion

### Selecting which MCP servers are enabled per session

Recommended policy:

- Default: no MCP servers enabled.
- Enable via chat session params (existing `cchat_ctl_set_param` path):
  - `mcp.enabled`: boolean
  - `mcp.server_ids`: string[]
  - Optional: `mcp.tool_allowlist`: string[] patterns (further restrict for this session)

Current implementation note:

- Task defaults may be applied automatically from `Task.config` when a chat session is created.
- Invocation-level override via invocation metadata/context is currently not supported.

This keeps the WASM API surface stable and makes MCP enablement explicit.

### Letting users choose MCP tools (best practices)

In industry deployments, “user selects which MCP tools can be called” is typically implemented as layered allowlisting and scoping, instead of exposing a large, flat tool set to the model.

- Layer 1 (platform/admin): only allow vetted MCP servers in the registry; default deny.
- Layer 2 (tenant/user): users enable integrations (servers) and optionally a read-only subset.
- Layer 3 (session/request): narrow down the enabled server_ids and tool patterns for a specific run.

Recommended session params (set via existing `cchat_ctl_set_param`):

- `mcp.enabled`: boolean
- `mcp.server_ids`: string[] (required to enable any MCP tools)
- `mcp.tool_allowlist`: string[] patterns (optional)
- `mcp.tool_denylist`: string[] patterns (optional)

Recommended tool calling policy (passed to the upstream model as request params):

- `tool_choice = "none"`: user explicitly disables tool calling for this request.
- `tool_choice = "auto"`: default; model may choose among the already-filtered tools.
- `tool_choice = {"type":"function","function":{"name":"mcp.<server_id>.<tool_name>"}}`: user selected a specific tool; force the model to use it.

Product UX guideline:

- Prefer presenting “integrations/capability packs” (by server) over raw tool lists.
- Keep the injected tool set small; enable write tools only with additional approval or explicit user action.

### Building the upstream request

When building the Chat Completions request, Spear constructs:

- `tools = wasm_tools + mcp_tools`
  - wasm_tools: already registered via `cchat_write_fn`
  - mcp_tools: fetched from MCP servers via `tools/list` and filtered by:
    - registry `allowed_tools`
    - per-session allowlist overrides
    - global governance policies

### Executing tool calls

Spear reuses the existing auto tool-call loop:

1. Send chat request with injected `tools`.
2. If the model returns `tool_calls`:
   - For each call:
     - If name matches a WASM tool, invoke WASM function by `fn_offset`.
     - If name matches `mcp.<server_id>.<tool_name>`, call MCP `tools/call`.
   - Append each tool result as `role=tool` with the correct `tool_call_id`.
3. Repeat until no more tool calls or budgets are exceeded.

Budgets and safety limits should be enforced exactly the same way for both WASM and MCP tools:

- `max_iterations`
- `max_total_tool_calls`
- `max_tool_output_bytes`
- per-tool timeout

## MCP hostcalls (programmable API)

### Design principles

- fd-based, syscall-like API consistent with existing `cchat_*`.
- Avoid exposing raw spawning capabilities without registry policy.
- Prefer calling registered servers by `server_id`.

### Proposed hostcall set

#### 1) `mcp_open(server_id) -> mcp_fd`

- Opens a session/connection handle to a registered MCP server.
- `server_id` is resolved via the registry.
- The host establishes (or reuses) a connection from the client pool.

#### 2) `mcp_list_tools(mcp_fd, out_buf, out_len_ptr) -> rc`

- Returns JSON with a stable schema:

```json
{
  "server_id": "fs",
  "tools": [
    {"name": "read_file", "description": "...", "inputSchema": {"type":"object", "properties":{}}}
  ]
}
```

#### 3) `mcp_call_tool(mcp_fd, tool_name, args_json, out_buf, out_len_ptr) -> rc`

- `tool_name` is the MCP-native name (without the `mcp.<server_id>.` prefix).
- `args_json` is a UTF-8 JSON string.
- Returns JSON string output (success or error) in `out_buf`.

#### 4) `mcp_close(mcp_fd) -> rc`

- Releases the handle; the host may keep pooled connections alive.

### Optional hostcalls

If WASM needs discovery of registered servers:

- `mcp_registry_list(out_buf, out_len_ptr) -> rc`
- `mcp_registry_get(server_id, out_buf, out_len_ptr) -> rc`

Registry mutation (register/update/delete) is recommended to remain in control-plane APIs, not hostcalls.

## Security and governance

Recommended best practices:

- **Deny-by-default**: no MCP server enabled unless explicitly configured.
- **Allowlist tools**: registry-level `allowed_tools` patterns; optionally restrict further per session.
- **Namespacing**: avoid collisions and make audits unambiguous.
- **Credential indirection**: store secrets in Spear secret store and reference them by `auth_ref`.
- **Approval hooks**: for sensitive tools, require approval by policy (human or programmatic).
- **Network controls**: restrict egress for Streamable HTTP servers.
- **stdio hygiene**: ensure MCP stdio protocol stream is not polluted by logs; send logs to stderr.

## Observability

Minimum recommended signals:

- Per server: connection status, reconnect count, last error, average latency
- Per tool: call count, error rate, timeout rate, p50/p95 latency, output bytes
- Per chat session: total tool calls, iterations, budget hits

Audit logging (policy dependent):

- `request_id`, `session_id`, `server_id`, `tool_name`, timestamp, status
- Redact sensitive arguments (configurable)

## Failure handling

- If a server is unavailable:
  - Tool calls should return a structured error JSON and be appended as `role=tool`.
- If tool output exceeds caps:
  - Truncate with an explicit marker and a structured error field.
- If the model loops:
  - Enforce `max_iterations` and `max_total_tool_calls`.

## Suggested rollout plan

1. Phase 1: Registry + tool injection + MCP tool execution in Chat Completion loop.
2. Phase 2: Add MCP hostcalls (`mcp_open/list_tools/call_tool/close`).
3. Phase 3: Add resources/prompts support if needed.
4. Phase 4: Add gateway mode (optional) for large deployments.

## Engineering design details

This section is implementation-oriented. It proposes concrete module boundaries, data structures, execution flows, concurrency/budget enforcement, error model, and a minimal test plan.

### Code layout (recommended)

Split the MCP integration into three independent parts: registry (in SMS, control plane), client (in Spearlet, data plane), and bridge/hostcalls (in Spearlet, integration plane).

- `src/sms/mcp/registry/`
  - `types.rs`: registry record, policy, budgets
  - `store.rs`: persistent store + revision
  - `service.rs`: registry business logic (CRUD + validation)
  - `http.rs`: public API (`/api/v1/mcp/*`) and admin API (`/admin/api/mcp/*`)
- `src/spearlet/mcp/registry_client/`
  - `client.rs`: fetch registry from SMS (revision-aware)
  - `cache.rs`: in-memory cache (TTL + revision)
- `src/spearlet/mcp/client/`
  - `transport/mod.rs`: `McpTransport` trait
  - `transport/stdio.rs`: stdio subprocess transport
  - `transport/http_streamable.rs`: Streamable HTTP transport
  - `jsonrpc.rs`: JSON-RPC 2.0 encode/decode
  - `types.rs`: MCP Tool/CallResult structs
  - `pool.rs`: connection pool, concurrency, reconnect, health
  - `cache.rs`: tools/list cache (TTL + revision)
- `src/spearlet/mcp/bridge/`
  - `tool_injection.rs`: MCP tools -> OpenAI tools mapping + filtering
  - `router.rs`: parse and route `mcp.<server_id>.<tool_name>`
  - `policy.rs`: session allow/deny + approval policy enforcement
- `src/spearlet/execution/host_api/mcp.rs`
  - MCP fd API (`mcp_open/list_tools/call_tool/close`) using the shared pool

### Configuration and registry persistence

#### 1) SMS config (registry file)

Because SMS is the authoritative registry owner, the recommended workflow is to load MCP server registrations into SMS.

SMS can be configured to load a registry file at startup and upsert entries into the registry.

#### 1.1) Loading MCP server configs from a configurable directory (recommended)

If you want “MCP configs live under a directory and SMS discovers all supported MCP server configs”, this is a common production practice: one file per server, easy review, easy rollback.

Suggested behavior:

- SMS exposes a directory path (or CLI flag).
- On startup, SMS scans the directory, parses each config file into a server record, and upserts into the registry keyed by `server_id`.
- Optional reload: SIGHUP, periodic polling, or a Web Admin/CLI-triggered rescan.
- Spearlet never loads registry state from disk; it only fetches from SMS.

Suggested config naming (examples):

- SMS:
  - CLI: `--mcp-registry-dir <DIR>`
  - ENV: `SMS_MCP_REGISTRY_DIR=<DIR>`
  - Config: `mcp.registry_dir = "..."`

Directory scan rules (recommended):

- Read only `*.toml` and `*.json` (an initial implementation can start with one).
- Non-recursive by default (optional recursion).
- Ignore hidden/temp files (`.*`, `~`, swap files).
- Do not follow symlinks by default.

File schema: prefer “one file per server record”.

Example (TOML, single server per file):

```toml
version = 1
server_id = "fs"
display_name = "Filesystem"
transport = "stdio"
tool_namespace = "mcp.fs"
allowed_tools = ["read_*", "search_*"]

[stdio]
command = "uvx"
args = ["xxx@latest"]

[budgets]
tool_timeout_ms = 8000
max_concurrency = 8
max_tool_output_bytes = 65536
```

Environment variables and references:

- `stdio.env` supports environment references in values:
  - Required: `${ENV:VAR_NAME}`
  - With default: `${ENV:VAR_NAME:-default_value}`
- `stdio.env_from` (directory-loaded config only) is a shorthand for “pass through these environment variables”:
  - `env_from = ["API_TOKEN"]` expands to `env.API_TOKEN = "${ENV:API_TOKEN}"`
- Behavior when an environment variable is missing:
  - If a required reference (no default) cannot be resolved, Spearlet will treat `tools/list` as failed and inject no tools for that MCP server in the chat session.
  - If a default is provided, injection proceeds using the default value.
- Example GitLab MCP config: [config/sms/mcp.d/gitlab.toml](../config/sms/mcp.d/gitlab.toml)

Merge strategy:

- default: upsert (same `server_id` overwrites)
- optional conflict detection: if multiple files define the same `server_id`, apply lexicographic-last-wins and emit a warning with file sources
- optional: `--dry-run` to validate and show diffs
- optional: `--strict` to reject unknown fields / validation failures

#### 2) Cluster mode (SMS-managed registry)

Expose registry CRUD + revision in SMS, cache it in Spearlet:

- gRPC `McpRegistryService` (proto-based)
  - `ListMcpServers` (list + revision)
  - `WatchMcpServers` (server-side streaming)
  - `UpsertMcpServer` / `DeleteMcpServer` (admin only)

The Spearlet data plane only needs read access.

#### 2.0) Spearlet data-plane fetch contract (recommended)

Spearlet should fetch registry data from SMS with a revision-aware API.

- RPC: `McpRegistryService.ListMcpServers`
- Response:

```json
{
  "revision": 123,
  "servers": [
    {"server_id":"fs","transport":"stdio","allowed_tools":["read_*"]}
  ]
}
```

Spearlet cache rules:

- Cache by `revision` and a TTL.
- If `revision` is unchanged, skip rebuilding injected tool lists.

#### 2.0.1) Observing SMS updates (best practices)

To ensure Spearlet updates its cache promptly when SMS registry changes, use a push+pull approach:

- Push (primary): a gRPC server-side streaming watch that notifies Spearlet of registry changes.
- Pull (fallback): periodic `ListMcpServers` calls to guarantee eventual consistency.

This avoids relying on a single mechanism; watch streams can disconnect and polling alone is either slow or expensive.

Recommended API patterns:

- Watch RPC (gRPC server-side streaming):
  - `McpRegistryService.WatchMcpServers(WatchMcpServersRequest{ since_revision }) -> stream`
  - Event payload should be lightweight:

```json
{"revision": 124, "upserts": ["fs"], "deletes": ["jira"]}
```

- Poll RPC (existing list):
  - `McpRegistryService.ListMcpServers` returns `{revision, servers}`.

Spearlet cache update algorithm:

1. On startup, fetch list and store `revision` + snapshot.
2. Start watch stream from `since_revision=revision`.
3. On each watch event:
   - Update a local `target_revision`.
   - Trigger a refresh (either full list fetch or incremental changes fetch).
4. Run a low-frequency poll loop with jitter (e.g. 30s~120s):
   - If SMS `revision` > local `revision`, refresh.

Reliability and safety recommendations:

- Use exponential backoff for watch reconnects; include jitter.
- If watch returns `failed_precondition` (since_revision too old) or the stream ends with an error, do a full `ListMcpServers` resync and restart watch from the latest revision.
- Apply cache updates via “build new snapshot then swap” (atomic replace).
- If refresh fails, keep serving with the old snapshot and mark the registry cache as degraded.
- Drive dependent invalidations on revision change:
  - tool injection set rebuild
  - per-server MCP connection restart if transport config changed
  - tools/list cache invalidation by `server_id`

### Rust MCP client library choice

For Rust, the recommended approach is to use the official Rust MCP SDK when it satisfies requirements, instead of re-implementing JSON-RPC + transports by hand.

- Preferred: `rmcp` from the official MCP Rust SDK: https://github.com/modelcontextprotocol/rust-sdk
  - Pros: protocol types, stdio child-process transport, capability negotiation, fewer bespoke bugs.
  - Cons: extra dependency surface; may still require custom glue for Streamable HTTP depending on the transport support level.
- Alternative: implement a minimal, Spear-only MCP client using existing deps (`tokio`, `serde_json`, `reqwest`) if:
  - You only need `tools/list` and `tools/call` for a constrained subset.
  - You want strict control over IO, logging, and resource limits.

If `rmcp` is adopted, document the pinned version/features in `Cargo.toml` and keep all MCP-specific logic behind the `src/spearlet/mcp/` boundary.

### Implementation notes

- Proto and SMS gRPC service: [mcp_registry.proto](../proto/sms/mcp_registry.proto)
- SMS service implementation: [service.rs](../src/sms/service.rs)
- Spearlet registry sync (watch+poll cache): [registry_sync.rs](../src/spearlet/mcp/registry_sync.rs)
- Chat Completion MCP tool injection and execution: [cchat.rs](../src/spearlet/execution/host_api/cchat.rs)
- MCP client wrapper (stdio only): [client.rs](../src/spearlet/mcp/client.rs)

Runtime params (per chat session):

- `mcp.enabled`: bool
- `mcp.server_ids`: string[]
- `mcp.tool_allowlist`: string[] patterns
- `mcp.tool_denylist`: string[] patterns

#### 2.1) Web Admin: registry management tab

To support operational workflows and self-service onboarding, add a dedicated tab in SMS Web Admin (e.g. `MCP Servers`) to manage external MCP server registrations.

Recommended UI capabilities:

- List: server_id, transport, status (Connected/Degraded/Down), last error, tool count, updated_at
- Create/Edit: form for stdio/http config, allowed_tools, budgets, approval policy
- Delete: confirmation required
- Connection test (optional): trigger a `tools/list` and show a short summary
- Tool preview (optional): show the post-filter tool list (including namespaced names)
- Import from file (optional): upload a registry file, validate, then upsert

Recommended backend endpoints under the existing `/admin/api` prefix, reusing current optional auth:

- `GET /admin/api/mcp/servers`
- `GET /admin/api/mcp/servers/{server_id}`
- `POST /admin/api/mcp/servers`
- `PUT /admin/api/mcp/servers/{server_id}`
- `DELETE /admin/api/mcp/servers/{server_id}`
- `POST /admin/api/mcp/servers/{server_id}/test` (optional)
- `POST /admin/api/mcp/servers/import` (optional)

### Function/method-level details (recommended)

This subsection proposes concrete Rust-level function boundaries to reduce ambiguity during implementation.

#### SMS: registry store and service

Core store trait:

```rust
pub trait McpRegistryStore {
    fn revision(&self) -> u64;
    fn list_servers(&self) -> Result<Vec<McpServerRecord>, RegistryError>;
    fn get_server(&self, server_id: &str) -> Result<Option<McpServerRecord>, RegistryError>;
    fn upsert_server(&self, record: McpServerRecord) -> Result<u64, RegistryError>;
    fn delete_server(&self, server_id: &str) -> Result<u64, RegistryError>;
}
```

Service layer:

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

gRPC handlers (shape, not final names):

```rust
async fn list_mcp_servers(...) -> Result<ListMcpServersResponse, Status>;
async fn watch_mcp_servers(...) -> Result<tonic::Response<impl Stream<Item = Result<WatchMcpServersResponse, Status>>>, Status>;
async fn upsert_mcp_server(...) -> Result<UpsertMcpServerResponse, Status>;
async fn delete_mcp_server(...) -> Result<DeleteMcpServerResponse, Status>;
```

#### Spearlet: registry client and cache

Registry client:

```rust
pub struct McpRegistryClient {
    sms_base_url: String,
    http: reqwest::Client,
}

impl McpRegistryClient {
    pub async fn list_servers(&self) -> Result<(u64, Vec<McpServerRecord>), RegistryError>;
}
```

Cache:

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

### Core data structures (recommended)

#### Registry

- `McpServerRecord`
  - `server_id`, `transport`, `stdio/http` config
  - `tool_namespace` (default `mcp.<server_id>`)
  - `allowed_tools` patterns
  - `approval_policy`
  - `budgets`

#### Client & Tool

- `McpToolDescriptor`
  - `name`, `description`, `input_schema`
  - `server_id`
  - `namespaced_name` (model-facing)

- `McpCallRequest`
  - `server_id`, `tool_name`
  - `arguments` (JSON object)
  - `timeout_ms`, `max_output_bytes`

### MCP client: connect, discover, call

#### Connection pool

Maintain a per-`server_id` state machine:

- stdio: long-lived subprocess (optional idle reaping), restart on disconnect
- HTTP: keep session state if needed, reconnect with backoff

Enforce per-server budgets:

- `Semaphore` for `max_concurrency`
- `timeout` around each `tools/call`
- health tracking: `Connected/Degraded/Down` + `last_error`

#### tools/list cache

Avoid listing tools on every `cchat_send`.

- cache key: `(server_id, policy_hash)`
- TTL: e.g. 30s~5min (configurable)
- short failure TTL (1~5s) to prevent thundering herds

### Chat Completion: selection, injection, routing, execution

#### Session params

Set via existing `cchat_ctl_set_param`:

- `mcp.enabled`: bool
- `mcp.server_ids`: string[]
- `mcp.task_tool_allowlist`: string[] patterns (task-level; injected by host; read-only to guest)
- `mcp.task_tool_denylist`: string[] patterns (task-level; injected by host; read-only to guest)
- `mcp.tool_allowlist`: string[] patterns
- `mcp.tool_denylist`: string[] patterns
- `tool_choice`: `none | auto | {"type":"function",...}` (passed upstream)

#### Injection algorithm

Input: `session_params + registry_records + cached_tools`.

1. If `mcp.enabled != true` or `mcp.server_ids` is empty: inject nothing.
2. For each server_id:
   - load registry record
   - list_tools (via cache)
   - map tool name to `mcp.<server_id>.<tool_name>`
   - filter by registry allowlist + session allow/deny
3. Append MCP tools to the `tools` array (merge with WASM tools).

#### Routing and execution

Extend the existing auto tool-call loop with a unified dispatcher:

- if tool name matches a WASM tool: call by `fn_offset`
- if tool name matches `mcp.<server_id>.<tool_name>`:
  - parse `server_id/tool_name`
  - parse `arguments` into a JSON object (return structured error on failure)
  - call MCP `tools/call`

Return tool output as a JSON string (success or error) and append as `role=tool`.

### MCP hostcalls: fd model and ABI

#### fd kind

Introduce `FdKind::McpSession` (or a tagged generic) with internal state:

- `server_id`
- optional: pool handle reference
- optional: session-level policy overrides

#### ABI

Use the same `(ptr,len)` input and `(out_ptr,out_len_ptr)` output pattern as cchat:

- `mcp_open(server_id_ptr, server_id_len) -> mcp_fd`
- `mcp_list_tools(mcp_fd, out_ptr, out_len_ptr) -> rc`
- `mcp_call_tool(mcp_fd, tool_name_ptr, tool_name_len, args_ptr, args_len, out_ptr, out_len_ptr) -> rc`
- `mcp_close(mcp_fd) -> rc`

Use `-errno` for errors.

### Error model

For tool failures, prefer returning structured tool output and letting the model decide the next step, rather than failing the entire Chat Completion:

```json
{"error": {"code": "mcp_unavailable", "message": "...", "retryable": true}}
```

Suggested error categories:

- `mcp_unavailable`
- `mcp_timeout`
- `mcp_invalid_arguments`
- `mcp_policy_denied`
- `mcp_output_too_large`

### Concurrency, budgets, and resource control

Implement budgets at three levels:

- session: `max_iterations`, `max_total_tool_calls`, `max_tool_output_bytes`
- per server: `max_concurrency`, `tool_timeout_ms`
- global: total concurrency cap, subprocess count cap per Spearlet

For stdio subprocesses:

- limit maximum subprocesses
- idle timeout reaping

### Observability and audit

Recommended metrics in Spearlet:

- `mcp_server_connect_total{server_id}`
- `mcp_tool_call_total{server_id,tool}`
- `mcp_tool_call_error_total{server_id,tool,code}`
- `mcp_tool_call_latency_ms{server_id,tool}`
- `mcp_tool_call_output_bytes{server_id,tool}`

Audit logs should include `request_id/session_id/server_id/tool_name/status` with argument redaction.

### Minimal test plan

- Unit tests
  - route parsing for `mcp.<server_id>.<tool_name>`
  - allow/deny pattern matching
  - injection filtering
- Integration tests (tokio)
  - stdio: spawn a fake MCP server subprocess for `tools/list` and `tools/call`
  - HTTP: start a local axum mock for Streamable HTTP
  - cchat auto tool-call loop: inject MCP tools and verify `role=tool` append behavior
- Regression
  - behavior unchanged when MCP is disabled
  - compatibility for `tool_choice` (`none/auto/force`)
