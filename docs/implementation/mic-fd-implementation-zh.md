# Mic 采集（mic_fd）实现文档（面向落地）

本文是对 `mic_*` 这一组 WASM hostcalls 的工程落地版设计文档，目标是在当前仓库中把 **真实麦克风采集** 以 **fd + epoll** 的方式实现，并与现有 `rtasr_fd` 无缝对接。

文档风格与 `realtime-asr-implementation-zh.md` 一致，偏实现细节与函数/方法级别说明。

## 0. 关联文档与代码入口

### 0.1 规范/实现入口

- Realtime ASR 实现文档：`docs/implementation/realtime-asr-implementation-zh.md`
- fd/epoll 子系统：`docs/api/spear-hostcall/fd-epoll-subsystem-zh.md`

### 0.2 现有代码入口（已存在）

- `mic_*` host API（当前实现）：`src/spearlet/execution/host_api/mic/mod.rs`
- `MicState/MicConfig`：`src/spearlet/execution/hostcall/types.rs`
- fd table 与 epoll：`src/spearlet/execution/hostcall/fd_table.rs`
- WASM hostcalls glue：`src/spearlet/execution/runtime/wasm_hostcalls.rs`

### 0.3 当前现状（必须明确）

当前 `mic_fd` 的数据来自 stub task（定时生成伪造帧），不是真实设备音频。

本文设计将 stub 升级为可插拔的输入源，并提供真实设备采集的实现路径（macOS/Windows/Linux）。

## 1. 目标、边界与可测性

### 1.1 目标

- 保持对 guest 暴露的 ABI 语义稳定：`create/ctl/read/close` + epoll readiness。
- 支持真实麦克风采集：从 OS 设备读取 PCM 数据，按 `frame_ms` 切片推入 `MicState.queue`。
- 支持背压：guest 读不及导致队列满时按策略丢帧并计数。
- 保持 CI 可跑：默认不依赖真实麦克风权限（stub/file mode 可覆盖）。

### 1.2 非目标（v1 不要求）

- 不在 WASM guest 内做系统级音频权限申请与 UI。
- 不提供“系统声音回环采集”（loopback）。
- 不提供复杂 DSP（AEC/AGC/NS），仅做必要格式转换与重采样。

## 2. 对外 ABI（WASM hostcalls）

### 2.1 hostcalls 签名

- `mic_create() -> i32`
- `mic_ctl(fd: i32, cmd: i32, arg_ptr: i32, arg_len_ptr: i32) -> i32`
- `mic_read(fd: i32, out_ptr: i32, out_len_ptr: i32) -> i32`
- `mic_close(fd: i32) -> i32`

`mic_ctl` 的行为与 `rtasr_ctl` 一样：输入是 JSON bytes（UTF-8），输出在同一块 buffer 中回写。

### 2.2 readiness 语义（必须可预测）

对 `mic_fd`：

- `EPOLLIN`：`MicState.queue` 非空（`mic_read` 可读）
- `EPOLLERR`：`MicState.last_error` 非空（设备异常、权限拒绝等）
- `EPOLLHUP`：`mic_close` 调用或采集停止

### 2.3 mic_read 语义

- 一次 `mic_read` 返回一帧音频 bytes（长度由 `frame_ms/sample_rate/channels/format` 决定；允许最后一帧不足）。
- 队列为空返回 `-EAGAIN`。
- 输出 buffer 不足返回 `-ENOSPC` 并回写需要长度。

## 3. 控制面：mic_ctl 命令

### 3.1 命令编号（与当前实现保持一致）

- `MIC_CTL_SET_PARAM = 1`

v1 暂不新增更多 cmd；如未来需要列设备等，可扩展：

- `MIC_CTL_LIST_DEVICES = 2`（建议）
- `MIC_CTL_GET_STATUS = 3`（建议）

### 3.2 MIC_CTL_SET_PARAM JSON（推荐 schema）

保持兼容当前字段：

```json
{
  "sample_rate_hz": 24000,
  "channels": 1,
  "format": "pcm16",
  "frame_ms": 20
}
```

扩展字段（设计新增）：

```json
{
  "stub_pcm16_base64": "...",
  "source": "device",
  "device": {
    "name": "MacBook Pro Microphone"
  },
  "sample_rate_hz": 24000,
  "channels": 1,
  "format": "pcm16",
  "frame_ms": 20,
  "max_queue_bytes": 524288,
  "drop_policy": "drop_oldest",
  "fallback": {
    "to_stub": true
  }
}
```

字段说明：

- `source`: `"device" | "file" | "stub"`
  - `device`: 真实麦克风
  - `file`: 文件回放（用于测试/回归；建议仅在 host 侧允许）
  - `stub`: 现有伪造帧
- `device.name`: 选择设备（可选，空则用系统默认输入设备）
- `sample_rate_hz/channels/format/frame_ms`: mic_fd 对外输出格式
- `max_queue_bytes`: 覆盖 `MicState.max_queue_bytes`
- `drop_policy`: 队列满时策略（v1 固定 `drop_oldest`，保留字段便于演进）
- `fallback.to_stub`: 设备不可用/无权限时是否自动降级到 stub（默认 true 以保证可用性）
- `stub_pcm16_base64`: 仅对 `source = "stub"` 生效，base64 编码的原始 PCM16 bytes；stub 将循环回放这些 bytes 产出每帧数据

返回值：

- 成功：返回 `0`
- 失败：返回 `-errno`（见第 8 节错误码）

## 4. 数据结构与状态（hostcall/types.rs）

### 4.1 现有结构

当前：

- `MicConfig { sample_rate_hz, channels, frame_ms, format }`
- `MicState { config, queue, queue_bytes, max_queue_bytes, dropped_frames, last_error, running }`

### 4.2 v1 建议扩展（仅设计）

在 `MicState` 增加“输入源句柄与线程控制”字段：

- `source_kind: MicSourceKind`（device/file/stub）
- `source_handle: Option<MicSourceHandle>`（用于 stop/join）
- `capture_status: MicCaptureStatus`（Init/Running/Stopped/Error）
- `last_overflow_at: Option<Instant>`（用于诊断）

注意：fd table 当前用 `Mutex<FdEntry>`，所以 source handle 必须是可 Send + 可安全释放的对象。

## 5. 模块拆分（建议）

避免 `host_api.rs` 继续膨胀，建议新增模块：

- `src/spearlet/execution/audio/mod.rs`
  - `mic.rs`: MicSource 抽象、实现选择、配置解析
  - `convert.rs`: 格式转换、混音、重采样

以及最小公共 trait：

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

实现建议：

- `StubMicSource`: 现有逻辑
- `FileMicSource`: 从文件按 frame_ms 切帧输出（测试）
- `DeviceMicSource`: 使用跨平台音频库采集（建议 `cpal`）

## 6. 线程模型与数据流（关键）

真实麦克风采集通常由 OS 回调线程驱动，不能在回调里做阻塞/重锁。

推荐两阶段流水线：

1) **采集线程（回调）**：把原始 samples 写入无锁 ring buffer（或轻锁队列），尽量只 memcpy。
2) **组帧线程（host 后台任务）**：
   - 从 ring buffer 拉取 samples
   - 做必要转换：sample 格式、声道混合、重采样
   - 按 `frame_ms` 切片为 `Vec<u8>`
   - 入队 `MicState.queue`
   - 更新 poll_mask，必要时 `notify_watchers(fd)`

关键点：

- `MicState.queue` 与 `rtasr_fd.send_queue` 的背压不同：mic 是“生产者持续产出”，实时性优先，队列满时应 **drop_oldest**。
- 每次入队后如果 `poll_mask` 从无 `IN` 变为有 `IN`，必须 notify。

## 7. 代码落地：函数/方法级别设计

本节按当前 `DefaultHostApi` 的函数组织方式设计。

### 7.1 mic_create

目标：分配一个 `mic_fd`。

- 分配 `FdEntry { kind: Mic, inner: MicState::default() }`
- 初始 `poll_mask = EMPTY`

### 7.2 mic_ctl(fd, MIC_CTL_SET_PARAM, payload)

目标：设置配置，并启动/重启采集。

流程（建议实现顺序）：

1) 解析 JSON：
   - 若不含 `source`，默认 `device`，但可由 `fallback.to_stub` 决定降级行为
   - 填充 `MicConfig`
   - 更新 `max_queue_bytes`
2) 拿 fd lock：
   - 校验 fd kind
   - 更新 `MicState.config`
   - 如 `running == false`：启动采集（spawn）
   - 如 `running == true` 且配置变化：先 stop 再 start（v1 可简单实现为 stop+start）
3) 启动采集：
   - `spawn_mic_device_task(fd, cfg)` 或 `spawn_mic_file_task` 或 `spawn_mic_stub_task`
4) 更新 poll_mask（如果已有数据），notify watchers

错误处理：

- 解析失败：`-EINVAL`
- 设备不可用：
  - 若 `fallback.to_stub=true`：降级 stub 并返回 0
  - 否则：写入 `last_error` 并返回 `-EIO`（同时置 `EPOLLERR`）

### 7.3 mic_read(fd)

目标：从队列弹出一帧。

- pop 一帧 bytes
- 更新 `queue_bytes`
- recompute poll_mask（队列空则清除 `IN`）
- 必要时 notify watchers

### 7.4 mic_close(fd)

目标：停止采集并关闭 fd。

- 关闭前停止 source（device/file/stub task）
- 标记 fd closed
- `poll_mask` 加 `HUP`
- notify watchers

### 7.5 spawn_mic_device_task(fd, cfg)（新增）

目标：真实麦克风采集生产者。

建议拆为两层：

- `spawn_mic_device_capture_thread(...)`：
  - 打开设备
  - 注册回调，将 samples 写入 ring buffer
- `spawn_mic_frame_pump_task(fd, cfg, ring_buffer)`：
  - 以 `frame_ms` 为节拍组帧
  - 转换为 `MicFrame` bytes
  - push 到 `MicState.queue`，并处理溢出

## 8. 错误码与可观测性

建议保持与其它 hostcalls 一致：

- `-EBADF`：fd 不存在或类型不匹配
- `-EINVAL`：JSON 不合法或字段值不支持
- `-EAGAIN`：read 队列为空（仅 `mic_read`）
- `-EIO`：设备打开/运行异常
- `-ENOSPC`：输出 buffer 不足（WASM glue 层）

状态/诊断建议：

- `MicState.last_error`: 存储人类可读错误（不要包含敏感信息）
- `MicState.dropped_frames`: 统计队列满导致丢帧数量

## 9. 测试策略（不依赖真实麦克风）

### 9.1 单元测试

- 继续保留 stub 测试（验证 fd/epoll 语义）
- 增加 file mode 测试（确定性）：
  - 输入一段固定 pcm 数据，按 `frame_ms` 切片
  - 断言 `mic_read` 返回长度与帧数符合预期
  - 断言溢出时 `dropped_frames` 增长

### 9.2 手工测试（可选/ignored）

- `device` 模式：
  - 运行一个 demo guest（或 host 侧直接调用 mic_*）
  - 说话并观察 `mic_read` 是否持续产生非零帧

## 10. 与 rtasr_fd 的组合用法（关键）

推荐在 guest 侧用 epoll 同时等待 `mic_fd(EPOLLIN)` 与 `rtasr_fd(EPOLLOUT/EPOLLIN)`：

- `mic_fd IN`：读取一帧 PCM，写入 `rtasr_write`
- `rtasr_fd OUT`：发送队列可写，flush pending 音频
- `rtasr_fd IN`：读事件 JSON

在 `server_vad` 分段策略下，guest 通常不需要主动 flush。
