# Web Admin Overview

This document summarizes the new Web Admin in `spear-next`.

## What It Provides

- Independent port (default `127.0.0.1:8081`) with Axum router
- Nodes list with search, sort, pagination
- Stats cards (total, online, offline, recent 60s)
- SSE stream `GET /admin/api/nodes/stream`
  - For testing: `?once=true` returns a single snapshot event
- Theme toggle (Dark/Light) and timezone selection for human-friendly time
- Optional auth via `SMS_WEB_ADMIN_TOKEN` (Bearer token)

## Configuration

- Enable: `--enable-web-admin`
- Address: `--web-admin-addr 0.0.0.0:8081`
- ENV: `SMS_ENABLE_WEB_ADMIN`, `SMS_WEB_ADMIN_ADDR`

## Implementation Notes

- UI is delivered via embedded static files (`index.html`, `react-app.js`, `style.css`)
- Uses Ant Design 5 tokens with `ConfigProvider` algorithms to correctly switch themes
- TopBar shows timezone and a Profile placeholder
- SSE cancellation uses a `CancellationToken` to allow graceful shutdown

## Endpoints

- `GET /admin/api/nodes` → JSON list with `uuid`, `name`, `ip_address`, `port`, `status`, `last_heartbeat`, `registered_at`
- `GET /admin/api/nodes/:uuid` → Node + optional resource info
- `GET /admin/api/stats` → counts (total/online/offline/recent_60s)
- `GET /admin/api/nodes/stream[?once=true]` → SSE snapshot events

### Tasks Endpoints

- `GET /admin/api/tasks` → returns task list with fields:
  - `task_id`, `name`, `description`, `status`, `priority`, `node_uuid`, `endpoint`, `version`
  - `execution_kind` (`short_running | long_running`), `executable_type`, `executable_uri`, `executable_name`
  - `registered_at`, `last_heartbeat`, `metadata`, `config`
  - `result_uris`, `last_result_uri`, `last_result_status`, `last_completed_at`, `last_result_metadata`
- `GET /admin/api/tasks/{task_id}` → returns detail with the same fields
- `POST /admin/api/tasks` → create task
  - Body includes `name`, `description`, `priority`, `node_uuid`, `endpoint`, `version`, `capabilities`, `metadata`, `config`, optional `executable`
  - `metadata.execution_kind` determines `execution_kind` enum mapping on the server

## Testing

- Integration test for SSE uses `?once=true` to avoid blocking
- Frontend tests rely on manual verification or E2E in future; backend tests cover list/stats/SSE
