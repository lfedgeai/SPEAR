# mic-device Feature: Local Microphone Capture (mic_fd)

## Overview

`mic-device` is an optional build feature that switches `mic_fd` from the default `stub` frame generator to **real microphone capture**.

In the current implementation, if `mic_ctl` does not explicitly set `source`, it defaults to `source=device` and uses the default `fallback.to_stub=true` behavior:

- with `mic-device` enabled and a usable/authorized device: real capture
- without `mic-device`, or when the device is unavailable / permission is denied: automatically falls back to `stub`

- Without `mic-device`, `mic_ctl` with `source=device` follows the fallback behavior (controlled by `fallback.to_stub`).
- With `mic-device`, `source=device` opens a system input device, produces PCM frames, and exposes them via `mic_read` with `epoll` readiness (`EPOLLIN`).

Code references:

- `mic_ctl/mic_read`: `src/spearlet/execution/host_api/mic/mod.rs`
- device capture implementation: `src/spearlet/execution/host_api/mic/source_device.rs`

## Enable the feature

Build via Makefile with features:

```bash
make FEATURES=mic-device build
```

## mic_ctl parameters (device mode)

Pass JSON to `MIC_CTL_SET_PARAM` (cmd=1):

```json
{
  "source": "device",
  "fallback": { "to_stub": false },
  "sample_rate_hz": 24000,
  "channels": 1,
  "format": "pcm16",
  "frame_ms": 20,
  "max_queue_bytes": 524288
}
```

Fields:

- `source`: `"device"` selects real input capture
- `fallback.to_stub`: whether to fall back to `stub` when device/permission fails
- `sample_rate_hz/channels/format/frame_ms`: output frame format returned by `mic_read`
- `max_queue_bytes`: internal queue cap; on overflow it drops oldest frames (drop_oldest)

Optional device selection:

```json
{
  "source": "device",
  "device": { "name": "MacBook Pro Microphone" },
  "fallback": { "to_stub": false },
  "sample_rate_hz": 24000,
  "channels": 1,
  "format": "pcm16",
  "frame_ms": 20
}
```

## Platform & permissions

### macOS

- The first access triggers a microphone permission prompt.
- If permission is denied or device open fails:
  - with `fallback.to_stub=false`: `mic_ctl` returns `-EIO` and the `mic_fd` will expose `EPOLLERR`.
  - with `fallback.to_stub=true`: it automatically falls back to `stub`.

### Linux / Windows

- Depends on OS audio backends and device availability.
- In CI or headless environments, prefer `stub` mode or keep the default build without `mic-device`.

## Behavior & limitations

- Current device path supports `format=pcm16` output only.
- Input is mixed down to mono (channel average) and resampled to `sample_rate_hz` using linear interpolation.
- A `generation` guard is used so that repeated `mic_ctl` restarts won’t leave stale background tasks producing frames.

## Tests

- There is a device capture test that validates `mic_fd` can produce real PCM16 frames; it only compiles with `mic-device`.
- If the environment has no microphone device or permission, the test returns early (no failure).
- You can control behavior via env vars:
  - `SPEAR_TEST_SKIP_MIC_DEVICE=1`: force skip
  - `SPEAR_TEST_REQUIRE_MIC_DEVICE=1`: require mic availability (fail otherwise)

```bash
make FEATURES=mic-device test
```

## How to confirm the microphone is actually used

Two practical options:

1) Run the device capture test and require mic availability:

```bash
SPEAR_TEST_REQUIRE_MIC_DEVICE=1 make FEATURES=mic-device test
```

Note: Rust tests capture stdout by default, so `make test` usually won’t show prints. To see output, run:

```bash
SPEAR_TEST_REQUIRE_MIC_DEVICE=1 cargo test --features mic-device test_mic_device_returns_pcm16_frames -- --nocapture
```

Or run only this test to avoid unrelated output:

```bash
SPEAR_TEST_REQUIRE_MIC_DEVICE=1 make test-mic-device
```

2) Run the probe example (prints default input device, lists inputs, and reads one frame):

```bash
cargo run --features mic-device --example mic_device_probe
```

Optional: select device by name

```bash
SPEAR_MIC_DEVICE_NAME='MacBook Pro Microphone' cargo run --features mic-device --example mic_device_probe
```
