# Spear Hostcall Engineering Design: General fd/epoll Subsystem (v1)

## 0. Purpose

This document defines the general I/O substrate for Spear WASM hostcalls:

- a unified **fd (file descriptor) table** (kinds, flags, lifecycle, reclamation)
- a unified **epoll-style readiness layer** (multiplexing, wakeups, level-triggered contract)
- a reusable **fd control interface** (nonblock, status, kind, metrics)
- engineering-grade **Rust structures / concurrency model / migration plan / test checklist**

Realtime ASR is one concrete fd type (`RtAsrFd`) built on top of this subsystem. ASR-specific protocol/parameters live in [realtime-asr-epoll-en.md](file:///Users/bytedance/Documents/GitHub/bge/spear/docs/api/spear-hostcall/realtime-asr-epoll-en.md).

---

## 1. Terminology

- **guest**: WASM module
- **host**: SPEARlet/Runtime
- **fd**: guest-visible `i32` handle
- **epfd**: epoll instance fd
- **readiness**: whether an fd is currently readable/writable/error/hangup
- **level-triggered**: as long as the condition holds, `ep_wait` reports it

---

## 2. Engineering Goals

### 2.1 Must-haves

- Every pollable resource must be registered in a unified fd table; epoll must not “side-watch” resources.
- `ep_wait` must be wakeable (Notify/Condvar); busy-loop is not a core mechanism.
- Readiness semantics must be deterministic for guests: fixed dedup, fixed ordering, fixed level-triggered behavior.
- Close/cancel must wake `ep_wait`, and must be observable via `EPOLLHUP|EPOLLERR`.
- Incremental migration is supported; existing family APIs (e.g. `cchat_*`) need not break immediately.

#### 2.1.1 Breaking changes are allowed (for legacy APIs like `cchat_*`)

To achieve consistency under the general fd/epoll subsystem, breaking changes to existing `cchat_*` APIs are acceptable (e.g. error model, control interface, and even function signatures/names).

Constraint:

- Any breaking change must be accompanied by updates to:
  - unit/integration tests
  - SDKs (e.g. C header) and sample code
  - the corresponding API documents

The migration section below lists impacted files.

### 2.2 Explicit non-goals (v1)

- No edge-triggered / oneshot.
- No full WASI socket/filesystem.
- No requirement on a specific guest async runtime.

---

## 3. Guest-visible ABI (hostcalls)

### 3.1 `spear_epoll_*` (epoll)

#### 3.1.1 Signatures

- `spear_epoll_create() -> i32`
- `spear_epoll_ctl(epfd: i32, op: i32, fd: i32, events: i32) -> i32`
- `spear_epoll_wait(epfd: i32, out_ptr: i32, out_len_ptr: i32, timeout_ms: i32) -> i32`
- `spear_epoll_close(epfd: i32) -> i32`

#### 3.1.2 Constants

ctl ops:

- `EPOLL_CTL_ADD = 1`
- `EPOLL_CTL_MOD = 2`
- `EPOLL_CTL_DEL = 3`

event bits:

- `EPOLLIN  = 0x001`
- `EPOLLOUT = 0x004`
- `EPOLLERR = 0x008`
- `EPOLLHUP = 0x010`

#### 3.1.3 Output buffer layout (stable ABI)

To avoid alignment differences, `spear_epoll_wait` outputs a packed little-endian array.

Each record is 8 bytes:

- `fd: i32` (4 bytes)
- `events: i32` (4 bytes)

So `need = n * 8`.

Buffer-too-small:

- `max_records = floor(capacity_bytes / 8)`
- if `max_records == 0`:
  - write back `8` into `*out_len_ptr`
  - return `-ENOSPC`
- else write `min(ready_count, max_records)` and write back actual bytes.

#### 3.1.4 Wait semantics

- `timeout_ms < 0`: wait forever
- `timeout_ms == 0`: return immediately (poll)
- `timeout_ms > 0`: wait up to timeout

Return:

- `n >= 0`: number of records
- timeout: `0`
- error: `-errno`

Wakeup conditions:

- any watched fd may transition from “not ready” to “ready”
- fd/epfd close/cancel

### 3.2 `spear_fd_ctl` (generic fd control)

#### 3.2.1 Motivation

Families (`cchat_ctl/rtasr_ctl/...`) will repeatedly implement the same generic controls (nonblock, status, metrics). Provide a single entrypoint that family-specific `*_ctl` can reuse internally.

#### 3.2.2 Signature

- `spear_fd_ctl(fd: i32, cmd: i32, arg_ptr: i32, arg_len_ptr: i32) -> i32`

#### 3.2.3 Generic cmds (recommended for v1)

- `FD_CTL_SET_FLAGS = 1`
- `FD_CTL_GET_FLAGS = 2`
- `FD_CTL_GET_KIND  = 3`
- `FD_CTL_GET_STATUS = 4`
- `FD_CTL_GET_METRICS = 5`
- `FD_CTL_CLOSE = 6` (optional in v1)

Flags JSON example:

```json
{ "set": ["O_NONBLOCK"], "clear": [] }
```

Status JSON example:

```json
{
  "kind": "RtAsrFd",
  "flags": ["O_NONBLOCK"],
  "poll_mask": ["EPOLLIN"],
  "closed": false
}
```

### 3.3 errno conventions (subsystem uses `-errno`)

Use negative libc errno values:

- `-EBADF`, `-EINVAL`, `-EAGAIN`, `-ENOSPC`, `-ENOTCONN`, `-EPIPE`, `-ETIMEDOUT`, `-EINTR`, `-ENOMEM`

Compatibility: existing `cchat_*` may keep `-1..-5` externally, but once those fds participate in epoll/generic controls, internal mapping should respect `-errno` semantics.

---

## 4. Host-side Engineering Architecture (Rust)

### 4.1 Suggested layout

(can be phased; not required to land at once)

- `src/spearlet/execution/hostcall/`
  - `fd_table.rs`: allocator + fd table + close
  - `epoll.rs`: epfd + watch sets + wait/wakeup
  - `ctl.rs`: `spear_fd_ctl` implementation
  - `errno.rs`: errno helpers + buffer-too-small conventions
  - `memory.rs`: linear-memory helpers for glue
- `src/spearlet/execution/stream/`
  - `rtasr.rs`: `RtAsrFd` implementation

### 4.2 Data structures (field-level)

#### 4.2.1 `FdKind`

```rust
pub enum FdKind {
    ChatSession,
    ChatResponse,
    RtAsr,
    Epoll,
    Mic,
    ObjectRead,
    ObjectWrite,
    NetStream,
}
```

#### 4.2.2 `FdFlags`

```rust
bitflags::bitflags! {
    pub struct FdFlags: u32 {
        const O_NONBLOCK = 0x1;
        const FD_CLOEXEC = 0x2;
    }
}
```

#### 4.2.3 `PollEvents`

```rust
bitflags::bitflags! {
    pub struct PollEvents: u32 {
        const IN  = 0x001;
        const OUT = 0x004;
        const ERR = 0x008;
        const HUP = 0x010;
    }
}
```

#### 4.2.4 `FdEntry`

```rust
pub struct FdEntry {
    pub kind: FdKind,
    pub flags: FdFlags,
    pub inner: FdInner,
    pub watchers: std::collections::HashSet<i32>,
    pub closed: bool,
}

pub enum FdInner {
    ChatSession(ChatSessionState),
    ChatResponse(ChatResponseState),
    RtAsr(RtAsrState),
    Epoll(EpollState),
}
```

Constraints:

- `watchers` must be deduplicated.
- when `closed == true`:
  - `poll_mask` must include `HUP`
  - I/O returns a standardized error/EOF policy.

#### 4.2.5 `FdTable`

```rust
pub struct FdTable {
    next_fd: i32,
    entries: std::collections::HashMap<i32, std::sync::Arc<std::sync::Mutex<FdEntry>>>,
}
```

Suggested API:

- `alloc(kind, flags, inner) -> fd`
- `get(fd) -> Arc<Mutex<FdEntry>>`
- `close(fd) -> Result<(), errno>`
- `register_watcher(fd, epfd)` / `unregister_watcher(fd, epfd)`

#### 4.2.6 `Pollable` (semantic constraint)

Rust can implement via match dispatch or trait objects, but semantics must match:

```rust
pub trait Pollable {
    fn poll_mask(&self) -> PollEvents;
}
```

### 4.3 Concurrency and locking (engineering constraints)

#### 4.3.1 Lock ordering (must be fixed)

1. Acquire entry handles from `FdTable` (lookup only; do not block while holding a global write lock)
2. `EpFd` watch set lock
3. Specific fd inner locks

Never acquire in reverse order.

#### 4.3.2 Wakeup rules (must be implemented)

When `poll_mask()` may transition from not containing a bit to containing it:

- iterate `watchers` and notify each epfd.

Typical triggers:

- recv queue empty -> non-empty (IN)
- send queue full -> non-full (OUT)
- error (ERR)
- close/peer close (HUP)

#### 4.3.3 Spurious wakeups

Allowed. `ep_wait` must loop “scan -> wait -> rescan”.

### 4.4 `epoll` implementation (field-level)

#### 4.4.1 `EpollState`

```rust
pub struct EpollState {
    pub watch: std::collections::HashMap<i32, PollEvents>,
    pub notify: tokio::sync::Notify,
}
```

#### 4.4.2 `ep_ctl`

- `ADD`:
  - validate fd exists and is pollable
  - insert into watch set
  - insert epfd into fd.entry.watchers
- `DEL`:
  - remove from watch set
  - remove epfd from fd.entry.watchers
- `MOD`:
  - update watch set

#### 4.4.3 `ep_wait` algorithm (pseudo)

```text
loop {
  ready = []
  for (fd, interests) in watch_set {
    mask = fd.poll_mask()
    revents = mask & interests
    if revents != 0 { ready.push((fd, revents)) }
  }
  ready = dedup_by_fd(ready)
  sort_by_fd(ready)
  if ready not empty { write out_buf; return n }
  if timeout==0 { return 0 }
  await notify with timeout
}
```

Use monotonic time.

---

## 5. Onboarding a new fd kind (engineering checklist)

To make a new resource pollable (e.g. object storage streaming fd), you must:

1. Register it in `FdTable` with a new `FdKind`
2. Define `poll_mask()` precisely (when IN/OUT/ERR/HUP)
3. Call `notify_watchers()` on readiness transitions
4. Ensure `-EAGAIN` behavior matches the IN/OUT readiness contract
5. On close:
   - set `closed=true`
   - include `HUP` in `poll_mask`
   - notify watchers

---

## 6. Migration: make `cchat_*` pollable via fd/epoll

### 6.1 Goal

There are two migration targets (choose one):

- **Target A (compatibility-first)**: keep `cchat_*` public ABI mostly unchanged; migrate internals into `FdTable` so epoll can observe readiness.
- **Target B (engineering-first, breaking allowed)**: converge `cchat_*` onto the shared fd/errno/ctl model, changing signatures/error codes/commands as needed.

Regardless of A or B:

- response fds must be pollable (`EPOLLIN/HUP/ERR`)
- close must wake `ep_wait`

### 6.2 Mapping rules

- `cchat_create`: alloc `ChatSession` entry and return fd
- `cchat_send`: alloc `ChatResponse` entry; after writing bytes, mark as `EPOLLIN` and notify
- `cchat_recv`: read bytes from `ChatResponse` entry
- `cchat_close`: route through `FdTable.close` internally

### 6.4 Recommended scope of breaking changes

The following breaking changes are considered reasonable and often desirable to align with the general subsystem:

1. **Unify errors under `-errno`**
   - deprecate fixed `-1..-5` or keep only as a compatibility shim
   - update C SDK and samples accordingly

2. **Introduce/require `spear_fd_ctl` for generic controls**
   - move shared controls (flags/status/metrics) to `spear_fd_ctl`

3. **Unify close entrypoint (long-term)**
   - allow `cchat_close` to become a wrapper over `spear_fd_ctl(FD_CTL_CLOSE)` or be removed

4. **Change send/recv semantics to support async/streaming**
   - allow `cchat_send` to become “return response-fd immediately, produce output in background”
   - allow `cchat_recv` to converge toward a generic response-fd read model (e.g. future `spear_fd_read`)

### 6.5 Impacted files that must be updated with any breaking change

- Docs:
  - `docs/api/spear-hostcall/chat-completion-en.md`
  - `docs/api/spear-hostcall/chat-completion-zh.md`
- C SDK and samples:
  - `sdk/c/include/spear.h`
  - `samples/wasm-c/chat_completion.c`
- Rust link/behavior tests:
  - `src/spearlet/execution/runtime/wasm.rs` (WAT import symbol tests)
  - `src/spearlet/execution/host_api.rs` (cchat pipeline unit tests)
  - `tests/wasm_openai_e2e_tests.rs` (if relying on specific symbols/semantics)

Recommended addition: an “ABI version test” using `spear_fd_ctl(FD_CTL_GET_KIND/GET_FLAGS)` or an explicit `spear_abi_version()` mechanism.

### 6.3 Step-by-step refactor checklist

- Step 1: introduce `FdTable` (can start inside `host_api.rs`)
- Step 2: introduce `EpollState` and `spear_epoll_*` glue
- Step 3: implement `RtAsrFd` as the first pollable fd
- Step 4: migrate `ChatResponse` into `FdTable`, set `EPOLLIN` on ready
- Step 5: migrate `ChatSession` into `FdTable`
- Step 6: add `spear_fd_ctl` and share common controls

Acceptance criteria:

- `ep_wait` can watch both an `rtasr_fd` and a `cchat_send` response fd
- close wakes `ep_wait` with `EPOLLHUP`

---

## 7. Test checklist

### 7.1 ABI link tests

- WAT imports for `spear_epoll_*` and `spear_fd_ctl` must link

### 7.2 epoll semantics

- empty watch set: timeout returns 0
- ready fds: dedup and ascending fd ordering
- buffer-too-small: `-ENOSPC` and required length writeback
- close wakeup: wait returns immediately with HUP

### 7.3 cchat integration

- `cchat_send` response fd becomes `EPOLLIN`
- `cchat_close` results in `EPOLLHUP`

---

Version: engineering v1 (2026-01)
