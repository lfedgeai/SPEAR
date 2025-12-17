# 执行模式支持实现文档

## 概述

本文档描述了为 Spear 项目添加异步/同步执行模式支持的实现细节。该功能允许函数调用支持三种执行模式：同步（Sync）、异步（Async）和流式（Stream）。

## 修改内容

### 1. ExecutionResponse 结构体重构

#### 问题
原有的 `ExecutionResponse` 结构体缺少执行模式相关的字段，无法区分同步和异步执行的状态。

#### 解决方案
在 `src/spearlet/execution/runtime/mod.rs` 中：

1. **重命名冲突解决**：将 runtime 模块中的 `ExecutionResponse` 重命名为 `RuntimeExecutionResponse`，避免与 execution 模块中的 `ExecutionResponse` 冲突。

2. **添加新字段**：
   ```rust
   pub struct RuntimeExecutionResponse {
       pub request_id: String,
       pub output_data: Vec<u8>,
       pub status: ExecutionStatus,
       pub error_message: String,
       pub execution_time_ms: u64,
       pub metadata: HashMap<String, String>,
       pub timestamp: SystemTime,
       // 新增字段
       pub execution_mode: ExecutionMode,     // 执行模式
       pub execution_status: ExecutionStatus, // 执行状态
       pub is_async: bool,                    // 是否异步执行
   }
   ```

3. **便利构造函数**：
   ```rust
   impl RuntimeExecutionResponse {
       pub fn new_sync(/* 参数 */) -> Self { /* 同步执行构造 */ }
       pub fn new_async(/* 参数 */) -> Self { /* 异步执行构造 */ }
   }
   ```

### 2. Runtime 模块更新

更新了以下文件以使用新的 `RuntimeExecutionResponse`：

- `src/spearlet/execution/runtime/docker.rs`
- `src/spearlet/execution/runtime/process.rs`
- `src/spearlet/execution/runtime/wasm.rs`

所有运行时实现现在都使用统一的 `RuntimeExecutionResponse` 结构体，并通过便利构造函数创建响应对象。

### 3. FunctionService 执行模式处理

#### 修改的方法
在 `src/spearlet/function_service.rs` 中的 `invoke_function` 方法：

```rust
async fn invoke_function(&self, request: Request<InvokeFunctionRequest>) 
    -> Result<Response<InvokeFunctionResponse>, Status> {
    
    match req.execution_mode() {
        ExecutionMode::Sync => {
            // 同步执行：等待完成后返回完整结果
            let result = self.handle_sync_execution(&req, &execution_id, &task_id).await;
            // 返回包含完整结果的响应
        },
        ExecutionMode::Async => {
            // 异步执行：立即返回执行ID和状态端点
            let (status_endpoint, estimated_ms) = self.handle_async_execution(&req, &execution_id, &task_id).await;
            // 返回包含状态端点的响应
        },
        ExecutionMode::Stream => {
            // 流式执行应使用 StreamFunction RPC
            // 返回错误提示使用正确的 RPC 方法
        },
        _ => {
            // 未知执行模式处理
        }
    }
}
```

#### 新增的辅助方法

1. **`handle_sync_execution`**：处理同步执行逻辑
2. **`handle_async_execution`**：处理异步执行逻辑，返回状态端点和预估完成时间
3. **`create_failed_result`**：创建失败的执行结果

## 执行模式说明

### 同步模式 (Sync)
- 客户端发送请求后等待函数执行完成
- 响应中包含完整的执行结果
- `status_endpoint` 字段为空
- 适用于快速执行的函数

### 异步模式 (Async)
- 客户端发送请求后立即收到响应
- 响应中包含执行ID和状态查询端点
- 客户端需要通过状态端点轮询执行结果
- 适用于长时间运行的函数

### 流式模式 (Stream)
- 使用独立的 `StreamFunction` RPC 方法
- 支持实时数据流传输
- 在 `invoke_function` 中返回错误提示

### 模式对比

| 模式 | 响应时机 | 结果获取 | 适用场景 |
|------|----------|----------|----------|
| Sync | 执行完成后 | 响应中直接包含 | 快速函数 |
| Async | 立即返回 | 通过状态端点轮询 | 长时间运行 |
| Stream | 使用专用 RPC | 实时流式传输 | 流式处理 |

## 响应结构差异

### 同步模式响应
```rust
InvokeFunctionResponse {
    success: true/false,
    message: "执行结果消息",
    execution_id: "exec_xxx",
    task_id: "task_xxx",
    result: Some(ExecutionResult { /* 完整结果 */ }),
    status_endpoint: "", // 空字符串
    estimated_completion_ms: 0,
}
```

### 异步模式响应
```rust
InvokeFunctionResponse {
    success: true,
    message: "异步函数执行已启动",
    execution_id: "exec_xxx",
    task_id: "task_xxx",
    result: Some(ExecutionResult { 
        status: Pending,
        completed_at: None, // 尚未完成
        // 其他字段...
    }),
    status_endpoint: "/status/exec_xxx",
    estimated_completion_ms: 5000,
}
```

## 错误处理策略

1. **未知执行模式**：返回明确错误信息
2. **流式模式误用**：引导使用正确的 RPC 方法
3. **运行时错误**：统一错误码与消息格式

## 后续工作

### 立即需要
1. 将 `handle_sync_execution` 与 `handle_async_execution` 与真实运行时集成
2. 实现状态查询端点（HTTP 或 gRPC）
3. 实现异步任务调度与状态跟踪

### 中期目标
1. 添加执行超时机制
2. 实现任务优先级
3. 添加执行监控与指标

### 长期规划
1. 支持任务依赖关系
2. 实现分布式执行
3. 添加资源配额管理

## 测试策略

### 单元测试
1. 各执行模式的响应结构验证
2. 错误情况处理测试
3. 便利构造函数测试

### 集成测试
1. 端到端执行流程测试
2. 不同运行时的兼容性测试
3. 性能基准测试

### 压力测试
1. 高并发同步执行
2. 大量异步任务管理
3. 资源使用监控

## 兼容性说明

- 该修改向后兼容，现有的客户端代码无需修改
- 新的执行模式字段有默认值，确保兼容性
- 建议客户端逐步迁移到新的执行模式 API
