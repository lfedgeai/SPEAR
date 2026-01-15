# Mic capture (mic_fd) implementation notes

This document is an implementation-oriented companion to the `mic_*` WASM hostcalls. The goal is to support **real microphone capture** exposed as **fd + epoll**, and integrate cleanly with `rtasr_fd`.

For detailed Chinese notes, see: [mic-fd-implementation-zh.md](./mic-fd-implementation-zh.md).

## 0. Related docs and code entry points

### 0.1 Specs / designs

- Realtime ASR implementation notes: [realtime-asr-implementation-zh.md](./realtime-asr-implementation-zh.md)
- fd/epoll subsystem: [fd-epoll-subsystem-en.md](../api/spear-hostcall/fd-epoll-subsystem-en.md)

### 0.2 Existing code locations

- `mic_*` host API: [mic/mod.rs](../../src/spearlet/execution/host_api/mic/mod.rs)
- `MicState/MicConfig`: [types.rs](../../src/spearlet/execution/hostcall/types.rs)
- fd table + epoll: [fd_table.rs](../../src/spearlet/execution/hostcall/fd_table.rs)
- WASM hostcall glue: [wasm_hostcalls.rs](../../src/spearlet/execution/runtime/wasm_hostcalls.rs)

### 0.3 Current status

`mic_fd` currently uses a stub producer (periodic fake frames), not real device audio.
This plan upgrades the stub into a pluggable input source and adds a real device capture path (macOS/Windows/Linux).

## 1. Goals, boundaries, testability

### 1.1 Goals

- Keep guest-facing ABI stable: `create/ctl/read/close` + epoll readiness.
- Support real mic capture: read PCM from OS device and push frames into `MicState.queue` based on `frame_ms`.
- Backpressure: when guest reads too slowly and the queue is full, drop frames by policy and count drops.
- Keep CI runnable: default tests must not require microphone permissions (stub/file mode).

### 1.2 Non-goals (v1)

- No permission UI flows inside WASM guest.
- No loopback/system-audio capture.
- No advanced DSP (AEC/AGC/NS), only minimal conversion/resampling.

## 2. Guest ABI (WASM hostcalls)

### 2.1 Signatures

- `mic_create() -> i32`
- `mic_ctl(fd: i32, cmd: i32, arg_ptr: i32, arg_len_ptr: i32) -> i32`
- `mic_read(fd: i32, out_ptr: i32, out_len_ptr: i32) -> i32`
- `mic_close(fd: i32) -> i32`

`mic_ctl` follows the same convention as `rtasr_ctl`: JSON bytes in, response is written back into the same buffer.

### 2.2 Readiness semantics

For `mic_fd`:

- `EPOLLIN`: `MicState.queue` is non-empty (readable)
- `EPOLLERR`: `MicState.last_error` is set (device error/permission denied)
- `EPOLLHUP`: closed or capture stopped

### 2.3 mic_read semantics

- One `mic_read` returns exactly one frame (size determined by sample_rate/channels/format/frame_ms).
- If queue is empty: `-EAGAIN`.
- If out buffer is too small: `-ENOSPC` and write required length into `*out_len_ptr`.

## 3. Control plane: mic_ctl commands

### 3.1 Command ids

- `MIC_CTL_SET_PARAM = 1`

Future extensions (optional):

- `MIC_CTL_LIST_DEVICES = 2`
- `MIC_CTL_GET_STATUS = 3`

### 3.2 MIC_CTL_SET_PARAM JSON

Keep current fields:

```json
{
  "sample_rate_hz": 24000,
  "channels": 1,
  "format": "pcm16",
  "frame_ms": 20
}
```

Proposed extensions:

```json
{
  "source": "device",
  "device": {"name": "MacBook Pro Microphone"},
  "sample_rate_hz": 24000,
  "channels": 1,
  "format": "pcm16",
  "frame_ms": 20,
  "max_queue_bytes": 524288,
  "drop_policy": "drop_oldest",
  "fallback": {"to_stub": true},
  "stub_pcm16_base64": "..."
}
```

Field notes:

- `source`: `device | file | stub`
- `device.name`: optional; empty means system default input device
- `max_queue_bytes`: overrides `MicState.max_queue_bytes`
- `drop_policy`: v1 can fix to `drop_oldest` but keep field for evolution
- `fallback.to_stub`: if device is unavailable/permission denied, fall back to stub (default true)
- `stub_pcm16_base64`: only for `source=stub`, loops a given PCM16 byte sequence

Return value:

- success: `0`
- failure: `-errno`

## 4. Host-side state (types.rs)

Existing:

- `MicConfig { sample_rate_hz, channels, frame_ms, format }`
- `MicState { config, queue, queue_bytes, max_queue_bytes, dropped_frames, last_error, running }`

Suggested v1 fields (design-only):

- source kind/handle (device/file/stub)
- capture status
- last overflow timestamp for diagnostics

## 5. Suggested module split

To avoid growing host_api modules further:

- `src/spearlet/execution/audio/mod.rs`
  - `mic.rs`: MicSource abstraction + config parsing
  - `convert.rs`: format conversion/mixing/resampling

Minimal trait:

```rust
pub struct MicStartRequest {
    pub device_name: Option<String>,
    pub output_sample_rate_hz: u32,
    pub output_channels: u8,
    pub output_format: String,
    pub frame_ms: u32,
}

pub struct MicFrame {
    pub bytes: Vec<u8>,
}

pub trait MicSource: Send {
    fn start(&mut self, req: MicStartRequest) -> Result<(), String>;
    fn stop(&mut self);
    fn try_recv_frame(&mut self) -> Option<MicFrame>;
}
```

Suggested implementations:

- Stub source
- File source (tests)
- Device source (cross-platform library recommended: `cpal`)

## 6. Threading model and data flow

Avoid heavy work inside OS audio callbacks.

Recommended pipeline:

1) capture callback thread writes raw samples into a ring buffer
2) a host background task pulls samples, converts/resamples, frames by `frame_ms`, and pushes into `MicState.queue`

Backpressure for mic differs from rtasr:

- mic produces continuously; prefer realtime; when full, drop oldest.
- after pushing a new frame, if readiness transitions to `IN`, notify watchers.

## 7. Implementation mapping (function-level)

- `mic_create`: alloc fd entry and initialize state
- `mic_ctl(MIC_CTL_SET_PARAM)`: parse JSON, apply config, start/restart source tasks
- `mic_read`: pop one frame, update readiness
- `mic_close`: stop source tasks, mark closed, set HUP, notify

## 8. Error codes and observability

Recommended errno mapping:

- `-EBADF`: invalid fd
- `-EINVAL`: invalid JSON/unsupported values
- `-EAGAIN`: empty queue for `mic_read`
- `-EIO`: device errors
- `-ENOSPC`: out buffer too small

Diagnostics:

- `MicState.last_error`: human readable, no secrets
- `MicState.dropped_frames`: drop counter

## 9. Testing strategy (no real mic required)

- Keep stub tests for fd/epoll semantics
- Add file mode tests:
  - use deterministic PCM input
  - assert framing sizes/count
  - assert drop counters on overflow

Manual tests (optional/ignored): device mode produces non-zero frames.

## 10. Combining mic_fd with rtasr_fd

In guest, use epoll to wait on both:

- mic_fd IN: read PCM frame and `rtasr_write`
- rtasr_fd OUT: send queue is writable; flush pending audio
- rtasr_fd IN: read JSON events

With server_vad segmentation, guest typically does not need to flush explicitly.

