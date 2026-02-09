# Web Admin UI Guide

This document explains how to use the `spear-next` Web Admin, covering nodes, files, and task creation.

## Access & Auth

- Enable: run SMS with `--enable-web-admin --web-admin-addr 127.0.0.1:8081`
- URL: `http://127.0.0.1:8081/admin`
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
  - `Copy URI` copies `smsfile://<id>`
  - `Delete` removes and refreshes list (React Query invalidate + local filter)

## Task Creation (Tasks → Create Task)

- Executable type: `No Executable | Binary | Script | Container | WASM | Process`
- Executable URI accepts `smsfile://<id>` for embedded file artifacts
- Parameters: `Capabilities` (comma), `Args` (comma), `Env` (`key=value` per line)
 - MCP tools (optional):
   - Enable MCP tools and pick per-task servers from the MCP registry
   - “Default” servers map to `Task.config["mcp.default_server_ids"]`
   - “Allowed” servers map to `Task.config["mcp.allowed_server_ids"]` (upper bound)
   - Tool filters map to `Task.config["mcp.tool_allowlist"]` / `["mcp.tool_denylist"]`

## AI Models

- AI Models page is split into `Local` and `Remote`
- List supports search and availability filtering (available/unavailable)
- Click a row to navigate to the model detail page and inspect per-node instances

### Local: Create a deployment

- Entry: Local → AI Models → `Create`
- Form:
  - Node: which node to deploy to
  - Provider: defaults to `LLaMA CPP`
  - Model name: display name
  - Model URL: required for llamacpp, a direct `.gguf` URL (http/https)
- On success the UI focuses the created deployment (via `deployment_id=...`) and shows progress in the `Provisioning` panel

### Local: Delete a deployment (even if available)

- Entry: Local → AI Models table → `Actions` → `Delete`
- Behavior:
  - Deletes matching deployments for the selected `(provider, model)` across the associated node(s)
  - Spearlet will stop the local process and remove it from the backend registry on the next reconcile loop

## Known Issues & Fixes

- None tracked in this doc; use issues in repo.

## Related Docs

- `docs/web-admin-overview-en.md`
- `docs/ui-tests-guide-en.md`
- `docs/ollama-discovery-en.md`
