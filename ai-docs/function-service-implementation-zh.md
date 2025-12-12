# 函数服务实现

## 概述

本文档描述了 Spearlet 函数服务的实现，该服务提供了基于 gRPC 的综合 API，用于函数执行、任务管理和监控功能。

## 架构设计

### Proto 定义 (`function.proto`)

函数服务在 `proto/spearlet/function.proto` 中定义，遵循与现有对象服务相同的架构模式：

#### 核心 RPC 方法

1. **函数执行**
   - `InvokeFunction`: 执行函数，支持同步/异步模式
   - `GetExecutionStatus`: 查询执行状态和结果
   - `CancelExecution`: 取消正在运行的执行
   - `StreamFunction`: 基于流的函数执行

2. **任务管理**
   - `ListTasks`: 列出可用任务，支持分页
   - `GetTask`: 获取特定任务信息
   - `DeleteTask`: 从系统中删除任务
   - `ListExecutions`: 列出执行历史

3. **健康检查与监控**
   - `GetHealth`: 服务健康状态，包含详细指标
   - `GetStats`: 综合服务统计信息

#### 消息结构

**请求/响应模式：**
- 所有请求遵循一致命名：`{Method}Request`
- 所有响应遵循一致命名：`{Method}Response`
- 分页支持，包含 `limit`、`start_after` 和 `has_more` 字段
- 可选字段用于详细信息（`include_details`、`include_logs`）

**关键数据结构：**
- `TaskInfo`: 任务元数据和配置
- `ExecutionInfo`: 执行状态和结果
- `HealthDetails`: 服务健康指标
- `ServiceStats`、`TaskStats`、`ExecutionStats`: 统计信息

### Rust 实现 (`function_service.rs`)

Rust 实现遵循 `object_service.rs` 的既定模式：

#### 核心组件

1. **FunctionServiceImpl**
   ```rust
   pub struct FunctionServiceImpl {
       kv_store: Arc<dyn KvStore>,
       stats: Arc<RwLock<FunctionServiceStats>>,
       default_timeout_ms: u64,
       max_concurrent_executions: usize,
       start_time: std::time::Instant,
   }
   ```

2. **统计信息跟踪**
   ```rust
   pub struct FunctionServiceStats {
       pub total_tasks: u64,
       pub total_executions: u64,
       pub running_executions: u64,
       pub successful_executions: u64,
       pub failed_executions: u64,
   }
   ```

3. **存储模型**
   - `StoredTask`: 可序列化的任务信息
   - `StoredExecution`: 可序列化的执行数据

#### 实现模式

**一致的错误处理：**
- 所有方法返回 `Result<Response<T>, Status>`
- 针对不同错误条件使用适当的 gRPC 状态码
- 提供详细的错误消息用于调试

**KV 存储集成：**
- 任务使用前缀 `task:` 存储
- 执行使用前缀 `execution:` 存储
- 原子操作确保一致性

**统计信息管理：**
- 使用 `Arc<RwLock<T>>` 实现线程安全的统计
- 操作期间实时更新
- 全面的指标收集

## 与 gRPC 服务器的集成

### 服务器注册

函数服务集成到主 gRPC 服务器（`grpc_server.rs`）中：

```rust
pub struct GrpcServer {
    object_service: Arc<ObjectServiceImpl>,
    function_service: Arc<FunctionServiceImpl>,  // 新增
    health_service: Arc<HealthService>,
}
```

### 健康服务集成

健康服务现在包含函数服务指标：

```rust
pub struct HealthStatus {
    pub status: String,
    pub uptime_seconds: i64,
    pub object_count: u64,
    pub total_object_size: u64,
    pub task_count: u64,        // 新增
    pub execution_count: u64,   // 新增
    pub running_executions: u64, // 新增
}
```

## 关键设计决策

### 1. 与对象服务的一致性
- 相同的架构模式和代码结构
- 一致的命名约定和错误处理
- 类似的 KV 存储使用模式

### 2. 全面的 API 接口
- 任务的完整 CRUD 操作
- 详细的执行跟踪和监控
- 丰富的统计和健康信息

### 3. 可扩展性考虑
- 可配置的超时和并发限制
- 大数据集的高效分页
- 数据一致性的原子操作

### 4. 监控和可观测性
- 包含服务特定指标的详细健康检查
- 全面的统计信息收集
- 实时状态跟踪

## 未来增强

### Proto 生成
- 需要重新生成 proto 文件以创建 gRPC 服务器存根
- 一旦 proto 类型可用，将添加集成测试

### 实现细节
- 实际的函数执行逻辑（当前为占位符）
- 高级任务调度和队列
- 分布式执行能力
- 增强的监控和告警

## 测试策略

### 单元测试
- 基本服务创建和初始化
- 统计信息跟踪和更新
- ID 生成工具

### 集成测试（待完成）
- 完整的 gRPC 方法测试
- 端到端执行工作流
- 错误处理场景
- 性能和负载测试

## 修改/创建的文件

1. **Proto 定义**: `proto/spearlet/function.proto`
2. **服务实现**: `src/spearlet/function_service.rs`
3. **测试框架**: `src/spearlet/function_service_test.rs`
4. **模块集成**: `src/spearlet/mod.rs`
5. **服务器集成**: `src/spearlet/grpc_server.rs`

## 下一步

1. 重新生成 proto 文件以创建 gRPC 存根
2. 完成集成测试
3. 实现实际的函数执行逻辑
4. 添加高级监控和指标
5. 性能优化和负载测试