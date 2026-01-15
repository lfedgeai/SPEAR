# ExecutionResponse fields vs metadata

This document describes how Spear represents execution identity fields and why they should not live in `metadata`.

## Background

Historically, some “identity” values (e.g. `invocation_id`, `task_id`, `function_name`, `instance_id`) were stored inside `ExecutionResponse.metadata`.
This caused two issues:

- These keys are special and should have stable semantics.
- `metadata` is intended for runtime-specific / auxiliary data and can be filtered/overwritten.

## Current behavior

`ExecutionResponse` now exposes the following identity fields explicitly:

- `invocation_id`
- `task_id`
- `function_name`
- `instance_id`

`metadata` is reserved for runtime-provided metadata (stringified values from `RuntimeExecutionResponse.metadata`).

## Code references

- `ExecutionResponse` definition: [execution/mod.rs](../src/spearlet/execution/mod.rs)
- Execution record population: [manager.rs](../src/spearlet/execution/manager.rs)
- gRPC mapping to proto `Execution`: [function_service.rs](../src/spearlet/function_service.rs)

