# mic-device Feature：本机麦克风采集（mic_fd）

## 概述

`mic-device` 是一个可选编译特性，用于将 `mic_fd` 的数据源从默认的 `stub` 伪造帧切换为“真实麦克风采集”。

当前实现中，如果 `mic_ctl` 未显式传 `source`，则默认使用 `source=device`，并使用 `fallback.to_stub=true` 的默认值：

- 在启用 `mic-device` 且设备可用/有权限时：走真实采集
- 在未启用 `mic-device`、设备不可用或无权限时：自动回退到 `stub`

- 默认构建（不启用 `mic-device`）时，`mic_ctl` 的 `source=device` 会走降级逻辑（按 `fallback.to_stub` 决定是否回退到 `stub`）。
- 启用 `mic-device` 后，`source=device` 会尝试打开系统输入设备并持续产出 PCM 帧，通过 `mic_read` 读取，并可用 `epoll` 监听 `EPOLLIN`。

相关代码：

- `mic_ctl/mic_read`：`src/spearlet/execution/host_api/mic/mod.rs`
- 设备采集实现：`src/spearlet/execution/host_api/mic/source_device.rs`

## 如何启用

使用 Makefile 通过 features 构建：

```bash
make FEATURES=mic-device build
```

## mic_ctl 参数（device 模式）

对 `MIC_CTL_SET_PARAM`（cmd=1）传入 JSON：

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

字段说明：

- `source`: `"device"` 表示使用真实输入设备
- `fallback.to_stub`: 设备不可用/无权限时是否回退到 `stub`
- `sample_rate_hz/channels/format/frame_ms`: `mic_read` 的输出帧格式
- `max_queue_bytes`: `mic_fd` 内部队列上限，满时丢旧帧（drop_oldest）

指定设备（可选）：

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

## 平台与权限说明

### macOS

- 首次访问麦克风时，系统会弹窗请求权限。
- 如果权限被拒绝或设备打开失败：
  - 当 `fallback.to_stub=false`：`mic_ctl` 会返回错误（`-EIO`），并将 `mic_fd` 的 `EPOLLERR` 置位。
  - 当 `fallback.to_stub=true`：会自动回退到 `stub`。

### Linux / Windows

- 依赖系统音频后端与设备可用性。
- 如果运行在无音频设备/无权限环境（例如 CI），建议使用 `stub` 模式或保持默认构建不启用 `mic-device`。

## 行为与限制

- 当前 `device` 实现仅支持 `format=pcm16` 输出。
- 当前实现会将输入做单声道化（多声道输入时做均值混音），并按目标 `sample_rate_hz` 做线性重采样。
- 为避免 `mic_ctl` 多次调用导致旧任务继续产出，内部通过 `generation` 做“重启隔离”。

## 测试

- 仓库包含一个设备采集测试，用于验证 `mic_fd` 能读到真实 PCM16 帧；该测试仅在启用 `mic-device` 时编译。
- 若环境无麦克风设备或无权限，测试会自动返回（不失败）。
- 可通过环境变量控制行为：
  - `SPEAR_TEST_SKIP_MIC_DEVICE=1`：强制跳过
  - `SPEAR_TEST_REQUIRE_MIC_DEVICE=1`：强制要求可用（失败则报错）

```bash
make FEATURES=mic-device test
```

## 如何确认确实访问到了麦克风

推荐两种方式：

1) 运行 device 采集测试并强制要求设备可用：

```bash
SPEAR_TEST_REQUIRE_MIC_DEVICE=1 make FEATURES=mic-device test
```

说明：Rust 测试默认会捕获 stdout，所以即使测试内部有输出，`make test` 通常也不会显示。需要看输出时，用：

```bash
SPEAR_TEST_REQUIRE_MIC_DEVICE=1 cargo test --features mic-device test_mic_device_returns_pcm16_frames -- --nocapture
```

也可以只跑这个测试（减少其他测试输出干扰）：

```bash
SPEAR_TEST_REQUIRE_MIC_DEVICE=1 make test-mic-device
```

2) 运行探测示例（会打印默认输入设备、列出输入设备、并读一帧音频）：

```bash
cargo run --features mic-device --example mic_device_probe
```

可选：指定设备名

```bash
SPEAR_MIC_DEVICE_NAME='MacBook Pro Microphone' cargo run --features mic-device --example mic_device_probe
```

如果你的环境没有麦克风权限或设备不可用，该测试会失败；这是预期行为。
