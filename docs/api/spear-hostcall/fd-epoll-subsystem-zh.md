# Spear Hostcall 工程化设计：通用 fd/epoll 子系统（v1）

## 0. 本文定位

本文定义 Spear WASM hostcall 的通用 I/O 基座：

- 统一的 **fd（file descriptor）表**（类型、flags、生命周期、资源回收）
- 统一的 **epoll 风格就绪通知**（多路复用、唤醒、level-triggered 合同）
- 可复用的 **fd 控制接口**（nonblock、status、kind、metrics）
- 工程化落地的 **Rust 数据结构/并发模型/迁移步骤/测试清单**

Realtime ASR 只是该子系统上的一个具体 fd 类型（`RtAsrFd`）。ASR 特有协议与参数在 [realtime-asr-epoll-zh.md](./realtime-asr-epoll-zh.md) 描述。

---

## 1. 术语

- **guest**：WASM 模块
- **host**：SPEARlet/Runtime
- **fd**：guest 可见的 i32 句柄
- **epfd**：epoll 实例 fd
- **readiness**：fd 当前是否“可读/可写/错误/挂起”
- **level-triggered**：只要条件成立，ep_wait 每次都报告就绪

---

## 2. 设计目标（工程化）

### 2.1 必须满足

- 所有可 poll 的资源统一进入 fd table；epoll 不允许“旁路监听”。
- `ep_wait` 可被唤醒（Notify/Condvar），不允许 busy loop 作为核心机制。
- readiness 语义对 guest 可预测：固定去重、固定排序、固定 level-triggered。
- close/取消必须唤醒 `ep_wait`，并能通过 `EPOLLHUP|EPOLLERR` 感知。
- 支持增量迁移：旧 family API（例如 `cchat_*`）不要求立即破坏性变更。

#### 2.1.1 允许破坏性变更（针对 cchat 等历史 API）

为实现“通用 fd/epoll 子系统”一致性，允许对现有 `cchat_*` API 做 **破坏性变更**（例如：错误码体系、控制接口、甚至函数签名/命名）。

约束：

- 任何破坏性变更必须同步更新：
  - 单元/集成测试
  - SDK（如 C header）与 sample 代码
  - 对应 API 文档

本文后续章节会给出迁移清单与受影响文件列表。

### 2.2 明确不做（v1）

- 不实现 edge-triggered/oneshot。
- 不实现完整 WASI socket/filesystem。
- 不要求 guest 使用特定 async runtime；syscall 组合足够即可。

---

## 3. Guest 可见 ABI（hostcalls）

### 3.1 `spear_epoll_*`（epoll）

#### 3.1.1 函数签名

- `spear_epoll_create() -> i32`
- `spear_epoll_ctl(epfd: i32, op: i32, fd: i32, events: i32) -> i32`
- `spear_epoll_wait(epfd: i32, out_ptr: i32, out_len_ptr: i32, timeout_ms: i32) -> i32`
- `spear_epoll_close(epfd: i32) -> i32`

#### 3.1.2 常量

`epoll_ctl` 操作：

- `EPOLL_CTL_ADD = 1`
- `EPOLL_CTL_MOD = 2`
- `EPOLL_CTL_DEL = 3`

事件位：

- `EPOLLIN  = 0x001`
- `EPOLLOUT = 0x004`
- `EPOLLERR = 0x008`
- `EPOLLHUP = 0x010`

#### 3.1.3 out buffer 布局（稳定 ABI）

为避免 guest/host struct 对齐差异，`spear_epoll_wait` 输出为紧凑 little-endian 数组。

每条记录固定 8 bytes：

- `fd: i32`（4 bytes）
- `events: i32`（4 bytes）

因此 `need = n * 8`。

缓冲区不足：

- `max_records = floor(capacity_bytes / 8)`
- 若 `max_records == 0`：
  - 写回 `8` 到 `*out_len_ptr`
  - 返回 `-ENOSPC`
- 否则写入 `min(ready_count, max_records)` 条，并写回实际写入字节数。

#### 3.1.4 wait 行为

- `timeout_ms < 0`：无限等待
- `timeout_ms == 0`：立即返回（轮询）
- `timeout_ms > 0`：最多等待 timeout

返回：

- `n >= 0`：表示写入了 `n` 条记录
- 超时：返回 `0`
- 错误：返回 `-errno`

唤醒条件：

- watch set 中任意 fd readiness 可能从“未就绪”变为“就绪”
- fd/epfd 被 close/cancel

### 3.2 `spear_fd_ctl`（通用 fd 控制）

#### 3.2.1 背景

`cchat_ctl/rtasr_ctl/...` 都会重复实现 nonblock、status、metrics 等通用能力。工程化上应提供统一入口，family 可在其 `*_ctl` 内部复用。

#### 3.2.2 函数签名

- `spear_fd_ctl(fd: i32, cmd: i32, arg_ptr: i32, arg_len_ptr: i32) -> i32`

#### 3.2.3 通用 cmd（v1 建议）

- `FD_CTL_SET_FLAGS = 1`：设置 flags（JSON）
- `FD_CTL_GET_FLAGS = 2`：读取 flags（JSON）
- `FD_CTL_GET_KIND  = 3`：读取 fd kind（JSON 或直接 i32，建议 JSON 以便扩展）
- `FD_CTL_GET_STATUS = 4`：读取通用状态（JSON）
- `FD_CTL_GET_METRICS = 5`：读取通用统计（JSON）
- `FD_CTL_CLOSE = 6`：通用 close（长期替代多个 `*_close`，v1 可不实现）

flags JSON（示例）：

```json
{ "set": ["O_NONBLOCK"], "clear": [] }
```

status JSON（示例）：

```json
{
  "kind": "RtAsrFd",
  "flags": ["O_NONBLOCK"],
  "poll_mask": ["EPOLLIN"],
  "closed": false
}
```

### 3.3 errno 体系（新子系统统一用 -errno）

建议使用 libc errno 的负值：

- `-EBADF`、`-EINVAL`、`-EAGAIN`、`-ENOSPC`、`-ENOTCONN`、`-EPIPE`、`-ETIMEDOUT`、`-EINTR`、`-ENOMEM`

兼容策略：旧 `cchat_*` 可保留 `-1..-5`，但一旦其 fd 纳入 fd table 并参与 epoll，就应在内部转换为 `-errno` 语义（至少在 epoll/通用接口层）。

---

## 4. Host 侧工程化架构（Rust）

### 4.1 目录建议

（可分阶段迁移，不要求一次到位）

- `src/spearlet/execution/hostcall/`
  - `fd_table.rs`：fd allocator + fd table + close
  - `epoll.rs`：epfd + watch set + wait/wakeup
  - `ctl.rs`：`spear_fd_ctl` 通用实现
  - `errno.rs`：errno 与 buffer-too-small 约定
  - `memory.rs`：线性内存读写工具（与 wasm_hostcalls glue 共享）
- `src/spearlet/execution/stream/`
  - `rtasr.rs`：`RtAsrFd` 具体实现（依赖 fd_table/epoll）

### 4.2 数据结构（字段级）

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
    // ...
}
```

约束：

- `watchers` 必须去重。
- `closed` 为 true 后：
  - `poll_mask` 至少包含 `HUP`
  - 所有 I/O 返回 `-EBADF` 或 family 规定的错误/EOF（需统一策略）。

#### 4.2.5 `FdTable`

```rust
pub struct FdTable {
    next_fd: i32,
    entries: std::collections::HashMap<i32, std::sync::Arc<std::sync::Mutex<FdEntry>>>,
}
```

接口建议：

- `alloc(kind, flags, inner) -> fd`
- `get(fd) -> Arc<Mutex<FdEntry>>`
- `close(fd) -> Result<(), errno>`
- `register_watcher(fd, epfd)` / `unregister_watcher(fd, epfd)`

#### 4.2.6 `Pollable`（实现面约束）

Rust 层可以用 match 分发而非 trait object，但语义需等价：

```rust
pub trait Pollable {
    fn poll_mask(&self) -> PollEvents;
}
```

### 4.3 并发与锁（工程约束）

#### 4.3.1 锁顺序（必须固定）

1. 只读获取 `FdTable` 中 entry handle（不持久持锁）
2. `EpFd` watch set 锁
3. 具体 fd 内部锁（队列、状态机等）

禁止反向获取。

#### 4.3.2 唤醒规则（必须实现）

当 `poll_mask()` 可能从“未包含某位”变为“包含某位”时：

- 遍历 `watchers` 集合，对每个 epfd 触发 notify。

典型触发点：

- recv_queue：空 -> 非空（IN）
- send_queue：满 -> 非满（OUT）
- 错误发生（ERR）
- close/对端关闭（HUP）

#### 4.3.3 spurious wakeup

允许 wake 后无就绪（竞争条件）。ep_wait 必须循环“扫描->等待->扫描”。

### 4.4 `epoll` 实现（字段级）

#### 4.4.1 `EpollState`

```rust
pub struct EpollState {
    pub watch: std::collections::HashMap<i32, PollEvents>,
    pub notify: tokio::sync::Notify,
}
```

#### 4.4.2 `ep_ctl`

- `ADD`：
  - 校验 fd 存在、可被监听（kind 允许）
  - 写入 watch set
  - fd.entry.watchers 插入 epfd
- `DEL`：
  - 从 watch set 删除
  - fd.entry.watchers 删除 epfd
- `MOD`：
  - 更新 watch set

#### 4.4.3 `ep_wait` 算法（伪代码）

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
  if ready 非空 { 写入 out_buf; return n }
  if timeout==0 { return 0 }
  await notify with timeout
}
```

时间源：使用单调时钟。

---

## 5. 新 fd 类型接入指南（工程流程）

要让一个新资源（例如对象存储流 fd）可被 epoll 使用，必须实现：

1. 资源进入 `FdTable`，确定 `FdKind`
2. 定义 `poll_mask()`：何时 IN/OUT/ERR/HUP
3. 在 readiness 触发点调用 `notify_watchers()`
4. 读写接口返回 `-EAGAIN` 的条件必须与 `EPOLLIN/OUT` 合同一致
5. close 必须：
   - 将 `closed=true`
   - `poll_mask` 包含 HUP
   - notify watchers

---

## 6. 迁移方案：将现有 `cchat_*` 纳入通用 fd/epoll

### 6.1 目标

迁移目标分两档（可择其一）：

- **目标 A（兼容优先）**：`cchat_*` 对外 ABI 尽量不变，仅内部纳入 `FdTable` 以支持 epoll。
- **目标 B（工程优先，允许破坏性变更）**：将 `cchat_*` 收敛到通用 fd/errno/ctl 体系，必要时修改函数签名/错误码/命令。

无论选择 A 或 B，都必须保证：

- chat 的“response fd 可 poll（EPOLLIN/HUP/ERR）”
- close 能唤醒 `ep_wait`

### 6.2 映射规则

- `cchat_create`：alloc `ChatSession` entry，返回 fd
- `cchat_send`：alloc `ChatResponse` entry，后台/同步写入 bytes 后置 `EPOLLIN` 并 notify
- `cchat_recv`：从 `ChatResponse` entry 取 bytes 输出
- `cchat_close`：统一走 `FdTable.close`（或保持旧 close 但内部调用）

### 6.4 允许的破坏性变更范围（建议）

以下变更被认为是“合理且推荐”的破坏性调整（用于对齐通用子系统）：

1. **错误码统一为 `-errno`**
   - 现有 `-1..-5` 可废弃或仅作为兼容层
   - C SDK 与 sample 需同步更新（见 6.5）

2. **引入/强制使用 `spear_fd_ctl` 的通用命令**
   - 例如把 `cchat_ctl` 的部分命令迁移到 `spear_fd_ctl(FD_CTL_SET_FLAGS/GET_STATUS/GET_METRICS)`

3. **统一 close 入口（长期）**
   - 允许将 `cchat_close` 退化为对 `spear_fd_ctl(FD_CTL_CLOSE)` 的包装，或直接废弃 `cchat_close`

4. **send/recv 语义变更以支持异步/streaming**
   - 允许将 `cchat_send` 变为“立即返回 response fd + 后台产出响应”的异步语义
   - 允许将 `cchat_recv` 变为“对 response fd 的通用 read”式语义（例如未来的 `spear_fd_read`），从而与 rtasr 的 read 模式对齐

### 6.5 破坏性变更的受影响文件（必须同步更新）

当对 `cchat_*` 做破坏性变更时，至少需要同步更新以下位置：

- 文档：
  - `docs/api/spear-hostcall/chat-completion-zh.md`
  - `docs/api/spear-hostcall/chat-completion-en.md`
- C SDK 与样例：
  - `sdk/c/include/spear.h`
  - `samples/wasm-c/chat_completion.c`
- Rust 代码中的链接/行为测试：
  - `src/spearlet/execution/runtime/wasm.rs`（WAT 导入符号测试）
  - `src/spearlet/execution/host_api.rs`（cchat pipeline 单测）
  - `tests/wasm_openai_e2e_tests.rs`（如依赖具体符号/语义）

建议新增一类“ABI 版本测试”：

- 通过 `spear_fd_ctl(FD_CTL_GET_KIND/GET_FLAGS)` 或新增 `spear_abi_version()` 机制，在测试中验证 guest/host ABI 匹配。

### 6.3 分步改造清单（面向代码）

以当前仓库结构为参考：

- 第 1 步：新增 `FdTable`（可先放 `host_api.rs` 内部）
- 第 2 步：新增 `EpollState` 与 `spear_epoll_*` hostcalls glue
- 第 3 步：将 `rtasr_fd` 作为首个 Pollable fd
- 第 4 步：迁移 `ChatResponse` 到 `FdTable`，并把“response ready”纳入 `EPOLLIN`
- 第 5 步：迁移 `ChatSession` 到 `FdTable`
- 第 6 步：提供 `spear_fd_ctl`，并让 family `*_ctl` 复用通用实现

验收标准：

- `ep_wait` 能同时监听 `rtasr_fd` 与 `cchat_send` 返回的 response fd
- close 能触发 `EPOLLHUP` 并唤醒等待

---

## 7. 测试清单（必须具备）

### 7.1 ABI 链接测试

- WAT 导入 `spear_epoll_*` 与 `spear_fd_ctl` 符号，确保链接

### 7.2 epoll 语义测试

- 空 watch set：wait 超时返回 0
- 有就绪 fd：返回记录去重、按 fd 升序
- 缓冲区不足：写回需要长度并返回 `-ENOSPC`
- close 唤醒：wait 立即返回并带 HUP

### 7.3 cchat 集成测试

- `cchat_send` 后 response fd 进入 `EPOLLIN`
- `cchat_close` 后 response fd 进入 `EPOLLHUP`

---

版本：engineering v1（2026-01）
