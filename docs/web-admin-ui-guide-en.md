# Web Admin UI Guide

This document explains how to use the `spear-next` Web Admin, covering nodes, files, and task creation.

## Access & Auth

- Enable: run SMS with `--enable-web-admin --web-admin-addr 127.0.0.1:8081`
- URL: `http://127.0.0.1:8081/`
- Admin Token:
  - Enter in Nodes toolbar or Settings page and click `Apply`
  - Token is stored in `window.__ADMIN_TOKEN` and `localStorage('ADMIN_TOKEN')`

## Top Settings

- Theme: dark/light using Ant Design theme algorithms
- Timezone: all timestamps render using the selected timezone

## Nodes

- List supports search, time sorting, pagination, and details modal
- SSE: backend `GET /admin/api/nodes/stream` for live updates
- Toolbar includes search, sort, Admin Token input, and `Apply Token` button

## Files

- Choose File: click the `Choose File` button (hidden native `<input type="file">` triggered)
- Upload: click `Upload`; success toast shows `Uploaded: <id>`
- Actions:
  - `Download` the object
  - `Copy URI` copies `sms+file://<id>`
  - `Delete` removes and refreshes list (React Query invalidate + local filter)

## Task Creation (Tasks → Create Task)

- Executable type: `No Executable | Binary | Script | Container | WASM | Process`
- Scheme options: `sms+file | s3 | minio | https`
  - Selecting `sms+file` pre-fills `sms+file://`
  - Switching from non-`sms+file` back will not reset to placeholder
- Choose Local SMS File:
  - Click `Choose Local` to open the picker
  - Click `Use` to set `Executable URI = sms+file://<id>` and `Executable Name`
- Parameters: `Capabilities` (comma), `Args` (comma), `Env` (`key=value` per line)

### Execution Kind

- Option: `Short Running | Long Running`
- Mapping: sent via `metadata.execution_kind` in `POST /admin/api/tasks`
- Policy:
  - `Short Running` supports ExistingTask invocation
  - `Long Running` is created via SMS events; invoking as ExistingTask is rejected by policy
- Table: Tasks list shows `Exec Kind` column reflecting the server response

## Known Issues & Fixes

- Always-open dropdown: removed forced `open`; default interaction restored
- Scheme reset: only updates from URI when `://` is present; `sms+file` pre-fills
- Picker width: fixed modal width and column widths with ellipsis

## Related Docs

- `docs/web-admin-overview-en.md`
- `docs/ui-tests-guide-en.md`
