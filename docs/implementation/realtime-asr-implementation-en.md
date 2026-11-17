# Realtime ASR (rtasr_fd) implementation notes

This document is an implementation-oriented companion to the Realtime ASR spec:

- [realtime-asr-epoll-en.md](../api/spear-hostcall/realtime-asr-epoll-en.md)
- [realtime-asr-epoll-zh.md](../api/spear-hostcall/realtime-asr-epoll-zh.md)

It describes how to implement Realtime ASR as fd + epoll style WASM hostcalls in this repository, including module boundaries, state machine, backpressure, and tests.

For deeper Chinese implementation details (including a large wasm-c style example loop), see: [realtime-asr-implementation-zh.md](./realtime-asr-implementation-zh.md).

## 0. Related docs and code entry points

### 0.1 Specs / designs

- fd/epoll subsystem: [fd-epoll-subsystem-en.md](../api/spear-hostcall/fd-epoll-subsystem-en.md)
- streaming/realtime guidance: [streaming-en.md](../backend-adapter/streaming-en.md)
- mic_fd implementation notes: [mic-fd-implementation-en.md](./mic-fd-implementation-en.md)

### 0.2 Existing code

- fd/epoll types: [types.rs](../../src/spearlet/execution/hostcall/types.rs)
- fd table: [fd_table.rs](../../src/spearlet/execution/hostcall/fd_table.rs)
- cchat example (fd + epoll precedent): [host_api.rs](../../src/spearlet/execution/host_api.rs)
- WASM hostcall glue: [wasm_hostcalls.rs](../../src/spearlet/execution/runtime/wasm_hostcalls.rs)
- legacy Go realtime ASR reference: `legacy/spearlet/stream/rt_asr.go`
- LLM credentials / credential_ref: [llm-credentials-implementation-en.md](./llm-credentials-implementation-en.md)

## 1. Goals and boundaries

### 1.1 Goals

- Add `rtasr_*` hostcalls: `create/ctl/read/write/close`.
- Put `rtasr_fd` into the unified fd table so it can be waited via `spear_epoll_*`.
- Use `-errno` for all new APIs (`-EBADF/-EINVAL/-EFAULT/-EAGAIN/-ENOSPC/-EIO`).
- Backpressure:
  - when send queue is full, `rtasr_write` returns `-EAGAIN`
  - when writable again, readiness includes `EPOLLOUT`
- Event stream:
  - when recv queue is non-empty, readiness includes `EPOLLIN`
  - guest reads one event per `rtasr_read`

### 1.2 Non-goals (v1)

- No full WASI sockets/filesystem.
- Guest must not be tied to a specific async runtime; it only depends on syscall-like hostcalls.
- No edge-triggered/oneshot epoll; v1 is level-triggered.

### 1.3 Audio source

`rtasr_write` accepts raw audio bytes, but audio sourcing is separate:

- external audio input (network/file/caller provides bytes)
- host microphone capture via `mic_fd`, which the guest can read and forward to `rtasr_write`

## 2. Guest ABI

### 2.1 Hostcalls

- `rtasr_create() -> i32`
- `rtasr_ctl(fd: i32, cmd: i32, arg_ptr: i32, arg_len_ptr: i32) -> i32`
- `rtasr_write(fd: i32, buf_ptr: i32, buf_len: i32) -> i32`
- `rtasr_read(fd: i32, out_ptr: i32, out_len_ptr: i32) -> i32`
- `rtasr_close(fd: i32) -> i32`

Mic (for e2e):

- `mic_create/mic_ctl/mic_read/mic_close`

Epoll:

- `spear_epoll_create/ctl/wait/close`

### 2.2 Event and buffer conventions

- `rtasr_read` returns exactly one complete event per call (UTF-8 JSON bytes).
- If out buffer is too small: write required length to `*out_len_ptr` and return `-ENOSPC`.
- `rtasr_write` appends bytes (input format is configured via `SET_PARAM`).
- If send queue is full: return `-EAGAIN`.

### 2.3 Readiness

For `rtasr_fd`:

- `EPOLLIN`: recv_queue non-empty
- `EPOLLOUT`: send_queue not full
- `EPOLLERR`: connection error (pair with `rtasr_ctl(GET_STATUS)`)
- `EPOLLHUP`: peer closed or local close

For `mic_fd`:

- `EPOLLIN`: capture queue non-empty
- `EPOLLERR`: capture error
- `EPOLLHUP`: stopped/closed

## 3. Suggested code organization

### 3.1 Minimal-intrusion (v0/v1)

Implement state in `host_api.rs` similar to cchat, but backed by the unified fd table:

- `FdKind::RtAsr`
- `FdInner::RtAsr(Box<RtAsrState>)`

### 3.2 Recommended split (v1.1+)

- `src/spearlet/execution/stream/rtasr.rs`
  - `RtAsrState`, queues, backpressure, state machine
  - transport abstraction (stub/ws)
- `src/spearlet/execution/hostcall/memory.rs` (shared mem helpers)
- `src/spearlet/execution/hostcall/errno.rs` (shared errno + ENOSPC policy)

## 4. RtAsrState: state machine and queues

### 4.1 State machine

Suggested states:

- Init
- Configured
- Connecting
- Connected
- Draining
- Closed
- Error

Transitions should follow the spec section on lifecycle.

### 4.2 Queues and backpressure

Maintain two queues per fd:

- send_queue: audio bytes + control events
  - track send_queue_bytes with a max limit
  - overflow: `rtasr_write -> -EAGAIN`
- recv_queue: JSON event bytes
  - track recv_queue_bytes with a max limit
  - overflow: drop_oldest + increment dropped_events

### 4.3 Epoll integration

Any queue/state change must:

- recompute fd entry poll_mask (IN/OUT/ERR/HUP)
- notify fd watchers (`fd_table.notify_watchers(fd)`)

Encapsulate with an internal helper like `recompute_readiness_and_notify(fd)`.

## 5. WASM hostcall glue

Add `rtasr_*` exports similarly to cchat:

- read input bytes from guest memory
- write output bytes with (out_ptr,out_len_ptr) convention
- ctl payload is UTF-8 JSON bytes

Prefer reusing shared `mem_read/mem_write` helpers.

## 6. Transport strategy

### 6.1 Phase A: stub transport

Purpose: validate fd/epoll/backpressure/state machine without new deps or real keys.

- after `CONNECT`, spawn a tokio task that periodically appends mock events into recv_queue

Acceptance:

- guest epoll loop receives EPOLLIN
- `rtasr_read` returns JSON events
- overflow behavior is observable via dropped counters

### 6.2 Phase B: real WebSocket transport

Suggested deps:

- `tokio-tungstenite` + `url`

Key behavior:

- connect: resolve base_url; attach auth via host credentials when configured (`credential_ref`)
- send: base64-encode audio and emit append events (aligned with legacy implementation)
- recv: convert WS frames into JSON bytes into recv_queue

Security:

- secrets live only on host side; guest must not access them
- logs must not print secrets/headers

Providers:

- OpenAI realtime (`rt_asr_openai_ws`) and Qwen/DashScope (`rt_asr_qwen_ws`)
  share the same data plane (`input_audio_buffer.append` / `commit`). Only the
  `session.update` shape and the live-text event differ (Qwen emits
  `conversation.item.input_audio_transcription.text` with `text` + `stash`),
  which the per-kind adapter and SDK normalize. See
  [backends-en.md](../backend-adapter/backends-en.md) for config examples.

## 7. Config and routing

Align with `SpearletConfig.ai.backends` (`ops/features/transports`), and introduce a dedicated op name for realtime ASR (e.g. `speech_to_text` + `websocket` transport or a separate op).

Must support:

- explicit backend selection via params
- host allowlist/denylist enforcement

Config notes:

- `[[spearlet.ai.backends]] hosting` is required and must be `local` or `remote`.
- `credential_ref` is optional; if omitted, the backend is treated as “no-auth”.
- For Qwen realtime ASR, the backend kind must remain `rt_asr_qwen_ws`.
  Do not reuse `rt_asr_openai_ws` just because the control-plane base URL may be
  an OpenAI-compatible `/compatible-mode/v1` endpoint; otherwise the runtime will
  bind the wrong adapter and the websocket connect step can fail before any
  transcript event is produced.

## 8. Control plane: rtasr_ctl commands (v1)

Recommended set:

- SET_PARAM (1)
- CONNECT (2)
- GET_STATUS (3)
- SEND_EVENT (4)
- FLUSH (5)
- CLEAR (6)
- SET_SEGMENTATION (7)
- GET_SEGMENTATION (8)

## 9. Tests and acceptance

- Rust unit tests (in-memory): read/write/backpressure/close + epoll wakeups
- WAT link tests: import `rtasr_*` and `spear_epoll_*` + `spear_fd_ctl`
- E2E:
  - stub mode always runs in CI
  - real WS mode runs only when credentials are available (optional/ignored)
- Live provider coverage:
  - [realtime_asr_live_tests.rs](../../tests/realtime_asr_live_tests.rs) enumerates
    all configured realtime ASR backends that match `speech_to_text + websocket`
    and runs the same fixture audio through each resolved backend
  - [qwen_realtime_asr_live_tests.rs](../../tests/qwen_realtime_asr_live_tests.rs) exercises the
    repository RTASR host API against Qwen realtime ASR using the embedded PCM
    fixture [qwen_live_e2e_hello.pcm](../../tests/fixtures/qwen_live_e2e_hello.pcm)
  - shared live-test convention:
    - `SPEAR_RUN_LIVE_TESTS=1`
    - config comes from `SPEAR_TEST_CONFIG` or the default `config/spearlet/config.toml`
    - the backend must be `rt_asr_qwen_ws`
    - the backend credential must use `kind = "env"`
    - the real secret is still provided via the shell using that credential's `api_key_env`
  - provider-specific override:
    - `QWEN_LIVE_MODEL`
  - defaults:
    - `QWEN_LIVE_MODEL=qwen3-asr-flash-realtime`
  - runtime wiring:
    - the test builds an in-memory `SpearletConfig`
    - it mirrors the resolved backend name/base URL from the shared test config
    - the credential kind is `env`
    - the secret is injected into `RuntimeConfig.global_environment` using the
      resolved credential env name, mirroring host-side env resolution
  - sample config:

    ```toml
    [[spearlet.ai.credentials]]
    name = "qwen_live"
    kind = "env"
    api_key_env = "DASHSCOPE_API_KEY"

    [[spearlet.ai.backends]]
    name = "qwen-live"
    kind = "rt_asr_qwen_ws"
    base_url = "https://maas.qianwenaiapi.com/compatible-mode/v1"
    credential_ref = "qwen_live"
    ops = ["speech_to_text"]
    transports = ["websocket"]
    ```

  - example:

    ```bash
    SPEAR_RUN_LIVE_TESTS=1 \
    DASHSCOPE_API_KEY='your-api-key' \
    cargo test test_qwen_realtime_asr_live_transcribes_fixture_audio --test qwen_realtime_asr_live_tests -- --nocapture
    ```

  - aggregate example:

    ```bash
    SPEAR_RUN_LIVE_TESTS=1 \
    OPENAI_API_KEY='your-openai-key' \
    DASHSCOPE_API_KEY='your-qwen-key' \
    cargo test test_all_realtime_asr_live_backends_transcribe_fixture_audio --test realtime_asr_live_tests -- --nocapture
    ```

  - recommendation:
    - keep OpenAI and Qwen live tests in the same shared config file so both
      providers follow the same enable/configure/secret-injection workflow
    - the default sample file [config.toml](../../config/spearlet/config.toml)
      now includes a `qwen-live` backend and `qwen_live` env credential

## 10. Operational troubleshooting notes

- `websocket disconnected (code 1005): connection closed` is usually only the
  browser-facing symptom. In this repository it often means the upstream
  user-stream or runtime websocket was closed without an explicit reason, so the
  real failure must be confirmed from `sms` / `spearlet` logs or backend status.
- If Qwen realtime ASR is configured but runtime routing reports no candidate
  backend, check `GET /admin/api/ai-backend-statuses/{backend_id}` first. The
  backend must reach `status=ready` and `available=true`; otherwise the node has
  not published a usable runtime backend yet.
- A node-side status such as `remote preflight does not support provider 'qwen'
  with kind 'rt_asr_qwen_ws'` means control-plane validation blocked the
  backend before routing. In that state, execution-level RTASR requests typically
  degrade to `no candidate backend ... backends=[]`.
- For Qwen specifically, a `base_url` copied from the compatible HTTP endpoint
  like `https://maas.qianwenaiapi.com/compatible-mode/v1` is valid, but the
  runtime adapter must normalize it back to the realtime websocket root before
  appending `/api-ws/v1/realtime?model=...`.
- If the runtime connects but subtitles still look incomplete, inspect the
  transcript consumer next. Qwen emits preview-style
  `conversation.item.input_audio_transcription.text` events with `text + stash`,
  and the final visible transcript must be sealed from the later
  `Completed(transcript)` event instead of assuming OpenAI-style delta-only
  behavior.
