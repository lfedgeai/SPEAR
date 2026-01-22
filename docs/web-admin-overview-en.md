# Web Admin Overview

This document summarizes the new Web Admin in `spear-next`.

## What It Provides

- Independent port (default `127.0.0.1:8081`) with Axum router
- Nodes list with search, sort, pagination
- Backends list (aggregated view; includes a detail dialog with Raw JSON)
- Stats cards (total, online, offline, recent 60s)
- SSE stream `GET /admin/api/nodes/stream`
  - For testing: `?once=true` returns a single snapshot event
- Theme toggle (Dark/Light)
- Optional auth via `SMS_WEB_ADMIN_TOKEN` (Bearer token)

## Configuration

- Enable: `--enable-web-admin`
- Address: `--web-admin-addr 0.0.0.0:8081`
- ENV: `SMS_ENABLE_WEB_ADMIN`, `SMS_WEB_ADMIN_ADDR`

## Implementation Notes

- UI is delivered via embedded static files (`index.html`, `main.js`, `main.css`)
- UI source lives in `web-admin/` and build output overwrites `assets/admin/*`
- UI stack: Radix primitives + Tailwind (shadcn/ui style), enterprise console look
- SSE cancellation uses a `CancellationToken` to allow graceful shutdown

## Endpoints

- `GET /admin/api/nodes` → JSON list with `uuid`, `name`, `ip_address`, `port`, `status`, `last_heartbeat`, `registered_at`
- `GET /admin/api/nodes/:uuid` → Node + optional resource info
- `GET /admin/api/stats` → counts (total/online/offline/recent_60s)
- `GET /admin/api/nodes/stream[?once=true]` → SSE snapshot events
- `GET /admin/api/backends` → aggregated backend list (capabilities + per-node availability)

### Tasks Endpoints

- `GET /admin/api/tasks` → returns task list with fields:
  - `task_id`, `name`, `description`, `status`, `priority`, `node_uuid`, `endpoint`, `version`

#### Create vs Execute (Two Flows)

Web Admin treats “create task (register)” and “execute task (schedule + run)” as two steps:

- Step 1: create/register
  - `POST /admin/api/tasks`
  - `node_uuid` semantics:
    - pin to node: `node_uuid=<uuid>`
    - auto-schedule: `node_uuid=""` (empty string)
- Step 2: trigger execution (optional)
  - `POST /admin/api/executions`
  - uses SMS placement to pick candidates, then spillback-invokes Spearlet

Behavior differences:

- Pinned node (`node_uuid` non-empty):
  - indicates the task is pinned/owned by a specific node (not “last execution placement”)
  - execution is triggered via `POST /admin/api/executions`; `Run after create` will run on that node
- Auto-schedule (`node_uuid` empty):
  - indicates the task is not pinned to a node
  - execution is triggered via `POST /admin/api/executions`; `Run after create` will use SMS placement to pick a node

## Secret/Key Management Guidance

If you add an “API key configuration” component to Web Admin, design it as “secret reference management”, not plaintext key entry/storage.

- UI/control-plane manages: mapping between backend instances and `credential_ref` (or `credential_refs`)
- Secret values are provisioned by: the deployment system (Kubernetes Secrets / Vault Agent / systemd drop-in)
- Observability: show only “present/usable” (e.g., spearlet heartbeat reports `HAS_ENV:<ENV_NAME>=true`), never the value
  - `executable_type`, `executable_uri`, `executable_name`
  - `registered_at`, `last_heartbeat`, `metadata`, `config`
  - `result_uris`, `last_result_uri`, `last_result_status`, `last_completed_at`, `last_result_metadata`
- `GET /admin/api/tasks/{task_id}` → returns detail with the same fields
- `POST /admin/api/tasks` → create task
  - Body includes `name`, `description`, `priority`, `node_uuid`, `endpoint`, `version`, `capabilities`, `metadata`, `config`, optional `executable`

## Testing

- Integration test for SSE uses `?once=true` to avoid blocking
- Frontend includes Playwright UI tests (`make test-ui`)
