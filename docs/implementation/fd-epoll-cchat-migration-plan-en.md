# Implementation Plan: Generic fd/epoll subsystem + cchat migration (breaking changes allowed)

This document is the single “implementation entry point” for the next coding session. It captures the required context, decisions, phased steps, touched files, and acceptance tests so the work can start immediately.

## 0. Related design docs (read first)

- Generic subsystem design (structures/concurrency/tests/migration):
  - [fd-epoll-subsystem-en.md](../api/spear-hostcall/fd-epoll-subsystem-en.md)
  - [fd-epoll-subsystem-zh.md](../api/spear-hostcall/fd-epoll-subsystem-zh.md)
- Realtime ASR specialization (built on top of generic subsystem):
  - [realtime-asr-epoll-en.md](../api/spear-hostcall/realtime-asr-epoll-en.md)
  - [realtime-asr-epoll-zh.md](../api/spear-hostcall/realtime-asr-epoll-zh.md)
- cchat hostcall spec (explicitly allows breaking changes):
  - [chat-completion-en.md](../api/spear-hostcall/chat-completion-en.md)
  - [chat-completion-zh.md](../api/spear-hostcall/chat-completion-zh.md)

## 1. Current repository facts (context snapshot)

### 1.1 Existing cchat hostcalls (Rust)

- Hostcall glue: [wasm_hostcalls.rs](../../src/spearlet/execution/runtime/wasm_hostcalls.rs)
- cchat state machine currently uses a private map (`ChatHostState.sessions/responses`) and is not in the unified fd table:
  - [host_api.rs](../../src/spearlet/execution/host_api.rs)
- Existing error codes are fixed `-1..-5` and C SDK hard-codes those constants.

### 1.2 Tests and samples

- WAT link tests importing cchat: [wasm.rs](../../src/spearlet/execution/runtime/wasm.rs)
- C SDK header: [spear.h](../../sdk/c/include/spear.h)
- wasm-c chat sample: [chat_completion.c](../../samples/wasm-c/chat_completion.c)

### 1.3 Allowed scope of breaking changes

This work may introduce breaking changes for `cchat_*` (signatures/error codes/semantics), but must update in lock-step:

- Docs (chat-completion-zh/en)
- Tests (WAT link + Rust unit tests + E2E if any)
- SDK & samples (C header + wasm-c sample)

## 2. One-time decisions (must be settled before implementation)

### 2.1 Error code system

- Prefer using `-errno` everywhere (recommended): `-EAGAIN/-ENOSPC/-EBADF/...`
- Legacy fixed `-1..-5` codes:
  - either remove them (breaking), or
  - keep compatibility macros in the C header (not recommended)

### 2.2 cchat_send semantics

Two implementation options:

- Option A (stabilize first): `send` produces response bytes synchronously; response fd is immediately `EPOLLIN`.
- Option B (stronger later): `send` returns a response fd immediately and the response is produced asynchronously; readiness becomes `EPOLLIN` when ready.

Start with A to get the generic base working, then upgrade to B later.

### 2.3 epoll mode

- v1 is level-triggered.
- `ep_wait` returns packed records (8 bytes each: fd i32 + events i32).
- Return set guarantees: dedupe by fd, OR-merge events, sort by fd ascending.

## 3. Suggested module layout

Recommended new directory (can start minimal and split later):

- `src/spearlet/execution/hostcall/`
  - `mod.rs`
  - `fd_table.rs`
  - `epoll.rs`
  - `ctl.rs`
  - `errno.rs`
  - `memory.rs` (optional: shared mem_read/mem_write)

Phase 1 can live inside `host_api.rs` first and be extracted later.

## 4. Phased implementation steps

### Phase 1: Implement FdTable and EpollState (no external ABI changes)

Goal: host has a unified fd table + epoll state that hostcall glue can use.

Key tasks:

- Define `FdKind/FdFlags/PollEvents` (at least O_NONBLOCK and IN/OUT/ERR/HUP)
- Implement `FdTable`: alloc/get/close + watcher management
- Implement `EpollState`: watch set + notify + ep_wait scan algorithm

Acceptance:

- Rust unit tests: create dummy fds, verify ep_wait dedupe/sort/timeout and ENOSPC handling.

### Phase 2: Add hostcall ABI (spear_epoll_* + spear_fd_ctl)

Goal: WASM guest can import and call epoll and generic fd control.

Files:

- [wasm_hostcalls.rs](../../src/spearlet/execution/runtime/wasm_hostcalls.rs)
  - export `spear_epoll_create/ctl/wait/close`
  - export `spear_fd_ctl`
  - register new symbols in `build_spear_import*()`
- [wasm.rs](../../src/spearlet/execution/runtime/wasm.rs)
  - add WAT link tests importing `spear_epoll_*` and `spear_fd_ctl`

Acceptance:

- `cargo test` passes
- WAT link tests pass (with wasmedge feature)

### Phase 3: Migrate cchat into FdTable (breaking changes allowed)

Goal: move cchat sessions/responses into the unified fd table, and make response fd epoll-visible.

Key tasks:

- Introduce `FdKind::ChatSession` and `FdKind::ChatResponse`
- `cchat_create`: alloc ChatSession fd
- `cchat_send`: alloc ChatResponse fd
  - Option A: produce bytes synchronously
  - after bytes are ready: set poll_mask IN and notify watchers
- `cchat_recv`: read bytes from ChatResponse fd
- `cchat_close`: unified close sets HUP and notifies watchers

If using `-errno`:

- map invalid args/fd/buffer to `-EINVAL/-EBADF/-ENOSPC`

Acceptance:

- Rust unit tests:
  - after `cchat_send`, response fd is returned by epoll as `EPOLLIN`
  - after close, epoll returns `EPOLLHUP`

### Phase 4: Update C SDK and wasm-c sample

Goal: keep ABI consistent for C header and samples.

Files:

- [spear.h](../../sdk/c/include/spear.h)
  - update error code checks to `-errno` (at least ENOSPC/EAGAIN/EBADF)
  - optionally add `spear_epoll_*` and `spear_fd_ctl` declarations
  - update helpers like `sp_cchat_recv_alloc` to use `-ENOSPC` instead of `-3`
- [chat_completion.c](../../samples/wasm-c/chat_completion.c)

Acceptance:

- sample compiles and matches header semantics

### Phase 5: Extend test matrix (WAT + Rust + E2E)

Goal: protect breaking changes against regressions.

- update WAT import tests for cchat if symbols/signatures change
- update existing cchat pipeline tests
- update E2E if it depends on specific behavior

Acceptance:

- `cargo test` is green

### Phase 6: RtAsrFd prototype (optional / can be parallel)

Goal: put rtasr fd into fd table and support readiness. Start with a stub event source, then wire WebSocket.

## 5. Implementation notes (avoid footguns)

### 5.1 Lock ordering

Follow a consistent order:

1. lookup fd entry handle from FdTable (do not hold long)
2. epoll watch set lock
3. entry internal lock (queue/state)

Do not reverse the order.

### 5.2 Readiness triggers must notify

- recv_queue empty → non-empty: notify IN
- send_queue full → non-full: notify OUT
- error: notify ERR
- close/peer closed: notify HUP

### 5.3 ep_wait stability requirements

- one fd appears at most once in a single return
- OR-merge events for the same fd
- sort by fd ascending for stable tests
- allow spurious wakeups

### 5.4 Buffer-too-small (ENOSPC)

All hostcalls writing to an out buffer (ep_wait, recv/status/metrics) must support:

- when capacity is insufficient, write back required length
- return `-ENOSPC`

## 6. Suggested commands

After each phase:

- `cargo test`
- `cargo fmt --check`
- `cargo clippy -- -D warnings`

Version: implementation plan v1 (2026-01)

