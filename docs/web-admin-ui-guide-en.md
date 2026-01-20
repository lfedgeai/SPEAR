# Web Admin UI Guide

This document explains how to use the `spear-next` Web Admin, covering nodes, files, and task creation.

## Access & Auth

- Enable: run SMS with `--enable-web-admin --web-admin-addr 127.0.0.1:8081`
- URL: `http://127.0.0.1:8081/`
- Admin Token:
  - Enter in Settings and click `Save`
  - Token is stored in `localStorage('ADMIN_TOKEN')`

## Top Settings

- Theme: light/dark

## Nodes

- List supports search, time sorting, pagination, and details modal
- SSE: backend `GET /admin/api/nodes/stream` for live updates
- Toolbar includes search and refresh

## Files

- Choose files: click `Choose files` (native `<input type="file" multiple>`)
- Upload: click `Upload`; success toast shows completion
- Actions:
  - `Download` the object
  - `Copy URI` copies `sms+file://<id>`
  - `Delete` removes and refreshes list (React Query invalidate + local filter)

## Task Creation (Tasks → Create Task)

- Executable type: `No Executable | Binary | Script | Container | WASM | Process`
- Executable URI accepts `sms+file://<id>` for embedded file artifacts
- Parameters: `Capabilities` (comma), `Args` (comma), `Env` (`key=value` per line)
 - MCP tools (optional):
   - Enable MCP tools and pick per-task servers from the MCP registry
   - “Default” servers map to `Task.config["mcp.default_server_ids"]`
   - “Allowed” servers map to `Task.config["mcp.allowed_server_ids"]` (upper bound)
   - Tool filters map to `Task.config["mcp.tool_allowlist"]` / `["mcp.tool_denylist"]`

### Execution Kind

- Option: `Short Running | Long Running`
- Mapping: sent via `metadata.execution_kind` in `POST /admin/api/tasks`
- Policy:
  - `Short Running` supports ExistingTask invocation
  - `Long Running` is created via SMS events; invoking as ExistingTask is rejected by policy
- Table: Tasks list shows `Exec Kind` column reflecting the server response

## Backends

- List supports search and availability filtering
- Click a backend row to open a detail dialog
  - Summary: shows aggregated availability and capability counts
  - Raw JSON: shows the full aggregated backend JSON

## Known Issues & Fixes

- None tracked in this doc; use issues in repo.

## Related Docs

- `docs/web-admin-overview-en.md`
- `docs/ui-tests-guide-en.md`
- `docs/ollama-discovery-en.md`
