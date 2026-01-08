# Spear Hostcall 提案：Realtime ASR（rtasr_fd）在通用 fd/epoll 子系统之上

本文件聚焦 **Realtime ASR（`rtasr_*`）** 的特有设计。通用 fd/epoll 子系统的工程化实现、数据结构与迁移方案见：

- [fd-epoll-subsystem-zh.md](./fd-epoll-subsystem-zh.md)

## 1. 背景与问题

当前 WASM hostcall 已采用 syscall-like 抽象与 fd（file descriptor）模型（例如 `cchat_*`），适合 request/response 型交互。但 realtime ASR 属于全双工流式场景：

- 输入侧：持续接收用户语音并发送音频 chunk
- 输出侧：持续接收 ASR delta/completed/error 等事件

这两条数据流需要“同时推进”。在 WASM guest 侧通常是单线程（或受限线程模型），因此需要通过 POSIX 风格的 **非阻塞 I/O + 就绪通知**（poll/epoll）来进行多路复用，而不是依赖真实并行线程。

legacy Go 实现中（`legacy/spearlet/stream/rt_asr.go` + `legacy/spearlet/stream/backend/openai.go`）使用 WebSocket 与后台 goroutine 接收事件。Rust/wasm 侧需要一个等价的 hostcall 方案，将 host 的异步 WebSocket 事件桥接成 guest 可用的 fd 语义。

## 2. 目标与非目标

### 2.1 目标

- 为 realtime ASR 提供 **全双工** fd 抽象：写音频、读事件。
- 在 WASM guest 侧以 **POSIX 风格**实现 async：
  - 非阻塞 `read/write` 返回 `-EAGAIN`
  - 通过 `epoll` 风格接口等待就绪，避免 busy loop
- 支持背压（host 侧发送队列满时令 guest 感知 `EAGAIN`/`POLLOUT`）。
- 兼容现有 `cchat_*` 的 ABI 习惯：指针/长度、输出缓冲区不足返回、fd 生命周期。
- 安全边界明确：密钥、URL、网络访问策略由 host 决策，WASM 只提交能力需求与参数。

#### 2.1.1 通用化目标（epoll 面向所有 hostcall fd）

本提案中的 epoll 机制不应仅服务 realtime ASR。它应成为 Spear WASM hostcall 的通用“就绪通知层”，支撑后续所有需要异步/并发推进的能力：

- 流式 AI（realtime voice、streaming chat、RAG streaming）
- 受控网络 I/O（未来可能的受限 socket/HTTP stream fd）
- 受控文件/对象存储流（upload/download streaming）
- 运行时/任务侧事件订阅（例如 task log/event stream）

因此，本次落地需要对 fd 的类型体系、生命周期与 readiness 计算做较大整理，避免每个 family 自造一套“可 poll”的逻辑。

### 2.2 非目标

- 不引入完整 WASI sockets/filesystem 兼容层。
- 不在第一版中支持多连接/多路 websocket 复用的复杂传输层（先聚焦 realtime ASR）。
- 不要求 guest 侧必须使用 Rust async runtime；提供最小的 syscall 组合，guest 可自行封装。

## 3. 设计原则（POSIX 对齐）

1. **fd 一切**：guest 仅持有 i32 fd，不直接持有指针到 host 内部对象。
2. **非阻塞语义**：当读不到/写不进时返回 `-EAGAIN`，由 guest 决定等待策略。
3. **一次注册、反复等待**：提供 epoll 风格，避免 `poll(pollfd[])` 每次重组 fd 数组。
4. **内存拷贝最少但可控**：沿用现有 hostcall 的 out_ptr/out_len_ptr 模式；输出缓冲区不足返回 `-ENOSPC` 或 `-3`（见 6.4）。
5. **可观测与可调试**：提供 `GET_STATUS/GET_METRICS` 类控制命令读取状态与统计。

6. **依赖通用子系统**：`rtasr_fd` 作为 `FdKind::RtAsr` 的一个实现，readiness/close/wakeup/flags 遵循通用约定。

## 4. 总体架构

### 4.1 数据流

- guest 调用 `rtasr_write(fd, pcm16_chunk)`：
  - host 将音频 chunk 入队到发送队列
  - host 后台任务从发送队列取出并发送 websocket message

- host 后台任务从 websocket 读取事件（text/binary）：
  - 写入接收队列（event queue）
  - 驱动 readiness：`POLLIN` 就绪

- guest 调用 `rtasr_read(fd, out)`：
  - 从接收队列弹出一条完整事件（UTF-8 JSON bytes）
  - 队列为空返回 `-EAGAIN`

### 4.2 并发模型

- host 侧：每个 realtime ASR session 至少一个后台任务（或任务组）实现 websocket 双向收发。
- guest 侧：单线程事件循环 + `ep_wait` 等待就绪即可达到“同时收音频/收 ASR”。

### 4.3 readiness 模型

- `POLLIN`：接收队列非空，`rtasr_read` 将返回 >=0
- `POLLOUT`：发送队列未满，`rtasr_write` 将返回 >=0
- `POLLERR`：连接错误
- `POLLHUP`：对端关闭或 session 已关闭


## 4. 依赖：通用 fd/epoll 子系统

`rtasr_*` 依赖通用子系统提供：fd table、epoll、fd flags、统一 close/wakeup、以及（可选的）`spear_fd_ctl`。

工程化细节不在本文重复，请直接阅读：

- [fd-epoll-subsystem-zh.md](./fd-epoll-subsystem-zh.md)

### 4.4 会话状态机（rtasr_fd）

为保证 guest/host 行为可预期，rtasr_fd 需要一个明确的状态机。建议状态如下：

- `INIT`：`rtasr_create` 后，尚未配置/连接
- `CONFIGURED`：已设置至少一个参数（可选）
- `CONNECTING`：`RTASR_CTL_CONNECT` 已触发，正在创建 session / 建连
- `CONNECTED`：WebSocket 已建立，后台任务运行
- `DRAINING`：已半关闭写端（不再接受音频写入），仍可读事件直到完成
- `CLOSED`：`rtasr_close` 已调用或对端关闭
- `ERROR`：发生不可恢复错误（可读 `GET_STATUS.last_error`）

状态迁移规则（摘要）：

- `INIT/CONFIGURED -> CONNECTING -> CONNECTED`
- `CONNECTED -> DRAINING`（例如 `RTASR_CTL_SHUTDOWN_WRITE`）
- `CONNECTED/DRAINING -> CLOSED`（close 或对端关闭）
- 任意状态遇到不可恢复错误可进入 `ERROR`，随后 `rtasr_close` 进入 `CLOSED`

### 4.5 队列、背压与公平性

rtasr_fd 维护两条队列：

- 发送队列：guest 写入的音频 chunk（bytes）
- 接收队列：host 从 WebSocket 收到的事件帧（bytes，建议 UTF-8 JSON）

背压策略（建议）：

- 发送队列以 `max_send_queue_bytes` 限制总字节数
- 当 `send_queue_bytes + incoming_len > max_send_queue_bytes`：
  - nonblock：`rtasr_write` 返回 `-EAGAIN`
  - block（若未来支持）：阻塞等待直到队列有空间

接收队列溢出策略（必须明确）：

- 以 `max_recv_queue_bytes` 限制总字节数
- 溢出时默认行为建议为：丢弃最旧事件并递增 `dropped_events`，同时在状态中记录一次性告警（避免无限内存增长）

事件公平性：

- `rtasr_read` 每次只返回一条事件，guest 可循环读取直到 `-EAGAIN`
- host 后台收发应避免“只发不收”或“只收不发”导致饥饿，推荐 `select!`/轮询式公平

## 5. 现有架构需要的修改

本提案以最小侵入方式落地在当前结构：

- `src/spearlet/execution/host_api.rs`
  - 增加 Realtime ASR host state（fd 表、发送/接收队列、状态机、统计）
  - 增加 epoll host state（epfd 表、watch set、wait 阻塞唤醒）
  - 为 host 异步任务提供可复用的 tokio runtime（避免每次 hostcall new runtime）

- `src/spearlet/execution/runtime/wasm_hostcalls.rs`
  - 新增 `rtasr_*` 导出函数
  - 新增 `spear_epoll_*` 导出函数
  - 新增通用 errno/返回值约定（见 6）

- `Cargo.toml`
  - 如果要直连 WebSocket，需要引入 websocket 客户端依赖（例如 `tokio-tungstenite` + `url`）
  - 如果第一阶段仅做抽象/模拟，可暂不加依赖

### 5.1 建议的模块拆分（在不破坏现有结构的前提下）

为避免 `host_api.rs` 持续膨胀，建议逐步拆分，但第一阶段可以先落在现有文件中：

- `src/spearlet/execution/hostcall/memory.rs`
  - 线性内存读写工具（复用/抽象 `mem_read/mem_write/mem_write_with_len`）
- `src/spearlet/execution/hostcall/errno.rs`
  - errno 与返回值规范工具（统一 `-errno`、`ENOSPC` 写回长度等）
- `src/spearlet/execution/hostcall/epoll.rs`
  - epfd/epoll 状态表与 `ep_wait` 唤醒机制
- `src/spearlet/execution/stream/rtasr.rs`
  - rtasr_fd 的状态机、队列、后台任务（ws connect/send/recv）

第一阶段最小改动落地路径：

- 继续在 `host_api.rs` 内新增 `RtAsrHostState` 与 `EpollHostState`
- 在 `wasm_hostcalls.rs` 内新增 `rtasr_*` 与 `spear_epoll_*` 的 ABI glue

### 5.4 fd 整理的迁移路径（建议分阶段）

由于 epoll 将服务于所有 hostcalls，fd 整理建议分阶段落地，控制风险：

#### 阶段 A：引入通用 fd table 与 epoll，但只接入 rtasr

- 新增 `FdTable` 与 `Epoll`，`rtasr_fd` 作为第一个 `Pollable` 实现
- `cchat_*` 暂不迁移，但预留 `ChatResponseFd` 接入口

#### 阶段 B：把 cchat 的 response fd 接入 epoll

- 将 `ChatHostState.responses` 迁移为 `ChatResponseFd` Entry
- 定义 `ChatResponseFd` readiness：
  - 有 bytes 可读：`EPOLLIN`
  - close：`EPOLLHUP`

#### 阶段 C：逐步把所有“有队列/有数据可读写”的 hostcall fd 接入

- 对象存储读写流 fd
- streaming chat / realtime voice 等

#### 阶段 D：收敛 family API（可选、长期）

- 从 `cchat_close/rtasr_close/...` 收敛到统一 `spear_fd_close`
- 从 family 各自 `*_ctl` 收敛到统一 `spear_fd_ctl`（或保留 family 但底层统一）

### 5.2 host 侧唤醒机制（ep_wait 如何被唤醒）

必须明确 epoll 等待如何被唤醒，否则 guest 会陷入忙等。

推荐实现：

- 每个 `epfd` 维护一个 `Notify/Condvar`（或等价机制）
- 当任意 watch fd 的 readiness 可能变化时（例如 recv_queue 从空变非空、send_queue 从满变非满、连接进入 ERROR/HUP），调用 `notify_one/notify_waiters`
- `spear_epoll_wait`：
  - 先扫一遍 watch set 计算就绪集合
  - 若为空且 timeout 允许等待：进入 `Notify::notified().await` 或 condvar wait（由 host runtime 执行）
  - 被唤醒后重新扫描

### 5.3 WASM 实例与资源回收

在 runtime 里需要考虑：

- 实例结束时自动 close 所有 fd（至少 close rtasr_fd/epfd），避免后台任务泄漏
- close 必须幂等：重复 close 返回 0 或 `-EBADF`（需在文档明确）
- close 会触发 `ep_wait` 立即返回（`-EINTR` 或返回带 `EPOLLHUP` 的事件）

### 5.5 cchat fd 纳入 epoll 的详细映射（建议）

为实现“epoll 通用化”，建议对现有 `cchat_*` 做以下映射（对外 API 不变）：

#### 5.5.1 fd 类型

- `cchat_create` 返回：`ChatSessionFd`
- `cchat_send` 返回：`ChatResponseFd`

#### 5.5.2 ChatResponseFd 的 readiness

- `EPOLLIN`：response bytes 已写入并可被 `cchat_recv` 读取
- `EPOLLHUP`：`cchat_close(resp_fd)` 之后
- `EPOLLERR`：host 内部错误（建议配合 `cchat_ctl(GET_STATUS/GET_METRICS)` 或未来通用 `FD_CTL_GET_STATUS` 查看）

#### 5.5.3 ChatSessionFd 的 readiness（可选）

当前 chat session 是“写入请求、send 触发、recv 读取 response fd”的两阶段模型，因此 `ChatSessionFd` 本身不一定需要可 poll。

若未来支持 streaming chat（增量 token 输出），建议将输出放在 `ChatResponseFd` 上，使 `ChatResponseFd` 成为真正可读的流：

- 有新的增量输出：`EPOLLIN`
- 输出完成：可通过事件帧 `completed` 或 status 字段体现

#### 5.5.4 可选：为 chat 增加“真正异步”的 send

当前 `cchat_send` 是同步计算并直接写入 response。若未来 chat 也要支持 streaming/async，可引入：

- `cchat_send_async(fd) -> response_fd`：立即返回 response_fd，后台任务生成响应
- 当 response 可读时，将 `ChatResponseFd` 置为 `EPOLLIN` 并唤醒 epoll

这样 guest 可用同一套事件循环同时处理 chat 与 rtasr。

## 6. ABI 与错误码规范

### 6.1 基本约定

- 所有 hostcall 使用 WASM 友好类型：`i32/i64`、指针用 `i32` 表示线性内存偏移。
- 成功返回：
  - `create`：返回 `fd > 0`
  - `read`：返回实际写入字节数（`>= 0`）
  - `write`：返回消费的输入字节数（`>= 0`）
  - `ctl`：返回 `0` 或读取写入的字节数
- 失败返回：负值，优先使用 `-errno` 语义（例如 `-11` 表示 `EAGAIN`）。

### 6.2 推荐 errno 子集

- `-EBADF`：fd 无效或类型不匹配
- `-EINVAL`：参数错误（cmd 不支持、JSON 无效、flags 无效）
- `-EAGAIN`：非阻塞下读不到/写不进
- `-ENOSPC`：输出缓冲区不足（也可延用 `-3` 并写回需要长度）
- `-ENOTCONN`：未连接就读写
- `-ECONNRESET/-ECONNABORTED`：连接被重置/中止
- `-ETIMEDOUT`：等待/连接超时
- `-EINTR`：等待被取消（close/cancel 导致）

### 6.3 输出缓冲区约定

沿用当前 `cchat_recv` 的模式：

- `out_ptr`：输出缓冲区起始
- `out_len_ptr`：指向 u32，入参为最大长度，出参写回实际长度或需要长度

推荐行为：

- 若 `max_len < need`：
  - 写回 `need` 到 `*out_len_ptr`
  - 返回 `-ENOSPC`（或兼容现有 `-3`）

### 6.4 与现有 cchat 错误码兼容策略

当前 `cchat_*` 使用 `-1..-5` 固定码（invalid fd/ptr/buf-too-small/invalid-cmd/internal）。为了减少破坏性变更：

- 新增的 `rtasr_*` 与 `ep_*` 推荐直接采用 `-errno`。
- 旧的 `cchat_*` 保持原有返回值。
- 在文档中明确：不同 family 的错误码体系可能不同；长期可迁移 `cchat_*` 到 `-errno`。

## 7. fd 类型与生命周期

### 7.1 fd 类型

- `rtasr_fd`：Realtime ASR session fd
- `epfd`：epoll 实例 fd

host 侧需要一个全局 fd allocator，为不同资源类型分配不冲突的 fd，并在 fd 表中记录类型。

### 7.2 生命周期

- `*_create` 分配 fd 并初始化状态。
- `*_close` 释放 fd：
  - 取消后台任务
  - 关闭 websocket
  - 唤醒所有等待该 fd/epfd 的 `ep_wait`（返回 `-EINTR` 或带 `POLLHUP`）

## 8. Realtime ASR Hostcall 设计（rtasr_*）

### 8.1 函数签名

- `rtasr_create() -> i32`
- `rtasr_ctl(fd: i32, cmd: i32, arg_ptr: i32, arg_len_ptr: i32) -> i32`
- `rtasr_write(fd: i32, buf_ptr: i32, buf_len: i32) -> i32`
- `rtasr_read(fd: i32, out_ptr: i32, out_len_ptr: i32) -> i32`
- `rtasr_close(fd: i32) -> i32`

### 8.1.1 I/O 与控制的“最小完备集合”

为了让 guest 可以实现全双工与 async 封装，最小完备集合为：

- `create/close`：资源生命周期
- `ctl(SET_PARAM, CONNECT, GET_STATUS)`：配置与状态
- `read/write`：数据面
- 与之配套的 `spear_epoll_*`：就绪等待

不建议在 v1 设计成“阻塞 read/write + 线程并行”，因为在单线程 wasm 内不可移植。

### 8.2 rtasr_ctl 命令

#### 8.2.1 `RTASR_CTL_SET_PARAM = 1`

- 入参：`arg_ptr/arg_len_ptr` 指向 JSON（UTF-8）
- JSON 结构：

```json
{ "key": "input_audio_transcription.model", "value": "gpt-4o-mini-transcribe" }
```

- 行为：写入 session 的参数 map（不触网）。

推荐支持的 key（第一版）：

- `input_audio_format`: `"pcm16" | ...`
- `input_audio_transcription.model`: string
- `turn_detection.*`: object
- `vad.*`: object
- `max_send_queue_bytes`: number
- `max_recv_queue_bytes`: number
- `nonblock`: boolean（等价于 `FIONBIO/O_NONBLOCK`）

#### 8.2.1.1 参数层级与默认值（建议）

rtasr 会话参数建议采用“扁平 key + JSON value”的方式，避免 ABI 频繁变更。

推荐默认值：

- `input_audio_format = "pcm16"`
- `input_sample_rate_hz = 24000`
- `input_channels = 1`
- `max_send_queue_bytes = 1024 * 1024`
- `max_recv_queue_bytes = 1024 * 1024`
- `nonblock = true`

推荐额外 key（第一版可选，但建议预留）：

- `backend = "openai_chat_completion" | "openai_realtime_ws" | ...`（仅选择后端名称，不传 URL/secret）
- `model = "gpt-4o-mini-transcribe" | ...`（等价于 `input_audio_transcription.model` 的便捷写法）
- `connect_timeout_ms = 10000`
- `idle_timeout_ms = 60000`（无数据时的空闲超时策略）
- `drop_policy = "drop_oldest" | "drop_newest" | "error"`（队列溢出策略）

#### 8.2.2 `RTASR_CTL_CONNECT = 2`

- 行为：
  - 根据当前参数创建 transcription session（例如 OpenAI `POST /v1/realtime/transcription_sessions`）
  - 使用 `client_secret` 建立 WebSocket（例如 `wss://.../v1/realtime?intent=transcription`）
  - 启动后台任务：双向收发 + 队列 + readiness
- 返回：0 或 `-errno`

#### 8.2.2.1 OpenAI realtime 对齐（参考）

若采用与 legacy 相同的 OpenAI realtime transcription：

- 创建 session：`POST /v1/realtime/transcription_sessions`
- 建立 WS：`wss://api.openai.com/v1/realtime?intent=transcription`
- Header：
  - `Authorization: Bearer <client_secret>`
  - `OpenAI-Beta: realtime=v1`

host 应负责：

- 从 host 配置读取 API key / base_url
- 将 guest 的参数（model、turn_detection 等）映射到 session config
- 返回给 guest 的事件帧尽量透传（便于快速落地）

#### 8.2.3 `RTASR_CTL_GET_STATUS = 3`

- 出参：`arg_ptr/arg_len_ptr` 写入 JSON

建议字段：

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

#### 8.2.4 `RTASR_CTL_SHUTDOWN_WRITE = 4`（建议新增）

目的：支持“结束输入但继续接收输出”的半关闭语义（对齐 `shutdown(SHUT_WR)`）。

- 行为：
  - rtasr_fd 进入 `DRAINING`
  - 后续 `rtasr_write` 返回 `-EPIPE` 或 `-ENOTCONN`（需统一）
  - host 可向 backend 发送 commit/finish（如果协议需要）

#### 8.2.5 `RTASR_CTL_GET_METRICS = 5`（建议新增）

目的：读取 session 统计（吞吐、队列、丢包、时延估计）。

- 出参 JSON（建议字段）：

```json
{
  "audio_bytes_sent": 123456,
  "events_received": 42,
  "dropped_events": 0,
  "connect_rtt_ms": 120,
  "last_event_time_ms": 1730000000000
}
```

### 8.3 rtasr_write 语义（发送音频）

- 输入：原始音频 bytes（推荐 pcm16 little-endian，sample rate 由参数约定）
- 行为：
  - 非阻塞：若发送队列满，返回 `-EAGAIN`
  - 否则入队并返回 `buf_len`

#### 8.3.1 音频帧边界与切片建议

host 不应强制要求固定 chunk 大小，但为降低端到端延迟建议：

- 20ms 或 40ms 一帧（pcm16 24kHz mono：20ms≈960 samples≈1920 bytes）
- guest 侧以固定帧长写入更利于 VAD/turn detection

如果 guest 写入过大 chunk，host 可以：

- 原样入队（实现简单）
- 或在 host 内部切片为更小帧（实现复杂，v1 不建议）

#### 8.3.2 write 的返回值约定

- 成功：返回“已消费的输入字节数”
- 失败：返回 `-errno`

建议 v1 只实现“全量成功或失败”，不实现部分写（避免 guest 处理复杂度）。

host 后台任务负责将音频编码为 websocket 协议要求（例如 base64）并发送 `input_audio_buffer.append`。

### 8.4 rtasr_read 语义（接收事件）

- 输出：一条“完整事件”的 JSON bytes（UTF-8）
- 行为：
  - 队列非空：弹出并写入 out buffer，返回写入长度
  - 队列为空：
    - 若 nonblock：返回 `-EAGAIN`
    - 若 block：内部等待直到有事件或超时/关闭

第一版推荐只实现 nonblock + `ep_wait` 等待，避免在单个 read 内长时间阻塞。

#### 8.4.1 事件边界

`rtasr_read` 每次返回一条“完整事件帧”。完整性定义：

- WebSocket 文本帧：一帧即一条事件（UTF-8 JSON）
- WebSocket 二进制帧：按 backend 约定（v1 建议直接透传 bytes）

host 不应在 v1 中将多帧拼接为更大的“语句级事件”，以免引入协议语义差异。

#### 8.4.2 读尽策略（guest 端）

推荐模式：

- 当 `ep_wait` 返回包含 `EPOLLIN` 时，循环调用 `rtasr_read` 直到返回 `-EAGAIN`
- 这样可以减少 `ep_wait` 的频率并降低上下文切换

### 8.5 事件格式

建议直接透传 backend 的 websocket event（例如 OpenAI realtime 事件），guest 只需要解析 `type` 字段：

- `conversation.item.input_audio_transcription.delta`
- `conversation.item.input_audio_transcription.completed`
- `error`
- `input_audio_buffer.*`

host 不应在第一版强制重写协议为自定义事件，以减少适配成本。

#### 8.5.1 最小推荐事件解析字段

为保证跨 backend 的最低兼容性，guest 至少解析：

- `type: string`
- `event_id: string`（如果存在）
- `error.*`（如果存在）

对 OpenAI realtime transcription，常用字段示例：

- delta：`delta: string`
- completed：`transcript: string`

## 9. epoll Hostcall 设计（spear_epoll_*）

### 9.1 函数签名

- `spear_epoll_create() -> i32`
- `spear_epoll_ctl(epfd: i32, op: i32, fd: i32, events: i32) -> i32`
- `spear_epoll_wait(epfd: i32, out_ptr: i32, out_len_ptr: i32, timeout_ms: i32) -> i32`
- `spear_epoll_close(epfd: i32) -> i32`

### 9.2 常量

#### 9.2.1 epoll_ctl 操作

- `EPOLL_CTL_ADD = 1`
- `EPOLL_CTL_MOD = 2`
- `EPOLL_CTL_DEL = 3`

#### 9.2.2 事件位

- `EPOLLIN  = 0x001`
- `EPOLLOUT = 0x004`
- `EPOLLERR = 0x008`
- `EPOLLHUP = 0x010`

#### 9.2.3 事件位的语义约束

- `EPOLLIN`：调用对应 fd 的 read/recv 将返回 >=0（不保证一次读空）
- `EPOLLOUT`：调用对应 fd 的 write/send 将返回 >=0（不保证永久可写）
- `EPOLLERR`：错误状态已发生，建议配合 `GET_STATUS` 查询细节
- `EPOLLHUP`：对端关闭或本地 close，fd 不再可用

epoll 本身只做“就绪通知”，不替代 fd 的读写接口。

### 9.3 out events 缓冲区布局

为避免 guest/host 对齐差异，`spear_epoll_wait` 返回一个紧凑数组（little-endian）：

每条记录 8 bytes：

- `fd: i32`（4 bytes）
- `events: i32`（4 bytes）

因此 `need = n * 8`。

#### 9.3.1 缓冲区不足时的处理（必须明确）

`spear_epoll_wait` 的输出条数取决于“当前就绪 fd 数量”和“输出 buffer 容量”。建议语义：

- `max_records = floor(capacity_bytes / 8)`
- 若 `max_records == 0`：
  - 写回 `8` 到 `*out_len_ptr`
  - 返回 `-ENOSPC`
- 否则写入 `min(ready_count, max_records)` 条，并写回实际写入字节数

### 9.4 wait 行为

- `timeout_ms < 0`：无限等待
- `timeout_ms == 0`：立即返回（轮询）
- `timeout_ms > 0`：最多等待 timeout

唤醒条件：

- watch set 中任意 fd readiness 从“不可用”转为“可用”
- fd/epfd 被 close/cancel

返回：

- `n >= 0`：写入 n 条 event 记录，返回 `n`
- 超时：返回 `0`
- 错误：返回 `-errno`

### 9.5 触发模式：level-triggered（v1 固定）

为降低实现复杂度，v1 固定为 level-triggered：

- 只要 fd 处于就绪条件（例如 recv_queue 非空），每次 `ep_wait` 都会返回该 fd 的 `EPOLLIN`
- guest 负责把队列读到空（直到 `-EAGAIN`）以清除就绪条件

不在 v1 中支持 edge-triggered、oneshot 等高级语义。

### 9.6 epoll 内部算法约束（实现细节必须固定）

为了让 guest 可预测地使用 epoll，需要固定以下行为：

#### 9.6.1 返回集合去重

- 同一次 `spear_epoll_wait` 返回的记录中，同一个 fd 至多出现一次
- 若同时满足多个事件位（例如 `EPOLLIN|EPOLLERR`），应合并到同一条记录的 `events` 字段中

#### 9.6.2 返回顺序

建议固定为：按 `fd` 升序排序（稳定且便于测试与复现）。

#### 9.6.3 多个 epfd 监听同一 fd

- 同一 fd 可以被多个 epfd 监听
- readiness 变化时应唤醒所有相关 epfd 的 waiters

#### 9.6.4 “唤醒但无就绪”允许存在

由于并发竞争，可能出现：ep_wait 被唤醒后重新扫描发现没有就绪事件（例如事件被其他线程消费）。

- 允许这种情况
- `ep_wait` 应继续等待直到 timeout 或出现就绪

#### 9.6.5 timeout 与时间源

- timeout 应使用单调时钟（monotonic），避免 wall clock 跳变
- timeout 到期返回 0

### 9.7 规模与性能约束（建议）

- 单个 epfd watch 的 fd 数量建议设置上限（例如 1024/4096），超出返回 `-ENOMEM` 或 `-EINVAL`
- watcher 集合与 notify 机制需避免 O(N^2)（fd 数量大时会放大）

## 10. guest 侧并发推进（参考用法）

### 10.1 单 fd（只关心 ASR 输出）

- `epfd = spear_epoll_create()`
- `spear_epoll_ctl(epfd, ADD, rtasr_fd, EPOLLIN|EPOLLERR|EPOLLHUP)`
- 循环 `spear_epoll_wait`，就绪后 `rtasr_read` 读到 `-EAGAIN` 为止

### 10.2 双 fd（麦克风输入 + ASR 输出同时推进）

前提：麦克风同样抽象为 `mic_fd`（host 提供），并定义 `mic_read`：

- watch `mic_fd: EPOLLIN`
- watch `rtasr_fd: EPOLLIN|EPOLLOUT|EPOLLERR|EPOLLHUP`
- `EPOLLIN(mic)`：读音频 chunk 并 `rtasr_write`
- `EPOLLIN(rtasr)`：读事件并解析 delta
- `EPOLLOUT(rtasr)`：继续 flush 本地缓存的音频 chunk（背压解除）

### 10.3 参考伪代码（面向 POSIX 封装）

下面的模式展示了“同时推进”的本质：一个事件循环里根据就绪事件推进不同 fd。

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
    // -EINTR/-ETIMEDOUT/-...
    continue;
  }
  for (int i = 0; i < n; i++) {
    int fd = *(int*)(evbuf + i * 8 + 0);
    int revents = *(int*)(evbuf + i * 8 + 4);

    if (fd == mic_fd && (revents & EPOLLIN)) {
      int r = mic_read(mic_fd, pcm_buf, pcm_cap);
      if (r > 0) {
        int w = rtasr_write(rtasr_fd, (int)pcm_buf, r);
        // w == -EAGAIN: 本地缓存这段音频，等待 EPOLLOUT
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

## 11. 安全与策略

- backend URL、API key、client secret 必须由 host 管理；WASM 不应读取真实 secret。
- host 需要 enforce：
  - allowlist（可访问域名/路径）
  - 连接数、session 时长上限
  - send/recv 队列上限（防止 guest OOM host）
- guest 只允许提交 model/turn_detection 等参数，且需受 host policy 约束。

### 11.1 host 配置与 guest 参数的映射（建议）

建议在 host 配置中提供：

- `rtasr.backends[]`：可用后端列表（name、kind、base_url、transport、capabilities）
- `rtasr.default_backend`：默认后端
- `rtasr.allow_models[]`：允许的 model 列表（或正则/前缀）
- `rtasr.max_sessions`、`rtasr.max_session_seconds`
- `rtasr.max_send_queue_bytes`、`rtasr.max_recv_queue_bytes`

guest 的 `SET_PARAM` 只能在这些上限内收缩或选择，不能扩大 host policy。

### 11.2 资源与计费相关字段

host 若可从 backend 得到 usage/计费信息，建议通过 `GET_METRICS` 或事件透传提供给 guest；guest 不应自行拼装/伪造。

## 12. 可测试性与回归

- 链接测试：WAT 导入 `rtasr_*` 与 `spear_epoll_*`，确保符号可用。
- 语义测试：
  - `rtasr_read` 空队列返回 `-EAGAIN`
  - `spear_epoll_wait` 在写入事件后能唤醒
- 端到端测试（可选）：在具备 key 的环境下跑 websocket 实连；否则使用 stub backend 模拟事件产生。

## 13. 兼容性与演进

- 本提案不要求修改现有 `cchat_*`。
- 长期可将 `cchat_*` 的错误码从固定码迁移到 `-errno` 体系，并提供兼容层。
- 未来可扩展 fd 类型：
  - `rtvoice_fd`（realtime TTS/voice）
  - `net_fd`（受控网络 socket）
  - `file_fd`（受控文件/对象存储流）

## 14. 实现清单（面向落地）

### 14.1 必须实现（v1）

- `rtasr_create/ctl(CONNECT, SET_PARAM, GET_STATUS)/read/write/close`
- `spear_epoll_create/ctl/wait/close`
- send/recv 队列 + 背压 + readiness 计算
- close/cancel 唤醒 `ep_wait`

### 14.2 推荐实现（v1.1）

- `RTASR_CTL_SHUTDOWN_WRITE`（半关闭）
- `RTASR_CTL_GET_METRICS`（统计）
- stub backend：在无真实 key 环境下模拟 delta/completed 事件，跑 e2e

---

版本：proposal v0（2026-01）
