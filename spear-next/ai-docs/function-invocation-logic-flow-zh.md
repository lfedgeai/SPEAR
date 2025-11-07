# Spearlet 函数调用逻辑流程设计

## 概述

本文档详细描述了 Spearlet 中 `InvokeFunction` 接口的逻辑处理流程，重点说明新任务创建和现有任务调用的区分逻辑。

## 核心逻辑流程

### 1. 请求预处理阶段

```
InvokeFunctionRequest 接收
    ↓
参数验证和规范化
    ↓
根据 invocation_type 分流处理
    ├── INVOCATION_TYPE_NEW_TASK → 新任务创建流程
    ├── INVOCATION_TYPE_EXISTING_TASK → 现有任务调用流程
    └── INVOCATION_TYPE_UNKNOWN → 返回错误
```

### 2. 新任务创建流程 (INVOCATION_TYPE_NEW_TASK)

```
开始新任务创建
    ↓
验证必需参数
├── task_name (必需)
├── artifact_spec (必需)
└── function_name (必需)
    ↓
检查任务是否已存在
├── 存在 → 根据策略处理
│   ├── 如果 force_new_instance=true → 创建新实例
│   └── 否则 → 使用现有任务
└── 不存在 → 继续创建流程
    ↓
制品处理
├── 下载制品 (如果是远程位置)
├── 验证制品完整性 (checksum)
├── 解析制品元数据
└── 缓存制品到本地
    ↓
任务实例创建
├── 分配唯一 task_id
├── 创建任务运行时环境
├── 加载制品到运行时
└── 初始化任务实例
    ↓
可选：注册到 SMS
├── 如果配置了 SMS 集成
└── 发送 RegisterTask 请求
    ↓
执行函数调用 → 转到执行阶段
```

### 3. 现有任务调用流程 (INVOCATION_TYPE_EXISTING_TASK)

```
开始现有任务调用
    ↓
验证必需参数
├── task_id (必需)
└── function_name (必需)
    ↓
查找任务实例
├── 在本地任务池中查找
├── 如果未找到且配置了 SMS
│   └── 从 SMS 查询任务信息
└── 如果仍未找到
    ↓
任务不存在处理
├── 如果 create_if_not_exists=true
│   ├── 检查是否有足够信息创建任务
│   ├── 如果有 artifact_spec → 创建新任务
│   └── 否则 → 返回错误
└── 否则 → 返回任务不存在错误
    ↓
任务实例获取/创建
├── 如果任务存在但无可用实例
│   ├── 从实例池获取实例
│   ├── 如果池中无实例 → 创建新实例
│   └── 如果 force_new_instance=true → 强制创建新实例
└── 如果有可用实例 → 直接使用
    ↓
执行函数调用 → 转到执行阶段
```

### 4. 函数执行阶段

```
准备执行环境
├── 设置执行上下文
├── 配置环境变量
├── 设置超时和重试参数
└── 生成唯一 execution_id
    ↓
根据执行模式分流
├── EXECUTION_MODE_SYNC → 同步执行
├── EXECUTION_MODE_ASYNC → 异步执行
└── EXECUTION_MODE_STREAM → 流式执行
    ↓
执行函数调用
├── 调用任务实例的指定函数
├── 传递参数
├── 监控执行状态
└── 收集执行指标
    ↓
处理执行结果
├── 成功 → 返回结果
├── 失败 → 错误处理和重试
└── 超时 → 取消执行并返回超时错误
    ↓
清理和资源回收
├── 更新实例状态
├── 记录执行日志
└── 释放资源（如需要）
```

## 详细逻辑实现伪代码

### 主要处理函数

```rust
async fn invoke_function(request: InvokeFunctionRequest) -> Result<InvokeFunctionResponse> {
    // 1. 参数验证
    validate_request(&request)?;
    
    // 2. 根据调用类型分流
    let task_instance = match request.invocation_type {
        InvocationType::NewTask => {
            handle_new_task_creation(&request).await?
        },
        InvocationType::ExistingTask => {
            handle_existing_task_invocation(&request).await?
        },
        _ => return Err("Invalid invocation type"),
    };
    
    // 3. 执行函数调用
    let execution_result = execute_function(
        &task_instance,
        &request.function_name,
        &request.parameters,
        &request.execution_mode,
        &request.context,
    ).await?;
    
    // 4. 构造响应
    Ok(InvokeFunctionResponse {
        success: true,
        execution_id: execution_result.execution_id,
        task_id: task_instance.task_id,
        instance_id: task_instance.instance_id,
        result: execution_result.result,
        // ... 其他字段
    })
}
```

### 新任务创建处理

```rust
async fn handle_new_task_creation(request: &InvokeFunctionRequest) -> Result<TaskInstance> {
    // 验证新任务创建的必需参数
    if request.task_name.is_empty() || request.artifact_spec.is_none() {
        return Err("Missing required parameters for new task creation");
    }
    
    let task_name = &request.task_name;
    let artifact_spec = request.artifact_spec.as_ref().unwrap();
    
    // 检查任务是否已存在
    if let Some(existing_task) = task_manager.find_task_by_name(task_name) {
        if !request.force_new_instance {
            // 使用现有任务
            return task_manager.get_or_create_instance(&existing_task.task_id).await;
        }
    }
    
    // 处理制品
    let artifact = artifact_manager.download_and_verify_artifact(artifact_spec).await?;
    
    // 创建新任务
    let task_id = generate_unique_task_id();
    let task = Task {
        task_id: task_id.clone(),
        name: task_name.clone(),
        description: request.task_description.clone(),
        artifact_spec: artifact_spec.clone(),
        status: TaskStatus::Active,
        created_at: current_timestamp(),
        // ... 其他字段
    };
    
    // 注册任务
    task_manager.register_task(task).await?;
    
    // 可选：注册到 SMS
    if config.sms_integration_enabled {
        sms_client.register_task(&task).await?;
    }
    
    // 创建任务实例
    let instance = task_manager.create_instance(&task_id, &artifact).await?;
    
    Ok(instance)
}
```

### 现有任务调用处理

```rust
async fn handle_existing_task_invocation(request: &InvokeFunctionRequest) -> Result<TaskInstance> {
    // 验证现有任务调用的必需参数
    if request.task_id.is_empty() {
        return Err("Missing task_id for existing task invocation");
    }
    
    let task_id = &request.task_id;
    
    // 查找任务
    let task = match task_manager.find_task_by_id(task_id) {
        Some(task) => task,
        None => {
            // 任务不存在，检查是否可以创建
            if request.create_if_not_exists {
                if let Some(artifact_spec) = &request.artifact_spec {
                    // 有足够信息创建新任务
                    return handle_new_task_creation_from_existing_request(request).await;
                } else {
                    return Err("Cannot create task without artifact specification");
                }
            } else {
                return Err("Task not found and create_if_not_exists is false");
            }
        }
    };
    
    // 获取或创建任务实例
    let instance = if request.force_new_instance {
        task_manager.create_new_instance(&task.task_id).await?
    } else {
        task_manager.get_or_create_instance(&task.task_id).await?
    };
    
    Ok(instance)
}
```

### 函数执行处理

```rust
async fn execute_function(
    instance: &TaskInstance,
    function_name: &str,
    parameters: &[FunctionParameter],
    execution_mode: &ExecutionMode,
    context: &ExecutionContext,
) -> Result<ExecutionResult> {
    let execution_id = generate_execution_id();
    
    // 设置执行上下文
    let exec_context = ExecutionContext {
        execution_id: execution_id.clone(),
        timeout_ms: context.timeout_ms,
        max_retries: context.max_retries,
        environment: context.environment.clone(),
        // ... 其他字段
    };
    
    match execution_mode {
        ExecutionMode::Sync => {
            // 同步执行
            let result = instance.invoke_function_sync(
                function_name,
                parameters,
                &exec_context,
            ).await?;
            
            Ok(ExecutionResult {
                status: ExecutionStatus::Completed,
                result: Some(result),
                execution_id,
                // ... 其他字段
            })
        },
        ExecutionMode::Async => {
            // 异步执行
            let execution_handle = instance.invoke_function_async(
                function_name,
                parameters,
                &exec_context,
            ).await?;
            
            // 存储执行句柄以供后续查询
            execution_manager.store_execution(execution_id.clone(), execution_handle);
            
            Ok(ExecutionResult {
                status: ExecutionStatus::Pending,
                execution_id,
                // ... 其他字段
            })
        },
        ExecutionMode::Stream => {
            // 流式执行（通过单独的 StreamFunction RPC 处理）
            Err("Streaming mode should use StreamFunction RPC")
        },
        _ => Err("Unsupported execution mode"),
    }
}
```

## 错误处理策略

### 1. 参数验证错误
```rust
fn validate_request(request: &InvokeFunctionRequest) -> Result<()> {
    match request.invocation_type {
        InvocationType::NewTask => {
            if request.task_name.is_empty() {
                return Err("task_name is required for new task creation");
            }
            if request.artifact_spec.is_none() {
                return Err("artifact_spec is required for new task creation");
            }
        },
        InvocationType::ExistingTask => {
            if request.task_id.is_empty() {
                return Err("task_id is required for existing task invocation");
            }
        },
        _ => return Err("Invalid invocation_type"),
    }
    
    if request.function_name.is_empty() {
        return Err("function_name is required");
    }
    
    Ok(())
}
```

### 2. 制品处理错误
- 下载失败：重试机制，最终失败时返回详细错误信息
- 校验失败：立即失败，不进行重试
- 解析失败：检查制品格式，返回格式错误信息

### 3. 任务创建错误
- 资源不足：返回资源不足错误，建议稍后重试
- 权限不足：返回权限错误，建议检查权限配置
- 网络错误：重试机制，记录详细网络错误信息

### 4. 执行错误
- 函数不存在：返回函数不存在错误
- 参数错误：返回参数验证错误
- 执行超时：取消执行，返回超时错误
- 运行时错误：捕获异常，返回详细错误信息

## 性能优化考虑

### 1. 任务查找优化
- 使用内存缓存加速任务查找
- 实现任务索引（按名称、ID、标签等）
- 定期清理过期任务缓存

### 2. 实例池管理
- 预热常用任务的实例
- 实现实例复用策略
- 动态调整实例池大小

### 3. 制品缓存
- 本地制品缓存机制
- 制品版本管理
- 缓存清理策略

### 4. 并发控制
- 限制同时创建的任务数量
- 实现任务创建队列
- 资源争用检测和避免

## 监控和可观测性

### 1. 关键指标
- 任务创建成功率和耗时
- 函数调用成功率和耗时
- 实例池使用率
- 制品缓存命中率

### 2. 日志记录
- 完整的请求处理链路日志
- 错误详情和堆栈信息
- 性能指标记录

### 3. 分布式追踪
- 使用 trace_id 关联整个调用链
- 记录关键处理节点的时间戳
- 支持跨服务的链路追踪

## 总结

这个逻辑流程设计提供了：

1. **清晰的处理分流** - 根据调用类型明确区分处理逻辑
2. **灵活的任务管理** - 支持新任务创建和现有任务调用
3. **完善的错误处理** - 覆盖各种异常情况
4. **高效的资源利用** - 实例复用和制品缓存
5. **全面的可观测性** - 监控、日志和追踪

该设计确保了 `InvokeFunction` 接口的可靠性、性能和可维护性。