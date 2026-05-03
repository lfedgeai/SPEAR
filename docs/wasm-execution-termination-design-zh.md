# WASM 执行终止守卫（Terminate / Destroy 语义）设计

## 背景

当前 WASM 运行时在一个专用 worker 线程上执行 guest，并通过 `spear` import object 暴露同步 hostcall。

- 运行时在进入 guest 前用 thread-local 的 `CURRENT_WASM_EXECUTION_ID` 设置执行上下文，退出后清理。对应实现位于 `src/spearlet/execution/host_api/core.rs`，并在 `src/spearlet/execution/runtime/wasm.rs` 的 invoke 流程中调用。
- 大多数 hostcall 使用“返回负 errno”的方式表达错误（`Result<Vec<WasmValue>, CoreError>` 通常仍返回 `Ok([i32])`）。

问题：当执行需要被取消/终止（超时、用户取消、实例停止）时，guest 可能正阻塞在某个 hostcall 内（例如 `epoll_wait`、`sleep_ms`、或耗时网络操作）。仅靠运行时侧的取消并不能可靠抢占 guest 线程，因此需要在 **每个 hostcall 的最开始**做一次协作式的“终止守卫”检查。

## 目标

- 在所有 WASM hostcall 起始处增加低开销、统一的“是否需要终止？”检查。
- 一旦被标记终止，hostcall 立即返回明确的错误，并阻止继续执行。
- 携带可读的终止消息（reason），用于排障/对用户提示。
- 同时支持两种语义：
  - Terminate execution（cancel/timeout）：结束本次执行，但允许该 instance 后续继续被复用执行新的请求。
  - Destroy instance：终止当前执行并释放/删除该 instance（后续任何执行都应被拒绝或立刻终止；如需继续服务应重建新 instance）。

## 非目标

- 抢占不调用 hostcall 的纯 CPU 循环。
- 在 hostcall 内部对任意阻塞系统调用做强行中断（超出当前协作式边界的能力）。

## 现有代码切入点

- hostcall 注册：`src/spearlet/execution/runtime/wasm_hostcalls.rs`（`build_spear_import_with_api`）。
- 执行上下文（thread-local execution id）：`src/spearlet/execution/host_api/core.rs`。
- WASM worker invoke loop：`src/spearlet/execution/runtime/wasm.rs`（调用 `vm.run_func*` 前后设置/清理 execution id）。

## 端到端流程（UI -> API -> 终止变量 -> 控制执行/实例）

本节补齐“从用户点击按钮开始，到终止变量生效，再到执行/实例被结束”的完整链路。

### A. 用户点击 “停止当前 execution（Cancel）”

#### A1. UI 调用哪个 API

本设计中，Terminate execution / Destroy instance 属于运维/管理能力，**不要求 SPEAR Console 提供对应操作**，由 Web Admin 或其他运维入口触发即可。

- **Web Admin（面向管理员）**：建议调用 SMS 的 admin API（token/cookie 鉴权）
  - `POST /admin/api/executions/{execution_id}/terminate`
  - body：`{ "reason": "admin terminated execution" }`

最终由 SMS 解析 `execution_id -> node_uuid -> spearlet http_addr`，然后请求 spearlet 执行 Terminate。

#### A2. SMS 如何把请求转到正确的 spearlet（Terminate execution，基于 protobuf/gRPC）

控制面不依赖 WebSocket，也不要求走 HTTP gateway；推荐完全走 protobuf/gRPC：

1) SMS 调 `ExecutionIndexService.GetExecution(execution_id)` 得到 `execution.node_uuid`。
2) SMS 调 `NodeService.GetNode(node_uuid)` 得到 `ip + grpc_port`（优先使用 node.port；必要时从 metadata 推导）。
3) SMS 作为 gRPC client 直连对应 spearlet 的 gRPC 地址 `http://{ip}:{grpc_port}`，调用：
   - `spearlet.ExecutionService.TerminateExecution(execution_id, reason)`

#### A3. spearlet 收到 Terminate 后如何落到“终止变量”

在 spearlet 内部实现 terminate handler / service：

1) 解析 `execution_id + reason`。
2) 查询 execution 状态，拿到 `instance_id`（从 `TaskExecutionManager.executions` 中获取；找不到返回 404；已结束返回 409）。
3) 写入 execution 级终止变量：
   - `exec_termination.mark(execution_id, -ECANCELED, reason)`（scope=Execution/Terminate）

#### A4. hostcall 侧如何生效

- WASM guest 下一次进入任意 hostcall，统一 guard 会先检查：
  - `exec_termination`（命中则立刻返回错误或触发 trap）
- 这样能保证“下一次 hostcall 边界”停止当前执行。

#### A5. 运行时如何把结果收敛成 Terminated

- `vm.run_func*` 因 trap 返回错误
- runtime 将其映射为 Terminated（建议为 `ExecutionError::Terminated{ scope=Execution, message=... }` 或等价语义）
- 更新 execution 状态并上报 completion event（SMS 的 execution_index 最终可见 terminated/timeout；timeout 也可视作一种 terminate 原因）

### B. 用户点击 “Destroy Instance”

Destroy instance 的语义是：该 instance 当前正在跑的 **所有 execution 必须先被 Terminate**，并且之后该 instance 不允许再被复用（资源释放/删除；需要服务则重建新 instance）。

#### B1. UI 调用哪个 API

建议只提供给管理员（Web Admin），因为它是破坏性操作（SPEAR Console 不要求提供入口）：

- `POST /admin/api/instances/{instance_id}/destroy`
- body：`{ "reason": "admin destroyed instance" }`

#### B2. SMS 如何路由到正确 spearlet（Destroy instance，基于 protobuf/gRPC）

- instance_id -> node_uuid：优先走 SMS 的 InstanceRegistry/ExecutionIndex（例如 GetInstance 或从 instance 最近 executions 反推 node_uuid）。
- node_uuid -> spearlet gRPC addr：同 A2 的 node 查询逻辑，得到 `ip + grpc_port`。
- SMS 作为 gRPC client 直连对应 spearlet，并调用：
  - `spearlet.InstanceService.DestroyInstance(instance_id, reason)`（需要新增 protobuf 定义；见后文“实施计划/接口定义”）

#### B3. spearlet 如何“先 Terminate 所有 execution”

在 spearlet 的 destroy-instance 处理里建议按固定顺序执行（并保证幂等）：

1) **冻结实例（阻止新执行进入）**
   - `instance_termination.mark(instance_id, -ECANCELED, "instance destroyed: {reason}")`（scope=Instance/Destroy）
2) **枚举并 Terminate 当前运行中的 executions（满足“先 Terminate 所有 execution”）**
   - 从 `TaskExecutionManager` 中筛出 `status=running && instance_id==...` 的 `execution_id` 列表
   - 对每个 execution：`exec_termination.mark(execution_id, -ECANCELED, "instance destroyed: {reason}")`
3) **Destroy 实例资源**
   - 调用 `runtime.stop_instance(instance)`（对外语义为 Destroy；内部实现可先 stop 再 remove）
4) **拒绝后续执行**
   - instance 已被 stop + instance_termination 仍然存在：即使外部误触发执行，也应在调度/执行入口直接拒绝或立刻取消

#### B4. 注意事项（重要）

- “在 hostcall 开头检查 terminate”是协作式边界：如果 guest 当前阻塞在某个 hostcall 内部（例如 `sleep_ms` 直接 `thread::sleep`），仅靠“入口检查”无法抢占，需要让这些阻塞点本身也周期性检查 termination 或改为可中断等待。
- 因此 Destroy instance 的稳策略是：
  - 先标记 instance + executions（保证下一次 hostcall 边界必停）
  - 同时 `runtime.stop_instance` 尽可能释放/关闭资源，促使 guest 尽快回到 hostcall 边界（例如 epoll/wait 因 HUP/ERR 醒来）

## 设计方案

### 1) 终止状态模型

为了同时支持 “Terminate execution” 与 “Destroy instance”，终止状态拆成两类 key：

- execution 级（key=`execution_id`）：只影响当前 execution。
- instance 级（key=`instance_id`）：影响该 instance 上的所有 execution（现在与未来）。

终止状态结构建议包含：

- `terminated: AtomicBool`
- `errno: i32`（推荐使用 `-libc::ECANCELED` 作为统一终止错误码）
- `message: Arc<parking_lot::Mutex<Option<String>>>`（终止原因）
- `scope: enum { Execution, Instance }`（用于区分 Terminate execution 与 Destroy instance）
- `ts_ms: u64`（可选，诊断用）

### 2) 全局注册表 + HostApi 字段

这里需要同时满足两点：

1) 运行时侧（cancel/timeout/stop 等路径）在没有 WASM VM 内部 `DefaultHostApi` 引用的情况下也能标记终止。
2) hostcall 侧检查终止要尽量解耦且开销低。

实现方式：提供两个全局注册表，并且在 `DefaultHostApi` 上显式持有字段（“把需要 terminate 的消息作为变量放在对应 field”）：

- `WASM_EXEC_TERMINATION_REGISTRY: OnceLock<DashMap<String, Arc<TerminationState>>>`（key 为 `execution_id`）
- `WASM_INSTANCE_TERMINATION_REGISTRY: OnceLock<DashMap<String, Arc<TerminationState>>>`（key 为 `instance_id`）
- `DefaultHostApi` 新增字段：
  - `exec_termination: Arc<WasmTerminationRegistry>`（execution 级）
  - `instance_termination: Arc<WasmTerminationRegistry>`（instance 级）

`WasmTerminationRegistry` 是薄封装：

- `mark(execution_id, errno, message)`
- `clear(execution_id)`（防止遗留状态影响下次执行）
- `check(execution_id) -> Option<TerminationSnapshot>`

### 3) Hostcall 统一守卫函数

在 `wasm_hostcalls.rs` 中实现一个统一 helper，并在每个 hostcall 最开始调用（检查顺序建议固定）：

- 通过 `current_wasm_execution_id()`（thread-local）获取当前执行 id
- 用 `host_data.exec_termination` 查询 execution 级终止状态
- 用 `host_data.instance_termination` 查询 instance 级终止状态（需要 `host_data.instance_id` 存在）
- 任一命中 `terminated == true`，立刻返回/触发 trap

语义约束：
- execution 级终止：只影响本次 execution，下一次新的 execution（新的 execution_id）默认不受影响。
- instance 级终止（Destroy instance）：从被标记开始，该 instance 的任何 hostcall 都必须立刻终止；运行时层面也应拒绝新的执行或直接返回终止状态。

返回策略（两层）：

- **方案 A（软终止）**：直接返回 `Ok([WasmValue::from_i32(-libc::ECANCELED)])` 并可选写日志
  - 优点：符合现有“返回 errno”的 hostcall 风格
  - 缺点：guest 可能忽略错误继续运行
- **方案 B（硬终止，推荐用于“终止执行/实例”）**：记录终止原因后返回 `Err(CoreError::Common(UserDefError))` 触发 trap
  - 优点：保证在下一次 hostcall 边界立刻停止 guest 执行
  - 缺点：trap 本身不便携带任意 message 给 guest，需要额外通道暴露 reason

建议：默认采用 **方案 B**，以确保“agent 调用 hostcall 时能返回错误并终止 WASM 执行”。

### 4) 终止消息（reason）如何暴露

trap (`CoreError`) 无法直接把 message 写回 guest 内存，建议用以下方式暴露（可二选一或都做）：

- **写入 WASM 日志 ring**：守卫触发时写一条包含 `execution_id/task_id/instance_id` 的 error log；系统已具备 per-execution log ring。
- **可选新增 hostcall**：`spear_termination_reason(out_ptr, out_len_ptr) -> i32`
  - 无 execution id 返回 `-ENOTCONN`
  - 未终止返回 `-EAGAIN`
  - buffer 不够返回 `-ENOSPC` 并写入所需长度
  - 成功写入 message 返回 0

### 5) 终止标记的触发点

定义清晰的“谁在什么情况下 mark terminate”，并区分 scope：

- Terminate execution：
  - 用户触发 Terminate：`exec_termination.mark(execution_id, -ECANCELED, "terminated: ...")`
  - 超时：`exec_termination.mark(execution_id, -ECANCELED, "timeout: ...")`
- Destroy instance：
  - 用户 destroy instance / fatal error：`instance_termination.mark(instance_id, -ECANCELED, "instance destroyed: ...")`
  - 同时建议也 mark 当前 execution（如果 execution_id 可得），保证“当前执行”在下一次 hostcall 边界立即退出

清理：

- 每次 `Invoke` 开始时（worker loop 设置 execution id 之后）调用 `exec_termination.clear(execution_id)`，避免上一次遗留的 terminate 标记影响新执行。
- 执行结束后也 `exec_termination.clear(execution_id)`，避免注册表无限增长。
- instance 级终止（Destroy instance）通常不自动 clear，除非 instance 被重建/替换（否则会允许“死而复生”）。

### 6) Execution 级 vs Instance 级终止（两种必须支持的行为）

通常会同时存在两种需求：

- **只取消当前 execution**：后续仍可在该 instance 上跑新的 execution
- **终止整个 instance**：例如 fatal error 或用户明确 stop instance

关键差异：

- execution 级（停止当前 execution）：只影响当前 execution_id；运行时应允许后续新的 execution 继续复用该 instance。
- instance 级（Destroy instance）：影响该 instance 的所有执行；运行时应把 instance 状态置为 Stopped/Failed，并拒绝后续执行请求。

### 7) 运行时如何把“终止”转成取消/停止

当守卫触发并产生 trap，使 `vm.run_func*` 返回错误时：

- 将该错误映射为明确的 `ExecutionError::Cancelled { message }`（或新增专用错误类型，带 scope）
- completion event 的状态标记为 terminated/failed（按你们的语义选择）
- 若命中的是 instance 级终止（Destroy instance），在 runtime 侧进一步调用 `stop_instance`，使 instance 进入 Stopped，并阻止后续复用

## 实施计划（高层）

1) 新增终止 registry + state 类型（建议放在 `src/spearlet/execution/host_api/` 或 `runtime/` 下的独立模块）。
2) `DefaultHostApi` 增加 `termination_registry` 字段并初始化。
3) 在 `wasm_hostcalls.rs` 添加统一 guard，并在所有 hostcall 的开头调用。
4) 在执行生命周期里接入 mark/clear（invoke start/end；cancel/timeout/stop 路径）。
5) 测试：
   - 单测：mark terminate 后任意 hostcall 立刻返回终止错误/触发 trap
   - 集成：运行一个循环调用 `sleep_ms` 的 WASM，触发 terminate，验证能快速退出

## 兼容性说明

- guest 若依赖 Linux errno 数值，不应直接使用 WASI libc 的 `errno.h` 常量；应使用 SDK 提供的固定 errno 常量以避免数值不一致。
- 终止检查只在 hostcall 边界生效；纯 guest 计算无法被抢占。
