# 调用模型重构（Invocation/Execution/Instance）设计文档

## 背景

当前系统在“invoke / execution / instance / function”等概念上存在混用，导致接口语义和实现责任边界不清晰，具体表现在：

- SMS Web Admin API 使用 `/admin/api/executions`，但语义更像“发起一次调用请求”
- Spearlet gRPC 暴露 `FunctionService.InvokeFunction`，但 handler 进一步分流到 `handle_*_execution()`
- `TaskExecutionManager::submit_execution()` 更偏“提交一次执行排队”，对 `function_name` 的参与较弱
- Runtime（以 WASM 为例）最终执行模型却是按 `function_name` 执行函数

代码位置（相对路径）：

- Admin BFF spillback 调用 Spearlet gRPC： [src/sms/web_admin.rs](../src/sms/web_admin.rs)
- Spearlet gRPC handler 分流： [src/spearlet/function_service.rs](../src/spearlet/function_service.rs)
- 执行提交与上下文构造： [src/spearlet/execution/manager.rs](../src/spearlet/execution/manager.rs)
- WASM runtime 按函数名执行： [src/spearlet/execution/runtime/wasm.rs](../src/spearlet/execution/runtime/wasm.rs)

## 目标

- runtime 层只保留“调用函数（invoke function）”这一种语义。
- 清晰定义并拆分：
  - Function：被调用的入口
  - Invocation：一次调用请求（端到端追踪单位）
  - Execution/Attempt：一次具体执行尝试（在某节点/实例上运行）
  - Instance：承载状态的运行环境
- 将“可重入（reentrant）”落地成可控能力（capacity 模型）。
- 为下一步“类似 terminal console 的交互式输入输出”预留清晰的一等模型。
- 给出可分阶段交付、低风险的实施方案。

## 非目标

- 大改 SMS placement 策略。
- 一次性引入持久化执行历史存储。
- 追求严格 exactly-once（当前阶段优先保证可观测与可控重试）。

## 术语定义（强语义）

### Function（函数）

Task/Artifact 内的可调用入口。

- 以 `function_name` 标识。
- 可长可短。
- runtime 的核心输入：`function_name + input + context + timeout`。

### Task（任务 / 部署单元）

SMS 控制面与 placement 的调度单位。

- 以 `task_id` 标识。
- 绑定 artifact、runtime 类型、资源限制、调度 hint 等。

### Instance（实例）

在某个 Spearlet 节点上为某个 Task 创建的有状态运行环境。

- 以 `instance_id` 标识。
- 生命周期由 Spearlet 管理。
- 是否支持并发由 capacity/reentrancy 决定。

### Invocation（调用请求）

外部发起的一次“调用某 task 的某 function”的请求。

- 以 `invocation_id` 标识。
- spillback/retry 中保持稳定。
- 用于端到端追踪、聚合展示。

### Execution / Attempt（执行尝试）

在某个节点（通常绑定某个 instance）上对一次 invocation 的一次具体运行尝试。

- 以 `execution_id` 标识（attempt id）。
- 一个 invocation 可能对应多个 execution（spillback/重试）。

## 不变式

- 一个 execution 只调用一个 function。
- 一个 execution 绑定至多一个 instance。
- 一个 invocation 可能产生多个 execution。
- task/instance 的创建、复用、扩缩容属于 Spearlet 内部实现细节。

## 状态模型

### ExecutionStatus（attempt 级）

- `PENDING` → `RUNNING` → `COMPLETED | FAILED | CANCELLED | TIMEOUT`

### InvocationStatus（聚合级）

- `ACCEPTED`（收到请求）
- `RUNNING`（任一 attempt 运行中）
- `SUCCEEDED`（任一 attempt 成功）
- `FAILED`（所有 attempt 失败且不再重试）
- `CANCELLED`

## 可重入（Reentrancy）与 Capacity

用 instance capacity 建模可重入能力：

- `capacity = 1`：默认不并发（最安全）。
- `capacity > 1`：允许并发（runtime 需保证并发安全与隔离）。

这与现有 instance pool/scheduler 的健康/ready/capacity 判断可以自然对齐。

## 接口设计

本节定义目标语义以及迁移方式。

### SMS Web Admin API（BFF）

#### 建议新增：`POST /admin/api/invocations`

请求体字段：

- `task_id`（必填）
- `function_name`（必填）
- `execution_mode`（可选：`sync|async|stream|console`，默认 `sync`）
- `invocation_id`（可选，不传则生成）
- `execution_id`（可选，每个 attempt 生成一次）
- `node_uuid`（可选：指定落点，跳过 placement）
- `max_candidates`（可选：spillback 最大候选数）
- `timeout_ms`（可选）
- `session_id`（可选）
- `input_json` 或 `input_base64`（可选二选一）

响应字段：

- `success`
- `invocation_id`
- `execution_id`
- `node_uuid`
- `message`
- `result`（sync 时返回）

#### Console（类似 terminal）交互

对于“像 terminal 一样”的交互式会话（stdin/stdout/stderr + resize/signal 等控制消息），BFF 建议提供 WebSocket 接口，把浏览器/CLI 的 I/O 双向转发到 Spearlet gRPC 的双向流。

建议接口：

- `GET /admin/api/executions/{execution_id}/console`（WebSocket 升级）

消息模型（抽象级别）：

- Client → BFF：
  - `stdin` 原始字节
  - `resize`（`rows`, `cols`）
  - `signal`（如 `INT`, `TERM`）
  - `close`
- BFF → Client：
  - `stdout` 字节
  - `stderr` 字节
  - `status` 更新
  - `exit`（最终）

#### 兼容：保留 `POST /admin/api/executions`

本次重构允许破坏性升级：

- 用 `POST /admin/api/invocations` 替换 `POST /admin/api/executions`。
- 同步修改仓库内所有调用它的代码与 admin 资源。
- 不保留 alias/兼容入口。

### Spearlet gRPC API

#### 目标态：引入 v2 RPC（强烈建议）

为了避免继续在旧 RPC 上叠加“invoke 但实际是 submit execution / 可能建实例 / 甚至建 task”的屎山语义，建议用一套干净的 RPC 直接替换旧接口（破坏性变更）。

核心原则：

- runtime 面向的 API 只表达“调用函数”。
- invocation 与 execution（attempt）显式区分。
- 控制面动作（创建/删除 task）不再内嵌在 invocation RPC 内。
- payload 统一为 bytes + content_type，而不是到处 `Any`。

##### 包与版本策略

- 直接重写 `proto/spearlet/function.proto`（或拆分为 `invocation.proto` / `execution.proto`）来定义新服务。
- 移除旧的 `FunctionService` 与相关消息。
- 同步更新仓库内所有调用方到新 stub。

##### v2 proto 草案

```proto
syntax = "proto3";

package spearlet;

import "google/protobuf/timestamp.proto";

enum ExecutionMode {
  EXECUTION_MODE_UNSPECIFIED = 0;
  EXECUTION_MODE_SYNC = 1;
  EXECUTION_MODE_ASYNC = 2;
  EXECUTION_MODE_STREAM = 3;
  EXECUTION_MODE_CONSOLE = 4;
}

enum ExecutionStatus {
  EXECUTION_STATUS_UNSPECIFIED = 0;
  EXECUTION_STATUS_PENDING = 1;
  EXECUTION_STATUS_RUNNING = 2;
  EXECUTION_STATUS_COMPLETED = 3;
  EXECUTION_STATUS_FAILED = 4;
  EXECUTION_STATUS_CANCELLED = 5;
  EXECUTION_STATUS_TIMEOUT = 6;
}

message Payload {
  string content_type = 1;
  bytes data = 2;
}

message Error {
  string code = 1;
  string message = 2;
}

message InvokeRequest {
  string invocation_id = 1;
  string execution_id = 2;

  string task_id = 3;
  string function_name = 4;

  Payload input = 5;
  map<string, string> headers = 6;
  map<string, string> environment = 7;

  uint64 timeout_ms = 8;
  string session_id = 9;
  ExecutionMode mode = 10;

  bool force_new_instance = 11;
  map<string, string> metadata = 12;
}

message InvokeResponse {
  string invocation_id = 1;
  string execution_id = 2;
  string instance_id = 3;

  ExecutionStatus status = 4;
  Payload output = 5;
  Error error = 6;

  google.protobuf.Timestamp started_at = 7;
  google.protobuf.Timestamp completed_at = 8;
}

message InvokeStreamChunk {
  string invocation_id = 1;
  string execution_id = 2;
  string instance_id = 3;

  ExecutionStatus status = 4;
  Payload chunk = 5;
  bool is_final = 6;
  Error error = 7;
  map<string, string> metadata = 8;
}

service InvocationService {
  rpc Invoke(InvokeRequest) returns (InvokeResponse);
  rpc InvokeStream(InvokeRequest) returns (stream InvokeStreamChunk);
  rpc OpenConsole(stream ConsoleClientMessage) returns (stream ConsoleServerMessage);
}

message TerminalSize {
  uint32 rows = 1;
  uint32 cols = 2;
}

enum ConsoleSignal {
  CONSOLE_SIGNAL_UNSPECIFIED = 0;
  CONSOLE_SIGNAL_INT = 1;
  CONSOLE_SIGNAL_TERM = 2;
}

message ConsoleOpen {
  InvokeRequest invoke = 1;
  TerminalSize initial_size = 2;
}

message ConsoleClientMessage {
  oneof msg {
    ConsoleOpen open = 1;
    bytes stdin = 2;
    TerminalSize resize = 3;
    ConsoleSignal signal = 4;
  }
}

message ConsoleExit {
  int32 code = 1;
  string message = 2;
}

message ConsoleServerMessage {
  string invocation_id = 1;
  string execution_id = 2;
  string instance_id = 3;

  oneof msg {
    bytes stdout = 10;
    bytes stderr = 11;
    ExecutionStatus status = 12;
    ConsoleExit exit = 13;
    Error error = 14;
  }
}
```
message GetExecutionRequest {
  string execution_id = 1;
  bool include_output = 2;
}

message Execution {
  string invocation_id = 1;
  string execution_id = 2;
  string task_id = 3;
  string function_name = 4;
  string instance_id = 5;
  ExecutionStatus status = 6;
  Payload output = 7;
  Error error = 8;
  google.protobuf.Timestamp started_at = 9;
  google.protobuf.Timestamp completed_at = 10;
}

message CancelExecutionRequest {
  string execution_id = 1;
  string reason = 2;
}

message CancelExecutionResponse {
  bool success = 1;
  ExecutionStatus final_status = 2;
  string message = 3;
}

message ListExecutionsRequest {
  string task_id = 1;
  string invocation_id = 2;
  uint32 limit = 3;
  string page_token = 4;
}

message ListExecutionsResponse {
  repeated Execution executions = 1;
  string next_page_token = 2;
}

service ExecutionService {
  rpc GetExecution(GetExecutionRequest) returns (Execution);
  rpc CancelExecution(CancelExecutionRequest) returns (CancelExecutionResponse);
  rpc ListExecutions(ListExecutionsRequest) returns (ListExecutionsResponse);
}
```

##### v1（`proto/spearlet/function.proto`）中建议语义废弃的字段

仅保留用于 shim（不再鼓励使用）：

- `InvocationType` 及 `invocation_type`
- `create_if_not_exists`
- `wait`

行为约束：

- v1 `InvokeFunction` 语义收敛为“对已存在 task 的函数调用”，任何“创建 task”的行为迁移到 SMS TaskService。

#### 旧接口移除范围

新接口中直接移除以下旧语义/字段：

- `InvocationType` / “通过 invoke 创建 task”
- `create_if_not_exists`
- `wait`

创建/删除 task 继续由 SMS TaskService 承担。

### Spearlet HTTP Gateway

当前 OpenAPI 含 `/functions/invoke` 但 handler 多为 TODO。

方案：

- 要么实现 `/functions/invoke` 作为 gRPC 的薄适配层。
- 要么先移除/隐藏该 OpenAPI 路径，避免误导。

## Spearlet 内部架构（目标态）

### 分层

1. API 适配层（gRPC/HTTP）
2. Invocation 引擎
   - 校验
   - 选 task
   - 选/建 instance
   - 调 runtime invoke
3. Runtime（WASM/Process/Container）
4. Execution 记录存储（先内存）

### 核心内部数据结构（建议）

#### InvocationRequest（内部）

- `invocation_id: String`
- `execution_id: String`
- `task_id: String`
- `function_name: String`
- `mode: ExecutionMode`
- `timeout_ms: u64`
- `session_id: Option<String>`
- `input: bytes`（统一形态）
- `metadata: map<string,string>`

#### ExecutionRecord

- `execution_id`
- `invocation_id`
- `task_id`
- `instance_id`（若绑定）
- `node_uuid`（若已知）
- `status`
- `timestamps`
- `result / error`

### 最小可用内存索引

- `executions_by_id: execution_id -> ExecutionRecord`
- `executions_by_invocation: invocation_id -> Vec<execution_id>`

用于：

- 按 execution 查询状态。
- 在 admin 侧聚合展示同一次 invocation 的多个 attempt。

## 幂等与重试

### 重试原则

- spillback 的重试由 SMS Web Admin BFF 决策发起。
- Spearlet 将 `(invocation_id, execution_id)` 视为唯一 attempt。

### 幂等行为

- 同一个 `execution_id` 重放：返回已有记录（避免重复执行）。
- 同一个 `invocation_id` + 新 `execution_id`：创建新 attempt。

## 取消语义

### CancelExecution

仅取消一个 attempt（`execution_id`）。

可选扩展：

- `CancelInvocation(invocation_id)`：取消所有 attempts。

## Practical 实施方案（分阶段交付）

该方案允许破坏性升级，但要求一次性同步更新仓库内所有调用方与测试。

### Phase 1：重写 proto 并重新生成代码（破坏性变更）

改动：

- 将 `FunctionService` 替换为 `InvocationService` 与 `ExecutionService`。
- 将 `InvokeFunctionRequest/Response` 替换为 `InvokeRequest/Response`。
- `invocation_id` 与 `execution_id` 设为必填。
- 用 `Payload { content_type, bytes }` 统一输入输出，减少 `Any`。

验收：

- `cargo build` 通过且生成代码生效。

### Phase 2：更新 Spearlet 服务端实现

改动：

- 用新服务实现替换 [function_service.rs](../src/spearlet/function_service.rs)（可重命名为 invocation/execution service）。
- 更新 [grpc_server.rs](../src/spearlet/grpc_server.rs) 注册新服务。
- 确保执行记录包含 `invocation_id`、`execution_id`、`function_name`、`instance_id`。

验收：

- 调用/流式/查询/取消/列举能本地跑通。
- OpenConsole 能跑通一条 stdin/stdout/stderr 的交互链路。

### Phase 3：同步更新仓库内所有调用方

范围（需全仓搜索确认）：

- SMS Web Admin BFF：[web_admin.rs](../src/sms/web_admin.rs)
- 测试：
  - [placement_spillback_e2e_tests.rs](../tests/placement_spillback_e2e_tests.rs)
  - [spearlet_fetch_task_from_sms_tests.rs](../tests/spearlet_fetch_task_from_sms_tests.rs)
  - 其它引用 `proto::spearlet::*` 的测试
- Admin 静态资源 `assets/admin/`（对接新的 BFF endpoint 与请求结构）。

验收：

- `cargo test` 全部通过。

### Phase 4：删除遗留入口与死代码

改动：

- 删除不再使用的旧消息/服务与相关分支。
- 若 HTTP gateway OpenAPI 与实现不一致，移除误导路径。

## 测试方案

### 单元测试

- 请求校验：Phase 2 后空 `function_name` 必须失败。
- 幂等：重复 `execution_id` 不应重复执行。

### 集成测试

- SMS Web Admin spillback：
  - 第一个候选节点 Unavailable，第二个成功
  - 验证同一 invocation_id，多次 execution_id

- Console 交互：
  - 建立 console，会话内发送 stdin，收到 stdout/stderr
  - resize 不应中断流
  - cancel/terminate 后应收到最终 exit/status

### 可观测性检查

- 执行记录至少包含：`invocation_id`、`task_id`、`function_name`、`instance_id`。

## 灰度与回滚

### 灰度

- 以“单次合入”为主：proto + server + callers 一起改。

### 回滚

- 整体回滚该破坏性变更提交。

## 待确定项（默认建议）

- Phase 0 缺省 function_name：建议 `"__default__"` 或 task 定义 default。
- 输入形态：建议统一为 bytes payload，对 JSON 提供便捷层。
