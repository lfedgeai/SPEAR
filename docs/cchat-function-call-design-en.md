# CChat Function Call (Tool Calling) Design

## Background

Spear exposes a WASM hostcall-based Chat Completion session API. On the WASM side, `cchat_write_fn(fd, fn_offset, fn_json)` registers a WASM function as a tool for the chat session.

The current gap is that the host does not automatically execute tool/function calls returned by the model. As a result, the common industry loop is missing:

- Model requests tool calls
- Client executes tools
- Client appends tool results back into the conversation
- Client calls Chat Completion again until no more tool calls

Related legacy notes:

- The hostcall documentation already plans an “auto function call” flag (bit 1) for `cchat_send`. See [chat-completion-en.md](./api/spear-hostcall/chat-completion-en.md).
- `cchat_write_fn` persists `fn_offset` and `fn_json` (tool schema) in the session. Entry point: [wasm_hostcalls.rs](../src/spearlet/execution/runtime/wasm_hostcalls.rs).

This design keeps compatibility with existing hostcalls and aligns with OpenAI-compatible best practices.

## Goals

- During `cchat_send` (Chat Completion), include the tools registered via `cchat_write_fn` in the upstream request.
- When the model returns tool/function calls:
  - Resolve the tool by name to the corresponding `fn_offset`.
  - Invoke the WASM function via table + funcref.
  - Append tool results as `role=tool` messages.
  - Loop until the model stops requesting tool calls.
- Add observability (logs/metrics), safety limits, and a feature-gated rollout.

## Non-goals

- Streaming tool calling (can be added later).
- Cross-session sandboxing/permissions (but we will enforce guardrails and limits).

## Terminology

- **Tool / Function**: an external capability the model can call; implemented as a WASM function.
- **fn_offset**: a WASM function pointer/offset. For wasm32, a function pointer is effectively a table index.
- **tool schema**: JSON schema sent to the model (name/description/parameters).

## Legacy / Current State

### Tool registration

WASM registers a tool with `cchat_write_fn(fd, fn_offset, fn_json)`.

- `fn_offset`: table index (requires exporting a function table or having the default `__indirect_function_table`).
- `fn_json`: the tool schema JSON. To align with OpenAI, recommend:

```json
{
  "type": "function",
  "function": {
    "name": "tool_call",
    "description": "...",
    "parameters": {"type":"object", "properties": {}}
  }
}
```

### Chat completion invocation

`cchat_send` assembles `messages` and `tools` and calls the AI backend (e.g., OpenAI chat/completions). The backend adapter returns the raw JSON payload in a canonical envelope. See [openai_chat_completion.rs](../src/spearlet/execution/ai/backends/openai_chat_completion.rs).

### Missing loop

Even if the model returns `tool_calls` / `function_call`, the host currently does not parse and execute tools automatically.

## Industry Best Practices (OpenAI-compatible)

1. Requests include `tools` (function schemas), optionally `tool_choice`.
2. Responses return an assistant message with `tool_calls`:

```json
{
  "role": "assistant",
  "tool_calls": [
    {
      "id": "call_abc",
      "type": "function",
      "function": {"name": "tool_call", "arguments": "{...json...}"}
    }
  ]
}
```

3. The client executes tools and appends tool results using `role=tool` and `tool_call_id`:

```json
{"role":"tool","tool_call_id":"call_abc","content":"{...result...}"}
```

4. Repeat until no new tool calls.

This design follows these message shapes to keep model behavior predictable.

## High-Level Design

### Layering

- **WASM hostcall layer (orchestration)**: implements the tool-calling loop (has access to both `host_data` and `instance`).
- **HostApi / session storage (state)**: manages fd lifecycle and persists messages/tools.
- **AI backend adapters (upstream)**: translate canonical request to provider request.

### Data model changes

To correctly append tool call messages, we need to extend the internal Chat Completion IR.

1. **Extend ChatMessage**

- Current: `role` + `content` only.
- Add optional fields:
  - `tool_call_id: Option<String>` (for `role=tool`)
  - `tool_calls: Option<Vec<ToolCall>>` (for `role=assistant`)
  - `name: Option<String>` (provider compatibility)

2. **ToolCall**

- `id: String`
- `name: String`
- `arguments_json: String` (preserve the raw `arguments` string)

3. **Per-session ToolRegistry**

- `Vec<ToolEntry>` or `HashMap<String, ToolEntry>`:
  - `name: String`
  - `fn_offset: i32`
  - `schema_json: String`

`name` is parsed from `fn_json` (prefer `function.name`). Tools with parse failures are excluded from auto-calling.

### WASM Tool ABI

Recommended function signature:

```text
tool(args_ptr: i32, args_len: i32, out_ptr: i32, out_len_ptr: i32) -> i32
```

- `args_ptr/args_len`: UTF-8 JSON arguments (same as upstream `tool_calls[].function.arguments`).
- `out_ptr/out_len_ptr`: output buffer and its length pointer.
- Return code:
  - `0`: success; tool writes output and stores the actual length into `*out_len_ptr`.
  - `-ENOSPC`: buffer too small; tool stores required length into `*out_len_ptr`, host retries with a bigger buffer.
  - other negative values: errors.

This matches the existing “ENOSPC with required length” pattern used by `cchat_recv`.

### Resolving `fn_offset` into a callable function

Host resolution strategy:

1. Pick an exported table: prefer `__indirect_function_table`, then `table`, else the first exported table.
2. `table.get_data(fn_offset)` to obtain a funcref.
3. Convert funcref into a `Function`, validate signature `(i32,i32,i32,i32)->i32`.
4. Call via `Executor::call_func`.

Production code must properly allocate/manage guest memory for args/output.

## Core Flow (Auto Tool Calling Loop)

### Trigger

- `cchat_send(fd, flags)` includes `AUTO_TOOL_CALL` (recommended to reuse the planned bit 1).
- When `AUTO_TOOL_CALL` is enabled, a “max tool call count” limit must be enforced to cap the total number of tool invocations triggered by this `cchat_send` (accumulated across multiple completion rounds).

### Algorithm

Given a session `fd`:

1. Build a ChatCompletions request from session snapshot: `messages` + `tools` (ToolRegistry).
2. Call the AI backend.
3. Parse response:
   - If no tool calls: store final assistant content and exit.
   - If tool calls exist:
     1) Append the assistant message (with `tool_calls`) to session messages.
     2) For each tool call:
        - Resolve `name` to `fn_offset` in ToolRegistry.
        - Call the WASM tool and obtain a `tool_result` string.
        - Append `role=tool, tool_call_id=..., content=tool_result`.
     3) Go back to step 1.

### Termination and limits

To prevent infinite loops and resource exhaustion:

- `max_iterations`: default 8 (configurable)
- `max_tool_output_bytes`: default 64KiB (configurable)
- `max_total_tool_calls`: default 32 (configurable). Caps total tool invocations triggered by `AUTO_TOOL_CALL` (each WASM tool execution counts as 1).

### Multiple tool calls per assistant message

- Execute sequentially (simple, deterministic, aligns with most provider examples).
- Append `role=tool` results in the same order.

## Error Handling

### Unknown tool

- If the model requests an unregistered tool name:
  - Prefer appending a structured error as a `role=tool` message and letting the model recover.
  - Optionally fail fast in strict mode (configurable).

### Tool execution failure

- Non-zero rc:
  - Append a `role=tool` message describing the error (rc + short summary).
- Memory allocation failure / OOB:
  - Abort and return an execution error.

### Upstream format variance

- Parse OpenAI-compatible: prefer `choices[0].message.tool_calls`, also support legacy `function_call`.
- Parse failure: treat as no tool calls but emit debug logs.

## Observability

- Structured debug logs inside the loop:
  - iteration, tool_name, tool_call_id, rc, output size, upstream request_id
- Suggested metrics:
  - tool_call_iterations_total
  - tool_calls_total (by tool name)
  - tool_call_failures_total (by rc)
  - tool_output_bytes_total

## Configuration and rollout

- `AUTO_TOOL_CALL` flag: default off; gradual rollout.
- Global limits: max_iterations / max_total_tool_calls / max_tool_output_bytes / strict_unknown_tool.

## Compatibility and migration

- Behavior remains unchanged when `AUTO_TOOL_CALL` is not set.
- Legacy tool schema can be wrapped:
  - If `fn_json` lacks top-level `type/function` but includes `name/parameters`, wrap it into the OpenAI tools shape.
- IR changes require backend adapters to serialize messages with the new optional fields while keeping old fields compatible.

## Test Plan

- Unit tests:
  - Mock a response containing `tool_calls` and verify the loop executes tools and appends `role=tool` messages.
  - Cover unknown tool, rc!=0, and ENOSPC resize-and-retry.
- Integration tests:
  - Use a WASM sample exporting a table + tool function and run a small end-to-end loop.
- Regression tests:
  - Ensure non-AUTO_TOOL_CALL path is unchanged.

## Security notes

- Do not log full tool arguments or outputs by default (log sizes and minimal metadata only).
- Enforce output size and UTF-8 validation.
- Enforce iteration and total tool call limits.
