# Invocation Log Persistence & Viewing Design

## Background

Today, a user-triggered run in Web Admin (`POST /admin/api/executions`) returns `invocation_id`/`execution_id`, but there is no first-class, user-facing way to persist and view per-invocation logs (stdout/stderr and runtime/system events). This makes troubleshooting and auditing hard.

This document proposes an industry-standard design to persist invocation logs and expose them through the SMS Web Admin BFF + Web Admin UI.

Related docs:

- Execution model: [invocation-execution-model-refactor-en.md](./invocation-execution-model-refactor-en.md)
- Web Admin overview: [web-admin-overview-en.md](./web-admin-overview-en.md)

## Goals

- Persist logs for each invocation (across retries/spillback executions).
- Let users quickly view logs from Web Admin: list → detail → tail → download.
- Support filtering (stream, level) and pagination/seek with stable cursors.
- Enforce safe defaults: retention, size limits, backpressure, redaction.
- Keep Spearlet service logs (control-plane) separate from user invocation logs.

## Non-goals

- Replacing enterprise log platforms (Loki/Elastic/Splunk). We keep an integration path.
- Providing arbitrary full-text search over years of logs in-app (can be phased in).
- Implementing an interactive console (covered separately).

## Definitions

- **Invocation**: a user/client request to run a function; stable `invocation_id`.
- **Execution**: a concrete attempt of an invocation on a node; `execution_id`.
- **Invocation log**: user-facing log stream attached to a specific execution, including:
  - process stdout/stderr
  - runtime/system events (optional)
  - structured app logs emitted via runtime APIs (optional)

## Requirements

### Functional

- Persist logs per **execution**; aggregate at **invocation** level.
- Provide **tail** (near real-time) and **history** (pagination) reads.
- Provide **download** of a whole execution log as a file.
- Provide consistent correlation keys in every log line:
  - `invocation_id`, `execution_id`, `task_id`, `function_name`, `node_uuid`, `instance_id`.

### Non-functional

- **Retention**: default TTL, configurable (e.g., 7–30 days).
- **Cost & size control**: per-execution max bytes, line truncation, chunking + compression.
- **Backpressure**: do not OOM Spearlet on noisy workloads; allow dropping policy.
- **Security**: authz, tenant isolation (future), secret redaction, safe downloads.
- **Reliability**: tolerate SMS outage; best-effort persistence with clear UX state.

## High-level Architecture

### Components

1. **Spearlet Log Collector (per execution)**
   - Captures stdout/stderr and optional runtime events.
   - Buffers in memory with bounded queues.
   - Flushes to central storage in chunks.

2. **SMS Log Storage (central)**
   - **Blob store** for log chunks (append-only): object storage in production; the existing `sms+file://` store can be a dev/default backend.
   - **Metadata store** for indices/cursors/retention: KV store (RocksDB/Sled) already exists in the project.

3. **SMS Web Admin BFF**
   - Lists invocations/executions and serves log history.
   - Provides SSE tail streaming for “Follow” mode.

4. **Web Admin UI**
   - Executions list and log viewer.
   - “View logs” deep link from task execution actions.

### Data flow

1. User triggers execution in Web Admin → SMS BFF spills back to a Spearlet node.
2. Spearlet starts execution and emits log events into the log collector.
3. Collector flushes chunk files + metadata updates to SMS Log Storage.
4. UI reads history via SMS BFF; tails via SSE.

## Storage Design

### Log event schema (logical)

Each line is a structured record; store as NDJSON.

```json
{
  "ts_ms": 1730000000000,
  "seq": 42,
  "invocation_id": "...",
  "execution_id": "...",
  "task_id": "...",
  "function_name": "__default__",
  "node_uuid": "...",
  "instance_id": "...",
  "stream": "stdout",
  "level": "info",
  "message": "hello",
  "attrs": {"k": "v"}
}
```

Notes:

- `seq` is a monotonic sequence per execution, used for stable pagination.
- `stream` is one of `stdout|stderr|system`.
- `attrs` is optional and can carry runtime-specific fields.

### Chunk format

- Chunk file: `invocations/{invocation_id}/executions/{execution_id}/chunks/{chunk_seq}.ndjson.zst`
- Chunk size: target 1–4 MiB compressed (configurable).
- Compression: Zstd (or gzip as a fallback).

### Metadata (KV)

Key patterns (logical):

- `exec:{execution_id}:meta` → { task_id, invocation_id, node_uuid, instance_id, started_at, completed_at, status, byte_count, first_seq, last_seq }
- `exec:{execution_id}:chunks` → list of { chunk_seq, first_seq, last_seq, uri, byte_count, created_at }
- `inv:{invocation_id}:executions` → ordered execution_ids (attempts)

This keeps listing fast without scanning blob storage.

### Retention & cleanup

- TTL policy runs in SMS:
  - delete chunk blobs and KV metadata when `completed_at + ttl < now`.
  - enforce per-execution `max_bytes` by truncating and marking `truncated=true`.

## API Design (SMS Web Admin BFF)

This design keeps UI-facing APIs in SMS to avoid the browser calling Spearlet directly.

### 1) Execution create response

Update `POST /admin/api/executions` response to include `invocation_id` and a log link:

```json
{
  "success": true,
  "invocation_id": "...",
  "execution_id": "...",
  "node_uuid": "...",
  "message": "ok",
  "log_url": "/admin/executions/{execution_id}"
}
```

### 2) List executions

`GET /admin/api/executions?task_id=&invocation_id=&limit=&cursor=`

Returns summaries with `execution_id`, `invocation_id`, `status`, timestamps, and `node_uuid`.

### 3) Get execution

`GET /admin/api/executions/{execution_id}`

Returns the execution meta and a `log_state`:

- `ready`: history available
- `pending`: execution running but not yet flushed
- `unavailable`: storage failure

### 4) Read logs (history)

`GET /admin/api/executions/{execution_id}/logs?limit=500&cursor=...&stream=stdout,stderr&level=info,warn,error`

Response:

```json
{
  "execution_id": "...",
  "lines": [ {"ts_ms":..., "seq":..., "stream":"stdout", "message":"..."} ],
  "next_cursor": "...",
  "truncated": false
}
```

Cursor recommendation:

- Use an opaque cursor encoded from `(seq, chunk_seq, offset)`.
- Support `direction=backward|forward` for “scroll up” behavior.

### 5) Tail logs (SSE)

`GET /admin/api/executions/{execution_id}/logs/stream`

SSE event payload:

- `event: log`
- `data: { line... }`

And an `event: eof` when the execution is completed and all flushes are done.

### 6) Download

`GET /admin/api/executions/{execution_id}/logs/download`

- Content-Type: `text/plain` or `application/x-ndjson`
- Optional `?format=text|ndjson`

## Spearlet Design

### Capture sources

- **Process runtime**: pipe child stdout/stderr, attach to execution.
- **WASM runtime**: support a hostcall “log” API that maps to invocation logs.
- **System events**: optionally log lifecycle milestones (scheduled/started/completed, resource usage).

### Backpressure strategy

- Per-execution bounded ring buffer (e.g., by bytes and/or line count).
- Flush trigger:
  - size threshold reached
  - time threshold (e.g., every 1s)
  - execution completion

Drop policy (configurable):

- `drop_oldest` (default): preserve latest context.
- `drop_newest`: preserve earliest context.

### Failure handling

- If SMS log storage is unreachable:
  - keep a small local buffer;
  - mark execution `log_state=unavailable` in metadata;
  - UI shows “logs unavailable, retry later”.

## Web Admin UI Changes (frontend)

Target codebase: `web-admin/` (React + TanStack Query). The UI already calls `POST /admin/api/executions` via [executions.ts](../web-admin/src/api/executions.ts).

### New pages & navigation

- Add “Executions” page:
  - route: `/executions`
  - list table with filters (task_id, status, time range, node)
  - click row → `/executions/:execution_id`

### Execution detail page

- Header: status, task_id, node_uuid, invocation_id, timestamps.
- Log viewer:
  - “Follow” toggle (auto-scroll)
  - stream filters (stdout/stderr/system)
  - level filters (if supported)
  - search-in-view (client-side)
  - download button
  - copy selected lines

### API client additions

- Add `src/api/logs.ts`:
  - `getExecution(execution_id)`
  - `getExecutionLogs(execution_id, cursor, limit, filters)`
  - `streamExecutionLogs(execution_id)` (EventSource)

### UX details (best practices)

- Virtualized rendering for large logs.
- Show “truncated” banner if max bytes reached.
- Clear separation of stdout vs stderr (color/label).
- Persist “Follow” and filters in query params for shareable links.

## Security Considerations

- Never expose secret values in UI.
  - Redact common patterns (API keys, Bearer tokens) in SMS before storage.
  - Do not store environment variables by default.
- Auth:
  - Reuse `SMS_WEB_ADMIN_TOKEN` bearer mechanism for log APIs.
  - Future: add per-tenant RBAC and audit.
- Downloads:
  - Content-Disposition attachment; limit size; protect against path traversal by using IDs only.

## Observability

- Every log line includes correlation IDs.
- Emit metrics:
  - bytes ingested, dropped lines, flush latency, storage failures.
- Add tracing spans around flush operations.

## Rollout Plan

1. **Phase 0 (MVP)**
   - Persist stdout/stderr logs per execution.
   - Implement history read + download.
   - UI: execution detail page with log viewer.

2. **Phase 1**
   - SSE tail streaming.
   - Execution list page.

3. **Phase 2**
   - Structured log levels and richer runtime events.
   - Optional full-text search (behind config).

## Alternatives

- **External log platform integration**: ship logs via OTEL/Fluent Bit and query in Grafana/Kibana.
  - Pros: scalable search, long retention.
  - Cons: not self-contained, requires infra.

This proposal keeps an in-product MVP while leaving a clean export path.
