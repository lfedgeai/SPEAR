# Spear Hostcall Proposal: Realtime ASR (`rtasr_fd`) on top of the general fd/epoll subsystem

This document focuses on Realtime ASR (`rtasr_*`) specifics. The engineering-grade design for the general fd/epoll subsystem (fd table, epoll, wakeups, concurrency, migration) is defined in:

- [fd-epoll-subsystem-en.md](file:///Users/bytedance/Documents/GitHub/bge/spear/docs/api/spear-hostcall/fd-epoll-subsystem-en.md)

## 1. Background

The current WASM hostcall surface already follows a syscall-like, fd-based model (e.g. `cchat_*`). This works well for request/response interactions, but realtime ASR is a full-duplex streaming workload:

- Input side: continuously ingest user audio and send audio chunks
- Output side: continuously receive ASR events (delta/completed/error)

These two flows must progress “at the same time”. In practice, a WASM guest is commonly single-threaded or heavily constrained, so the POSIX solution is not “real threads”, but **non-blocking I/O + readiness notification** (poll/epoll) to multiplex multiple fds in one event loop.

In the legacy Go implementation (`legacy/spearlet/stream/rt_asr.go` + `legacy/spearlet/stream/backend/openai.go`), WebSocket receive happens in goroutines and events are forwarded to the task. The Rust/wasm runtime needs an equivalent hostcall design that bridges host-side async WebSocket I/O into guest-visible fd semantics.

## 2. Goals / Non-Goals

### 2.1 Goals

- Provide a **full-duplex fd abstraction** for realtime ASR: write audio, read events.
- Enable guest-side “async” using POSIX-style semantics:
  - non-blocking `read/write` returning `-EAGAIN`
  - epoll-style wait to avoid busy loops
- Support backpressure: when host send-queue is full, guest observes `EAGAIN` and/or `EPOLLOUT` readiness.
- Match the existing ABI habits used by `cchat_*`: pointers+lengths, out buffer length negotiation, fd lifecycle.
- Preserve security boundaries: secrets/URLs/policies are host-controlled; WASM can only request capabilities/params.

#### 2.1.1 Generalization goal (epoll for all hostcall fds)

The epoll mechanism in this proposal must not be ASR-specific. It is intended to become Spear’s general readiness layer for WASM hostcalls, enabling future async/multiplexed features such as:

- streaming AI (realtime voice, streaming chat, streaming RAG)
- controlled network I/O (future restricted sockets / HTTP streaming fds)
- controlled file/object streaming (upload/download as streams)
- runtime/task event subscriptions (logs/events as streams)

As a result, the implementation will require a significant refactor to unify fd types, lifecycle, and readiness computation across hostcall families.

### 2.2 Non-Goals

- Full WASI sockets/filesystem compatibility.
- A generalized multi-transport streaming layer in v1; focus on realtime ASR first.
- Forcing a specific guest async runtime; the hostcalls are minimal syscalls that guests can wrap.

## 3. Design Principles (POSIX Alignment)

1. **fd is the handle**: the guest only holds `i32` fds, never host pointers.
2. **Non-blocking semantics**: `-EAGAIN` for “would block”.
3. **Register once, wait repeatedly**: provide epoll-style APIs so the guest does not rebuild `pollfd[]` arrays every loop.
4. **Simple, explicit memory contracts**: keep the `out_ptr/out_len_ptr` style already used by `cchat_recv`.
5. **Observable state**: offer `GET_STATUS/GET_METRICS` control ops for debugging and monitoring.

6. **Built on the general subsystem**: `rtasr_fd` is one `FdKind::RtAsr` implementation and must follow the shared readiness/close/wakeup/flags contracts.

## 4. High-Level Architecture

### 4.1 Data Flow

- `rtasr_write(fd, pcm16_chunk)`:
  - host enqueues chunk into a per-session send queue
  - host background task drains the queue and sends WebSocket messages

- Host background task reads WebSocket events:
  - enqueues event frames into a per-session recv queue
  - updates readiness: `EPOLLIN` becomes ready

- `rtasr_read(fd, out)`:
  - pops one complete event (UTF-8 JSON bytes) from recv queue
  - returns `-EAGAIN` if queue is empty and fd is non-blocking

### 4.2 Concurrency Model

- Host: at least one background task per realtime ASR session to implement full-duplex WS I/O.
- Guest: single-threaded event loop + `ep_wait` provides “simultaneous” progress.

### 4.3 Readiness Model

- `EPOLLIN`: recv queue is non-empty (a `rtasr_read` will succeed)
- `EPOLLOUT`: send queue has capacity (a `rtasr_write` will succeed)
- `EPOLLERR`: connection/session error
- `EPOLLHUP`: peer closed or session closed


## 4. Dependency: the general fd/epoll subsystem

`rtasr_*` depends on the general subsystem for: fd table allocation, epoll multiplexing, fd flags, shared close/wakeup semantics, and (optionally) `spear_fd_ctl`.

Engineering details are not duplicated here. See:

- [fd-epoll-subsystem-en.md](file:///Users/bytedance/Documents/GitHub/bge/spear/docs/api/spear-hostcall/fd-epoll-subsystem-en.md)

### 4.4 Session State Machine (`rtasr_fd`)

To make guest/host behavior deterministic, each `rtasr_fd` follows an explicit state machine:

- `INIT`: after `rtasr_create`, not configured/connected
- `CONFIGURED`: at least one parameter set (optional)
- `CONNECTING`: `RTASR_CTL_CONNECT` issued, session creation / connect in progress
- `CONNECTED`: WebSocket established, background tasks running
- `DRAINING`: write-half shutdown; no more audio accepted, events can still be read
- `CLOSED`: `rtasr_close` called or peer closed
- `ERROR`: unrecoverable error (inspect `GET_STATUS.last_error`)

Transition summary:

- `INIT/CONFIGURED -> CONNECTING -> CONNECTED`
- `CONNECTED -> DRAINING` (e.g. `RTASR_CTL_SHUTDOWN_WRITE`)
- `CONNECTED/DRAINING -> CLOSED` (close or peer close)
- Any state may enter `ERROR` on fatal errors; `rtasr_close` then enters `CLOSED`

### 4.5 Queues, Backpressure, and Fairness

Each `rtasr_fd` maintains:

- Send queue: audio chunks written by the guest
- Receive queue: event frames read from WebSocket

Backpressure (recommended):

- Send queue bounded by `max_send_queue_bytes`
- If `send_queue_bytes + incoming_len > max_send_queue_bytes`:
  - non-blocking: `rtasr_write` returns `-EAGAIN`
  - blocking (if ever added): block until capacity is available

Receive overflow policy (must be explicit):

- Receive queue bounded by `max_recv_queue_bytes`
- On overflow, default to `drop_oldest` and increment `dropped_events` to prevent unbounded memory growth

Fairness:

- `rtasr_read` returns exactly one frame; guests can drain in a loop until `-EAGAIN`
- Host background loops should avoid starvation (e.g. select fairness)

## 5. Required Changes to Current Architecture

Minimal-intrusion changes aligned with the current layout:

- `src/spearlet/execution/host_api.rs`
  - Add Realtime ASR host state (fd table, queues, state machine, metrics)
  - Add epoll host state (epfd table, watch sets, wait/wakeup)
  - Introduce a reusable tokio runtime for host-side async tasks

- `src/spearlet/execution/runtime/wasm_hostcalls.rs`
  - Add exported `rtasr_*` hostcalls
  - Add exported `spear_epoll_*` hostcalls
  - Define errno-based return conventions for new families

- `Cargo.toml`
  - If implementing real WebSocket connectivity, add a WS client dependency (e.g. `tokio-tungstenite` + `url`)
  - If v1 is an abstraction/stub first, dependencies can be deferred

### 5.1 Suggested Module Split (without breaking current layout)

To prevent `host_api.rs` from growing indefinitely, a gradual split is recommended (v1 may still land in existing files):

- `src/spearlet/execution/hostcall/memory.rs`: linear memory helpers
- `src/spearlet/execution/hostcall/errno.rs`: errno mapping and buffer-too-small conventions
- `src/spearlet/execution/hostcall/epoll.rs`: epfd table, watch sets, wait/wakeup
- `src/spearlet/execution/stream/rtasr.rs`: rtasr state machine, queues, WS connect/send/recv

Minimal landing path for v1:

- Add `RtAsrHostState` and `EpollHostState` directly to `host_api.rs`
- Add `rtasr_*` and `spear_epoll_*` glue in `wasm_hostcalls.rs`

### 5.4 Migration plan for fd refactor (phased)

Because epoll is intended to serve all hostcalls, the fd refactor should be phased:

#### Phase A: Introduce unified fd table + epoll; integrate only rtasr

- Add `FdTable` and `Epoll`
- Make `RtAsrFd` the first `Pollable` implementation
- Keep `cchat_*` untouched, but reserve integration hooks

#### Phase B: Integrate cchat response fds into epoll

- Migrate response storage into `ChatResponseFd` entries
- Define readiness for chat response fds (`EPOLLIN` when bytes ready)

#### Phase C: Integrate other stream-like fds

- object storage stream fds
- streaming chat / realtime voice

#### Phase D: Optional API convergence (long-term)

- converge `cchat_close/rtasr_close/...` into a unified `spear_fd_close`
- converge family-specific `*_ctl` into a unified `spear_fd_ctl` (or keep family APIs but unify internals)

### 5.2 Host-side Wakeup Semantics (how `ep_wait` is awakened)

`ep_wait` must be wakeable, otherwise guests will busy-loop.

Recommended approach:

- Each `epfd` owns a `Notify/Condvar`-like primitive
- When any watched fd may change readiness (recv queue becomes non-empty, send queue becomes non-full, ERROR/HUP), notify waiters
- `spear_epoll_wait` algorithm:
  - scan watch set and compute the ready set
  - if empty and timeout allows waiting: wait on notify/condvar
  - on wakeup: rescan and return

### 5.3 WASM Instance Shutdown and Resource Reclamation

- On instance termination, close all fds (at least `rtasr_fd` and `epfd`) to avoid leaked background tasks
- `close` should be idempotent: either return `0` or `-EBADF` on repeated closes (define explicitly)
- Closing any watched fd should wake `ep_wait` immediately (return `-EINTR` or deliver `EPOLLHUP`)

### 5.5 Detailed mapping: making `cchat_*` pollable via epoll (recommended)

To make epoll truly generic, integrate existing chat fds into the unified fd table while keeping the public `cchat_*` API stable.

#### 5.5.1 fd kinds

- `cchat_create` returns a `ChatSessionFd`
- `cchat_send` returns a `ChatResponseFd`

#### 5.5.2 `ChatResponseFd` readiness

- `EPOLLIN`: response bytes are available to be read via `cchat_recv`
- `EPOLLHUP`: after `cchat_close(resp_fd)`
- `EPOLLERR`: internal host error; inspect via `cchat_ctl(GET_STATUS/GET_METRICS)` or a future generic `FD_CTL_GET_STATUS`

#### 5.5.3 `ChatSessionFd` readiness (optional)

With the current two-phase model (write request -> send -> read response-fd), `ChatSessionFd` does not need to be pollable.

If streaming chat is added, keep the output on `ChatResponseFd` so it can behave as a readable stream:

- new incremental tokens available: `EPOLLIN`
- completion indicated via event frames or status

#### 5.5.4 Optional: truly async send for chat

Today `cchat_send` computes synchronously and writes the response immediately. For async/streaming chat, add:

- `cchat_send_async(fd) -> response_fd`: returns immediately, response is produced in background
- when response becomes readable, mark `ChatResponseFd` as `EPOLLIN` and wake epoll

This enables a single guest event loop to multiplex chat and rtasr.

## 6. ABI and Error Conventions

### 6.1 General Rules

- WASM-friendly types only: `i32/i64`, pointers are `i32` offsets into linear memory.
- Success returns:
  - `*_create`: `fd > 0`
  - `read`: bytes written (`>= 0`)
  - `write`: bytes consumed (`>= 0`)
  - `ctl`: `0` or bytes written for “get” operations
- Failure returns: negative values, preferably **`-errno`**.

### 6.2 Recommended errno Subset

- `-EBADF`: invalid fd or fd type mismatch
- `-EINVAL`: invalid args/cmd/JSON/flags
- `-EAGAIN`: would block (non-blocking read/write)
- `-ENOSPC`: output buffer too small (or keep `-3` compatibility, see 6.4)
- `-ENOTCONN`: read/write before connect
- `-ECONNRESET/-ECONNABORTED`: connection reset/aborted
- `-ETIMEDOUT`: connect/wait timeout
- `-EINTR`: interrupted by close/cancel

### 6.3 Output Buffer Contract

Keep the `cchat_recv` pattern:

- `out_ptr`: start of output buffer
- `out_len_ptr`: points to a `u32`; input is capacity, output is actual length (or required length)

If `capacity < required`:

- write `required` back to `*out_len_ptr`
- return `-ENOSPC` (or `-3` for compatibility)

### 6.4 Compatibility with existing `cchat_*`

Current `cchat_*` uses fixed error codes `-1..-5`. To avoid breaking changes:

- New `rtasr_*` and `spear_epoll_*` adopt `-errno`.
- Existing `cchat_*` remains unchanged.
- Long term, `cchat_*` can migrate to `-errno` with a compatibility shim.

## 7. fd Types and Lifecycle

### 7.1 fd Types

- `rtasr_fd`: realtime ASR session fd
- `epfd`: epoll instance fd

Host must keep a global fd allocator and type-tag each fd.

### 7.2 Lifecycle

- `*_create` allocates fd and initializes state.
- `*_close` releases fd:
  - cancels background tasks
  - closes WebSocket
  - wakes any `ep_wait` blocked on the fd/epfd (return `-EINTR` or deliver `EPOLLHUP`)

## 8. Realtime ASR Hostcalls (`rtasr_*`)

### 8.1 Signatures

- `rtasr_create() -> i32`
- `rtasr_ctl(fd: i32, cmd: i32, arg_ptr: i32, arg_len_ptr: i32) -> i32`
- `rtasr_write(fd: i32, buf_ptr: i32, buf_len: i32) -> i32`
- `rtasr_read(fd: i32, out_ptr: i32, out_len_ptr: i32) -> i32`
- `rtasr_close(fd: i32) -> i32`

### 8.1.1 Minimal Complete Set

To enable full-duplex async wrappers in the guest, the minimal complete set is:

- `create/close`: lifecycle
- `ctl(SET_PARAM, CONNECT, GET_STATUS)`: configuration and status
- `read/write`: data plane
- `spear_epoll_*`: readiness wait

Blocking read/write + threads is not portable for single-threaded WASM guests.

### 8.2 `rtasr_ctl` Commands

#### 8.2.1 `RTASR_CTL_SET_PARAM = 1`

- Input: `arg_ptr/arg_len_ptr` points to UTF-8 JSON
- JSON shape:

```json
{ "key": "input_audio_transcription.model", "value": "gpt-4o-mini-transcribe" }
```

- Behavior: store into a session param map; no network I/O.

Suggested keys for v1:

- `input_audio_format`: `"pcm16" | ...`
- `input_audio_transcription.model`: string
- `turn_detection.*`: object
- `vad.*`: object
- `max_send_queue_bytes`: number
- `max_recv_queue_bytes`: number
- `nonblock`: boolean (equivalent to `O_NONBLOCK`)

#### 8.2.1.1 Parameter hierarchy and defaults (recommended)

Use “flat key + JSON value” to avoid ABI churn.

Recommended defaults:

- `input_audio_format = "pcm16"`
- `input_sample_rate_hz = 24000`
- `input_channels = 1`
- `max_send_queue_bytes = 1024 * 1024`
- `max_recv_queue_bytes = 1024 * 1024`
- `nonblock = true`

Additional keys to reserve (optional for v1, recommended for evolution):

- `backend = "openai_chat_completion" | "openai_realtime_ws" | ...` (name only; no URL/secret)
- `model = "gpt-4o-mini-transcribe" | ...` (shortcut)
- `connect_timeout_ms = 10000`
- `idle_timeout_ms = 60000`
- `drop_policy = "drop_oldest" | "drop_newest" | "error"`

#### 8.2.2 `RTASR_CTL_CONNECT = 2`

- Behavior:
  - create a transcription session (e.g. OpenAI `POST /v1/realtime/transcription_sessions`)
  - establish WebSocket using `client_secret`
  - spawn background tasks to implement full-duplex WS I/O and readiness
- Return: `0` or `-errno`

#### 8.2.2.1 OpenAI realtime mapping (reference)

For OpenAI realtime transcription (matching the legacy design):

- Create session: `POST /v1/realtime/transcription_sessions`
- WebSocket: `wss://api.openai.com/v1/realtime?intent=transcription`
- Headers:
  - `Authorization: Bearer <client_secret>`
  - `OpenAI-Beta: realtime=v1`

Host responsibilities:

- read API key/base_url from host config
- map guest params (model, turn_detection, etc.) into the session config
- prefer pass-through event frames to reduce semantic drift in v1

#### 8.2.3 `RTASR_CTL_GET_STATUS = 3`

- Output: write a JSON blob to `arg_ptr/arg_len_ptr`

Suggested fields:

```json
{
  "connected": true,
  "nonblock": true,
  "send_queue_bytes": 0,
  "recv_queue_bytes": 1024,
  "dropped_events": 0,
  "last_error": null
}
```

#### 8.2.4 `RTASR_CTL_SHUTDOWN_WRITE = 4` (recommended)

Purpose: half-close semantics (align with `shutdown(SHUT_WR)`): stop accepting audio while still receiving output.

- Behavior:
  - transition to `DRAINING`
  - subsequent `rtasr_write` returns `-EPIPE` or `-ENOTCONN` (pick one and standardize)
  - host may send commit/finish frames if required by the backend protocol

#### 8.2.5 `RTASR_CTL_GET_METRICS = 5` (recommended)

Purpose: read session metrics (throughput, queue sizes, drops, latency hints).

Suggested JSON:

```json
{
  "audio_bytes_sent": 123456,
  "events_received": 42,
  "dropped_events": 0,
  "connect_rtt_ms": 120,
  "last_event_time_ms": 1730000000000
}
```

### 8.3 `rtasr_write` Semantics

- Input: raw audio bytes (recommended pcm16 LE)
- Non-blocking behavior:
  - if send queue is full: return `-EAGAIN`
  - otherwise enqueue and return `buf_len`

Host background tasks encode/send protocol frames (e.g. base64 for `input_audio_buffer.append`).

#### 8.3.1 Audio framing guidance

The host should not require a fixed chunk size, but for lower latency recommend:

- 20ms or 40ms frames (pcm16 24kHz mono: 20ms ≈ 1920 bytes)

If guests write very large chunks, v1 should enqueue as-is (simplest). Host-side slicing is optional and can be deferred.

#### 8.3.2 Return value conventions

- success: return “bytes consumed”
- failure: return `-errno`

Recommended for v1: all-or-nothing (no partial writes) to keep guest logic simple.

### 8.4 `rtasr_read` Semantics

- Output: one complete event frame as UTF-8 JSON bytes
- Behavior:
  - if recv queue non-empty: pop one and write to output
  - if empty and non-blocking: return `-EAGAIN`
  - blocking reads are not required for v1; use `ep_wait` instead

#### 8.4.1 Frame boundaries

`rtasr_read` returns exactly one “complete frame”:

- WebSocket text frame: one UTF-8 JSON event
- WebSocket binary frame: pass-through bytes (v1)

Host should not merge multiple frames into higher-level semantic events in v1.

#### 8.4.2 Guest drain pattern

When `ep_wait` indicates `EPOLLIN`, guests should call `rtasr_read` in a loop until `-EAGAIN`.

### 8.5 Event Format

Prefer pass-through of backend events in v1; guest can switch on `type`, e.g.:

- `conversation.item.input_audio_transcription.delta`
- `conversation.item.input_audio_transcription.completed`
- `error`
- `input_audio_buffer.*`

#### 8.5.1 Minimal recommended fields

For cross-backend survivability, guests should at least parse:

- `type: string`
- `event_id: string` (if present)
- `error.*` (if present)

For OpenAI realtime transcription common fields include `delta` and `transcript`.

## 9. epoll Hostcalls (`spear_epoll_*`)

### 9.1 Signatures

- `spear_epoll_create() -> i32`
- `spear_epoll_ctl(epfd: i32, op: i32, fd: i32, events: i32) -> i32`
- `spear_epoll_wait(epfd: i32, out_ptr: i32, out_len_ptr: i32, timeout_ms: i32) -> i32`
- `spear_epoll_close(epfd: i32) -> i32`

### 9.2 Constants

#### 9.2.1 ctl ops

- `EPOLL_CTL_ADD = 1`
- `EPOLL_CTL_MOD = 2`
- `EPOLL_CTL_DEL = 3`

#### 9.2.2 event bits

- `EPOLLIN  = 0x001`
- `EPOLLOUT = 0x004`
- `EPOLLERR = 0x008`
- `EPOLLHUP = 0x010`

#### 9.2.3 Semantic constraints for event bits

- `EPOLLIN`: calling the fd’s read/recv will return >= 0 (not necessarily drain everything)
- `EPOLLOUT`: calling the fd’s write/send will return >= 0 (not necessarily permanently writable)
- `EPOLLERR`: an error state exists; use `GET_STATUS`/family status to inspect details
- `EPOLLHUP`: peer closed or local close; fd is no longer usable

epoll only reports readiness; it does not replace the fd’s read/write interfaces.

### 9.3 Output Events Buffer Layout

To avoid ABI alignment issues, `spear_epoll_wait` returns a packed little-endian array.

Each record is 8 bytes:

- `fd: i32` (4 bytes)
- `events: i32` (4 bytes)

So `required = n * 8`.

#### 9.3.1 Buffer-too-small behavior (must be explicit)

`spear_epoll_wait` output count depends on both ready fds and buffer capacity.

Recommended semantics:

- `max_records = floor(capacity_bytes / 8)`
- if `max_records == 0`:
  - write back `8` to `*out_len_ptr`
  - return `-ENOSPC`
- else write `min(ready_count, max_records)` records and write back actual bytes

### 9.4 Wait Semantics

- `timeout_ms < 0`: wait indefinitely
- `timeout_ms == 0`: return immediately (poll)
- `timeout_ms > 0`: wait up to timeout

Wakeup when:

- any watched fd becomes ready
- fd/epfd is closed/cancelled

Return:

- `n >= 0`: wrote n event records, return n
- timeout: return `0`
- error: return `-errno`

### 9.5 Trigger mode: level-triggered (fixed in v1)

To reduce complexity, v1 is strictly level-triggered:

- as long as readiness holds (e.g. recv queue non-empty), `ep_wait` will keep returning `EPOLLIN`
- guests clear readiness by draining until `-EAGAIN`

Edge-triggered and oneshot are out of scope for v1.

### 9.6 epoll algorithm constraints (must be fixed)

To keep guest behavior predictable, these rules must be fixed:

#### 9.6.1 Deduplication per wait

- within a single `spear_epoll_wait` result, the same `fd` must appear at most once
- if multiple bits are ready (`EPOLLIN|EPOLLERR`), they must be OR-ed into the same record

#### 9.6.2 Result ordering

Recommend a stable rule: sort by ascending `fd`.

#### 9.6.3 Multiple epfds watching the same fd

- the same fd may be watched by multiple epfds
- readiness changes must wake all relevant epfds

#### 9.6.4 Spurious wakeups are allowed

Due to races, `ep_wait` may wake up and then observe no ready fds (e.g. another thread drained the queue).

- this is allowed
- `ep_wait` should continue waiting until timeout or readiness is observed

#### 9.6.5 Timeout and time source

- timeouts should use a monotonic clock
- on timeout, return `0`

### 9.7 Scale and performance constraints (recommended)

- bound the number of watched fds per epfd (e.g. 1024/4096), otherwise return `-ENOMEM` or `-EINVAL`
- watcher bookkeeping and notify must avoid O(N^2) behavior

## 10. Guest-side Full-Duplex Progress (Reference)

### 10.1 Single fd (ASR only)

- create `epfd`
- add `rtasr_fd` with `EPOLLIN|EPOLLERR|EPOLLHUP`
- loop `ep_wait`, then `rtasr_read` until `-EAGAIN`

### 10.2 Two fds (mic input + ASR output)

Assuming host provides a `mic_fd` with `mic_read`:

- watch `mic_fd: EPOLLIN`
- watch `rtasr_fd: EPOLLIN|EPOLLOUT|EPOLLERR|EPOLLHUP`
- `EPOLLIN(mic)`: read audio chunk, call `rtasr_write`
- `EPOLLIN(rtasr)`: read events and parse deltas
- `EPOLLOUT(rtasr)`: flush locally buffered audio if previously blocked

### 10.3 Reference pseudo-code (POSIX wrapper oriented)

```c
int epfd = spear_epoll_create();
spear_epoll_ctl(epfd, EPOLL_CTL_ADD, mic_fd, EPOLLIN | EPOLLERR | EPOLLHUP);
spear_epoll_ctl(epfd, EPOLL_CTL_ADD, rtasr_fd, EPOLLIN | EPOLLOUT | EPOLLERR | EPOLLHUP);

uint8_t evbuf[8 * 16];
uint32_t evlen = sizeof(evbuf);

for (;;) {
  evlen = sizeof(evbuf);
  int n = spear_epoll_wait(epfd, (int)evbuf, (int)&evlen, 50);
  if (n < 0) {
    continue;
  }
  for (int i = 0; i < n; i++) {
    int fd = *(int*)(evbuf + i * 8 + 0);
    int revents = *(int*)(evbuf + i * 8 + 4);

    if (fd == mic_fd && (revents & EPOLLIN)) {
      int r = mic_read(mic_fd, pcm_buf, pcm_cap);
      if (r > 0) {
        int w = rtasr_write(rtasr_fd, (int)pcm_buf, r);
      }
    }

    if (fd == rtasr_fd && (revents & EPOLLIN)) {
      for (;;) {
        uint32_t out_len = out_cap;
        int rr = rtasr_read(rtasr_fd, (int)out_buf, (int)&out_len);
        if (rr == -EAGAIN) break;
        if (rr < 0) break;
        handle_event(out_buf, out_len);
      }
    }

    if (fd == rtasr_fd && (revents & EPOLLOUT)) {
      flush_pending_audio();
    }

    if (revents & (EPOLLERR | EPOLLHUP)) {
      goto done;
    }
  }
}
done:
rtasr_close(rtasr_fd);
spear_epoll_close(epfd);
```

## 11. Security and Policy

- Secrets (API keys, client_secret) and URLs are host-managed; guest must not access them.
- Host must enforce:
  - outbound allowlists
  - limits on concurrency and session duration
  - send/recv queue limits to prevent host OOM
- Guest params are constrained by host policy.

### 11.1 Host config and guest param mapping (recommended)

Host config should define:

- `rtasr.backends[]`: available backends (name, kind, base_url, transport, capabilities)
- `rtasr.default_backend`
- `rtasr.allow_models[]` (or prefix/regex policies)
- `rtasr.max_sessions`, `rtasr.max_session_seconds`
- `rtasr.max_send_queue_bytes`, `rtasr.max_recv_queue_bytes`

Guest `SET_PARAM` can only select/restrict within host limits; it must not expand policy.

### 11.2 Usage/charging related fields

If usage can be obtained from the backend, expose it via `GET_METRICS` or pass-through events. The guest must not fabricate billing-related numbers.

## 12. Testing

- Link tests: WAT import checks for `rtasr_*` and `spear_epoll_*`.
- Semantics tests:
  - empty `rtasr_read` returns `-EAGAIN`
  - `spear_epoll_wait` wakes when events are enqueued
- Optional E2E: real WebSocket connectivity under a configured environment; otherwise stub the event source.

## 13. Compatibility and Evolution

- No required changes to existing `cchat_*`.
- Long term: unify all hostcall families under `-errno`.
- Future fd types:
  - realtime voice/TTS
  - controlled network sockets
  - controlled file/object streaming fds

## 14. Implementation Checklist

### 14.1 Must-have (v1)

- `rtasr_create/ctl(CONNECT, SET_PARAM, GET_STATUS)/read/write/close`
- `spear_epoll_create/ctl/wait/close`
- send/recv queues + backpressure + readiness computation
- close/cancel wakes `ep_wait`

### 14.2 Recommended (v1.1)

- `RTASR_CTL_SHUTDOWN_WRITE` (half-close)
- `RTASR_CTL_GET_METRICS` (metrics)
- stub backend to simulate delta/completed in environments without real keys

---

Version: proposal v0 (2026-01)
