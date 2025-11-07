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

## 后续工作

1. **运行时实现**：当前的 `handle_sync_execution` 和 `handle_async_execution` 方法还需要与实际的运行时系统集成
2. **状态查询端点**：需要实现状态查询的 HTTP 端点或 gRPC 方法
3. **异步任务管理**：需要实现异步任务的调度和状态跟踪
4. **错误处理**：完善各种错误情况的处理逻辑

## 测试建议

1. 测试不同执行模式的请求处理
2. 验证响应结构的正确性
3. 测试错误情况的处理
4. 性能测试（特别是异步模式）

## 兼容性说明

- 该修改向后兼容，现有的客户端代码无需修改
- 新的执行模式字段有默认值，确保兼容性
- 建议客户端逐步迁移到新的执行模式 API