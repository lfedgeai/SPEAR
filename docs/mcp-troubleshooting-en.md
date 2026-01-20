# MCP Troubleshooting

This note explains why the model might respond like:

> "I can't access external files or systems, including the MCP filesystem tools you mentioned"

and how to verify MCP is actually available in Spear.

## What the response means

That text is a normal model message (finish_reason = "stop"), not an MCP execution error.
It almost always means the OpenAI request did not include any callable `tools`, so the model had no way to invoke MCP.

## How MCP tools are injected (current implementation)

MCP tools are injected and executed only in the **cchat host API** auto tool-call loop:

- Tool injection: `cchat_inject_mcp_tools` in [cchat.rs](../src/spearlet/execution/host_api/cchat.rs)
- Tool execution: `cchat_exec_mcp_tool` in [cchat.rs](../src/spearlet/execution/host_api/cchat.rs)
- MCP registry sync (from SMS): [registry_sync.rs](../src/spearlet/mcp/registry_sync.rs)

If you are using a different call path (not cchat auto tool-call), MCP tools may not be attached.

## Required session params

MCP tool injection is gated by session params:

- `mcp.enabled`: boolean, must be `true`
- `mcp.server_ids`: array of strings, must include the server id (e.g. `"fs"`)
- `mcp.task_tool_allowlist` / `mcp.task_tool_denylist`: task-level filters (if configured); injected by host from `Task.config`; read-only to the guest

See parsing logic: `session_policy_from_params` in [policy.rs](../src/spearlet/mcp/policy.rs)

If either is missing, no MCP tools are injected and the model will respond as if tools do not exist.

Note: if the task has MCP policy in `Task.config`, `cchat_create` may auto-apply defaults for `mcp.enabled` / `mcp.server_ids`, so you may not need to set them manually.

## Common root causes

1. **MCP not enabled in params** (`mcp.enabled` / `mcp.server_ids` missing)
2. **Spearlet has no MCP registry sync** (not connected to SMS, or SMS not providing MCP registry)
3. **Server not found** in registry snapshot (server id mismatch)
4. **Server policy denies tools** (allowed tool patterns empty / restrictive)
5. **list_tools timed out / failed**, resulting in an empty tool list
6. **Task policy denied**: the task has MCP disabled or you tried to set server_ids outside task allowed set

## What to check

- Does the response contain `choices[0].message.tool_calls`?
  - If not present, tools were likely not attached or the model chose not to call tools.
- Spearlet logs for registry sync:
  - `MCP registry watch start failed`
  - `MCP registry watch ended`
  - These indicate SMS connectivity / MCP registry issues.
- Server allowlist:
  - `allowed_tools` must include patterns that match tool names.
