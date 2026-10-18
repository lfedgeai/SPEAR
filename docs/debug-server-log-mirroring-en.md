# Debug Server Log Mirroring

## Overview

SPEAR now mirrors WASM guest logs to the debug server in addition to the existing execution log path.

This keeps the original execution log behavior unchanged while making guest-side troubleshooting easier from the debug server UI.

## Current Behavior

- `spear.log(...)` inside a WASM guest still writes to the local WASM log ring
- those logs are still flushed to SMS execution logs with stream `wasm`
- the same log entry is now also mirrored to the debug server
- Rust guests can opt in to SDK-provided `println!` / `eprintln!` macro shadowing via `use spear_wasm::{println, eprintln};`, which routes guest info/error prints through the same mirrored WASM log path without changing the standard-library macros globally
- Rust guests can use `spear_wasm::{infoln, debugln, warnln, errorln}` when they need explicit levels for mirrored diagnostics, while keeping `println!` / `eprintln!` available as the ergonomic default
- The Rust samples keep protocol warnings and RTASR/chat failures on this mirrored debug surface, while leaving the visible text-stream output for transcript and reply content only

## Debug Server Event Shape

Mirrored WASM logs are sent as debug events with:

- `streamName = wasm-log`
- `msg = <original log message>`
- `data.eventType = log`
- `data.source = wasm`
- `data.level = <trace|debug|info|warn|error>`
- `data.taskId`
- `data.executionId`
- `data.instanceId`

The debug server keeps using the configured session and client names, so log selection still works with the existing session / client / stream hierarchy.

## Why This Design

- It does not break or replace the existing execution log pipeline
- It keeps the source of truth for runtime execution logs in SMS
- It adds a second observability surface optimized for fast debugging
- It keeps instance and execution context attached to each mirrored log entry
