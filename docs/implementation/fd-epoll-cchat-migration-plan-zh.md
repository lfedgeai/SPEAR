# 实施计划：通用 fd/epoll 子系统落地 + cchat 迁移（允许破坏性变更）

本文是“下一次开始改代码”的唯一实施入口文档，目标是把必要 context、决策点、分阶段步骤、受影响文件与验收测试一次性写清楚，便于在新 session 中直接进入实现。

## 0. 关联设计文档（先读）

- 通用子系统工程化设计（字段级结构/并发/测试/迁移）：
  - [fd-epoll-subsystem-zh.md](file:///Users/bytedance/Documents/GitHub/bge/spear/docs/api/spear-hostcall/fd-epoll-subsystem-zh.md)
  - [fd-epoll-subsystem-en.md](file:///Users/bytedance/Documents/GitHub/bge/spear/docs/api/spear-hostcall/fd-epoll-subsystem-en.md)
- Realtime ASR 特化（在通用子系统之上）：
  - [realtime-asr-epoll-zh.md](file:///Users/bytedance/Documents/GitHub/bge/spear/docs/api/spear-hostcall/realtime-asr-epoll-zh.md)
  - [realtime-asr-epoll-en.md](file:///Users/bytedance/Documents/GitHub/bge/spear/docs/api/spear-hostcall/realtime-asr-epoll-en.md)
- cchat hostcall 文档（已声明可破坏性变更与同步更新要求）：
  - [chat-completion-zh.md](file:///Users/bytedance/Documents/GitHub/bge/spear/docs/api/spear-hostcall/chat-completion-zh.md)
  - [chat-completion-en.md](file:///Users/bytedance/Documents/GitHub/bge/spear/docs/api/spear-hostcall/chat-completion-en.md)

## 1. 当前仓库关键事实（context 快照）

### 1.1 现有 cchat hostcalls（Rust）

- hostcall glue：
  - [wasm_hostcalls.rs](file:///Users/bytedance/Documents/GitHub/bge/spear/src/spearlet/execution/runtime/wasm_hostcalls.rs)
- cchat 状态机在 host_api 里是私有 map（`ChatHostState.sessions/responses`），并未进入统一 fd table：
  - [host_api.rs](file:///Users/bytedance/Documents/GitHub/bge/spear/src/spearlet/execution/host_api.rs)
- 现有错误码为固定 `-1..-5`，并且 C SDK 也写死了这些常量。

### 1.2 现有测试与样例

- WAT 链接测试导入 cchat：
  - [wasm.rs](file:///Users/bytedance/Documents/GitHub/bge/spear/src/spearlet/execution/runtime/wasm.rs)
- C SDK header：
  - [spear.h](file:///Users/bytedance/Documents/GitHub/bge/spear/sdk/c/include/spear.h)
- wasm-c chat sample：
  - [chat_completion.c](file:///Users/bytedance/Documents/GitHub/bge/spear/samples/wasm-c/chat_completion.c)

### 1.3 破坏性变更允许范围

本次实施允许对 `cchat_*` 做破坏性变更（签名/错误码/语义），但必须同步更新：

- 文档（chat-completion-zh/en）
- 测试（WAT link 测试 + Rust 单测 + E2E 如有）
- SDK 与 samples（C header + wasm-c sample）

## 2. 一次性决策（实现前必须先定）

以下决策写入代码与测试后，将成为 ABI 现实：

### 2.1 错误码体系

- **统一采用 `-errno`**（推荐）：`-EAGAIN/-ENOSPC/-EBADF/...`
- 旧 `-1..-5` 固定码：
  - 可以直接废弃（破坏性变更），或
  - 在 C header 中提供兼容宏（不推荐，增加维护成本）

### 2.2 cchat_send 语义

两个实现路线：

- 路线 A（先稳）：send 同步产出 response bytes，response fd 立即 `EPOLLIN`
- 路线 B（后强）：send 立即返回 response fd，后台异步产出，ready 后 `EPOLLIN`

建议从 A 开始，先把 fd/epoll 的“通用基座”打通，后续再升级到 B。

### 2.3 epoll 模式

- v1 固定 **level-triggered**
- `ep_wait` 输出固定 packed records（8 bytes/条：fd i32 + events i32）
- 返回集合：同 fd 去重、events OR 合并、按 fd 升序排序

## 3. 目录与模块落地位置（工程组织）

推荐新增目录（可从最小可行开始，后续再拆）：

- `src/spearlet/execution/hostcall/`
  - `mod.rs`
  - `fd_table.rs`
  - `epoll.rs`
  - `ctl.rs`
  - `errno.rs`
  - `memory.rs`（如果要复用 wasm_hostcalls 里的 mem_read/mem_write）

第一阶段也可先落在 `host_api.rs` 内部结构，等跑通后再做模块拆分。

## 4. 分阶段实施步骤（按可合并 PR 切片）

### Phase 1：落地通用 FdTable 与 EpollState（不碰 cchat 外部 ABI）

目标：host 侧存在统一 fd 表与 epoll，并能被后续 hostcall glue 调用。

实施要点：

- 定义 `FdKind/FdFlags/PollEvents`（至少支持 O_NONBLOCK 与 IN/OUT/ERR/HUP）
- 实现 `FdTable`：alloc/get/close + watchers 管理
- 实现 `EpollState`：watch set + notify + ep_wait 扫描算法

验收标准：

- rust 单测：构造两个 dummy fd（可临时内置），验证 ep_wait 去重、排序、timeout、ENOSPC 写回

### Phase 2：新增 hostcall ABI（spear_epoll_* + spear_fd_ctl）

目标：WASM guest 可导入并调用 epoll 与通用 fd 控制。

改动文件：

- [wasm_hostcalls.rs](file:///Users/bytedance/Documents/GitHub/bge/spear/src/spearlet/execution/runtime/wasm_hostcalls.rs)
  - 新增导出：`spear_epoll_create/ctl/wait/close`
  - 新增导出：`spear_fd_ctl`
  - 在 `build_spear_import*()` 中注册符号

- [wasm.rs](file:///Users/bytedance/Documents/GitHub/bge/spear/src/spearlet/execution/runtime/wasm.rs)
  - 新增 WAT 链接测试导入 `spear_epoll_*` 与 `spear_fd_ctl`

验收标准：

- cargo test 通过
- WAT link tests 通过（wasmedge feature 下）

### Phase 3：迁移 cchat 到 FdTable（允许破坏性变更）

目标：把 chat 的 session/response 资源从私有 map 迁移到通用 fd table，并让 response fd 可被 epoll 监听。

实施要点：

- 定义 `FdKind::ChatSession` 与 `FdKind::ChatResponse`
- `cchat_create`：alloc ChatSession fd
- `cchat_send`：alloc ChatResponse fd
  - 路线 A：同步产出 bytes
  - 写入 bytes 后：使 poll_mask 包含 IN，并 notify watchers
- `cchat_recv`：从 ChatResponse fd 读 bytes
- `cchat_close`：走通用 close，使 poll_mask 包含 HUP，并 notify

如果采用 `-errno`：

- 统一把参数错误/无效 fd/缓冲区不足映射到 `-EINVAL/-EBADF/-ENOSPC`

验收标准：

- 新增 Rust 单测：
  - `cchat_send` 后 response fd 被 epoll 返回 `EPOLLIN`
  - close 后 epoll 返回 `EPOLLHUP`

### Phase 4：同步更新 C SDK 与 wasm-c sample（必须）

目标：破坏性变更后的 ABI 在 C header 与 sample 里可编译/可运行。

受影响文件：

- [spear.h](file:///Users/bytedance/Documents/GitHub/bge/spear/sdk/c/include/spear.h)
  - 更新错误码：改为 `-errno`（至少要改 ENOSPC/EAGAIN/EBADF 的判断）
  - 新增 `spear_epoll_*` 与 `spear_fd_ctl` 的 import（可选，但建议补上）
  - 更新 `sp_cchat_recv_alloc`：由判断 `-3` 改为判断 `-ENOSPC`

- [chat_completion.c](file:///Users/bytedance/Documents/GitHub/bge/spear/samples/wasm-c/chat_completion.c)
  - 若 sample 使用了旧错误码或旧 ctl 语义，必须同步改

验收标准：

- sample 可编译（仓库若无现成构建脚本，至少保证代码与 header 一致）

### Phase 5：补齐测试矩阵（WAT + Rust 单测 + E2E）

目标：所有破坏性变更被测试覆盖，避免未来回归。

受影响文件：

- [wasm.rs](file:///Users/bytedance/Documents/GitHub/bge/spear/src/spearlet/execution/runtime/wasm.rs)
  - 更新 cchat 导入测试（如果符号/签名变了）
- [host_api.rs](file:///Users/bytedance/Documents/GitHub/bge/spear/src/spearlet/execution/host_api.rs)
  - 更新现有 cchat pipeline 单测
- `tests/wasm_openai_e2e_tests.rs`（如依赖特定语义）

验收标准：

- `cargo test` 全绿

### Phase 6：RtAsrFd 雏形（可并行/后续）

目标：rtasr fd 进入 fd table，并支持 IN/OUT/ERR/HUP readiness，先用 stub 事件源验证，再接 WebSocket。

---

## 5. 关键实现细节（避免踩坑）

### 5.1 锁顺序（必须遵守）

固定顺序：

1. FdTable 查找 entry 句柄（不要在此持久持锁）
2. EpFd watch set 锁
3. Entry 内部锁（队列/状态机）

禁止反向获取，避免死锁。

### 5.2 readiness 触发点（必须 notify）

- recv_queue 空 -> 非空：notify IN
- send_queue 满 -> 非满：notify OUT
- error：notify ERR
- close/对端关闭：notify HUP

### 5.3 ep_wait 的稳定性要求

- 同一次返回中同 fd 只能出现一次
- events 必须 OR 合并
- 返回顺序按 fd 升序（稳定便于测试）
- 允许 spurious wakeup：唤醒后无就绪继续等待

### 5.4 缓冲区不足（ENOSPC）

- 所有“写入 out buffer”的 hostcall（ep_wait、recv/status/metrics）必须支持：
  - capacity 不足写回需要长度
  - 返回 `-ENOSPC`

---

## 6. 建议的命令与验收

建议在每个 phase 完成后执行：

- `cargo test`
- `cargo fmt --check`
- `cargo clippy -- -D warnings`

---

版本：implementation plan v1（2026-01）
