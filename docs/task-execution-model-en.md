# Task Execution Model (Plan A)

This repository follows a “unified request execution (request/response) + optional streaming” model:

- Tasks no longer distinguish “long running / short running”.
- A Task represents an invokable executable unit (endpoint, executable descriptor, config, etc.).
- The interaction style of a single invocation is driven by `execution_mode` (e.g. sync/async/stream).

## Goals

- Reduce conceptual branching: Task status/type should not encode runtime strategy.
- Put policy into tunable parameters: timeout, concurrency, placement/scheduling, retries.
- Keep runtime optimizations internal: instance reuse, warmup, caching remain execution-layer details.

## Behavioral Notes

- “Whether instances exist” is not modeled as a Task type; it is reflected by runtime availability/status.
- “Run/Invoke” always creates a new execution request; an active task does not block further invocations (concurrency/resource limits apply).

