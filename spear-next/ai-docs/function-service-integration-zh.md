# FunctionService 与 Artifact-Task-Instance 架构集成

## 概述 / Overview

本文档描述了 FunctionService 如何与新的 Artifact-Task-Instance 架构集成，包括架构变更、实现细节和迁移策略。

## 架构变更 / Architecture Changes

### 原有架构 / Previous Architecture

```rust
pub struct FunctionServiceImpl {
    start_time: SystemTime,
    active_executions: Arc<RwLock<HashMap<String, ExecutionHandle>>>,
}
```

### 新架构 / New Architecture

```rust
pub struct FunctionServiceImpl {
    start_time: SystemTime,
    execution_manager: TaskExecutionManager,
    instance_pool: InstancePool,
    stats: Arc<RwLock<FunctionServiceStats>>,
}
```

## 核心组件集成 / Core Component Integration

### 1. TaskExecutionManager 集成

TaskExecutionManager 负责管理任务的生命周期：

- **任务创建** / Task Creation: 从 ArtifactSpec 创建新任务
- **执行管理** / Execution Management: 协调任务执行流程
- **状态跟踪** / Status Tracking: 跟踪任务执行状态

```rust
// 创建执行管理器 / Create execution manager
let runtime_manager = Arc::new(RuntimeManager::new());
let manager_config = TaskExecutionManagerConfig::default();
let execution_manager = TaskExecutionManager::new(manager_config, runtime_manager).await?;
```

### 2. InstancePool 集成

InstancePool 管理运行时实例：

- **实例调度** / Instance Scheduling: 使用 InstanceScheduler 进行负载均衡
- **资源管理** / Resource Management: 管理实例的生命周期
- **性能优化** / Performance Optimization: 实例复用和预热

```rust
// 创建实例池 / Create instance pool
let pool_config = InstancePoolConfig::default();
let scheduler = Arc::new(InstanceScheduler::new(SchedulingPolicy::RoundRobin));
let instance_pool = InstancePool::new(pool_config, scheduler).await?;
```

### 3. 统计信息增强

新的统计信息结构提供更详细的监控数据：

```rust
pub struct FunctionServiceStats {
    pub task_count: i32,
    pub execution_count: i32,
    pub running_executions: i32,
    pub artifact_count: i32,        // 新增
    pub instance_count: i32,        // 新增
    pub successful_executions: i32, // 新增
    pub failed_executions: i32,     // 新增
    pub average_response_time_ms: f64, // 新增
}
```

## 函数调用流程 / Function Invocation Flow

### 1. Artifact 创建

```rust
fn create_artifact_from_proto(proto_spec: &ArtifactSpec) -> Result<Artifact, ExecutionError> {
    let execution_spec = ExecutionArtifactSpec {
        name: proto_spec.artifact_id.clone(),
        version: proto_spec.version.clone(),
        description: "Generated from proto".to_string(),
        runtime_type: proto_spec.artifact_type.clone(),
        runtime_config: serde_json::Value::Null,
        environment: std::collections::HashMap::new(),
        resource_limits: Default::default(),
        invocation_type: InvocationType::NewTask,
        max_execution_timeout_ms: 30000,
        labels: proto_spec.metadata.clone(),
    };
    
    Artifact::new(execution_spec)
}
```

### 2. 任务执行

```rust
async fn invoke_function(
    &self,
    request: Request<InvokeFunctionRequest>,
) -> Result<Response<InvokeFunctionResponse>, Status> {
    let req = request.into_inner();
    
    // 1. 创建 Artifact
    let artifact = self.create_artifact_from_proto(&req.artifact_spec)?;
    
    // 2. 通过 ExecutionManager 执行
    let execution_request = ExecutionRequest {
        artifact_spec: req.artifact_spec,
        parameters: req.parameters,
        context: req.context,
    };
    
    let execution_response = self.execution_manager
        .execute_task(execution_request)
        .await?;
    
    // 3. 转换响应
    let response = self.execution_response_to_proto(execution_response)?;
    
    Ok(Response::new(response))
}
```

## 错误处理 / Error Handling

新架构提供统一的错误处理机制：

```rust
pub enum ExecutionError {
    ArtifactError(String),
    TaskError(String),
    InstanceError(String),
    RuntimeError(String),
    ConfigurationError(String),
}
```

## 性能优化 / Performance Optimizations

### 1. 实例复用
- 实例池维护热实例，减少冷启动时间
- 智能调度算法优化资源利用率

### 2. 异步执行
- 全异步架构，提高并发处理能力
- 非阻塞 I/O 操作

### 3. 内存管理
- Arc 和 RwLock 优化内存共享
- 智能缓存策略

## 监控和观测 / Monitoring and Observability

### 1. 指标收集
- 任务执行时间
- 成功/失败率
- 实例利用率
- 内存使用情况

### 2. 日志记录
```rust
use tracing::{info, warn, error, debug, trace};

// 结构化日志记录
info!(
    task_id = %task_id,
    execution_time_ms = execution_time,
    "Task execution completed"
);
```

### 3. 健康检查
```rust
async fn get_health(&self) -> Result<Response<GetHealthResponse>, Status> {
    let stats = self.get_stats().await?;
    
    let health_details = HealthDetails {
        service_name: "spearlet-function-service".to_string(),
        version: env!("CARGO_PKG_VERSION").to_string(),
        uptime_seconds: self.start_time.elapsed()?.as_secs() as i64,
        active_tasks: 10, // 从 execution_manager 获取
        memory_usage_mb: 0, // 待实现
        cpu_usage_percent: 0.0, // 待实现
    };
    
    Ok(Response::new(GetHealthResponse {
        healthy: true,
        details: Some(health_details),
    }))
}
```

## 迁移注意事项 / Migration Considerations

### 1. 向后兼容性
- 保持 gRPC 接口不变
- 渐进式迁移策略

### 2. 配置更新
- 新增 TaskExecutionManagerConfig
- 新增 InstancePoolConfig
- 更新 RuntimeConfig

### 3. 测试策略
- 单元测试覆盖新组件
- 集成测试验证端到端流程
- 性能测试确保无回归

## 未来扩展 / Future Extensions

### 1. 分布式执行
- 支持跨节点任务调度
- 分布式实例池

### 2. 高级调度
- 基于负载的智能调度
- 优先级队列

### 3. 自动扩缩容
- 基于负载自动调整实例数量
- 预测性扩容

## 总结 / Summary

新的 Artifact-Task-Instance 架构为 FunctionService 提供了：

1. **更好的模块化** / Better Modularity: 清晰的职责分离
2. **增强的可观测性** / Enhanced Observability: 详细的监控和日志
3. **提升的性能** / Improved Performance: 优化的资源管理
4. **更强的扩展性** / Better Scalability: 支持未来功能扩展

这种架构为构建高性能、可扩展的函数执行服务奠定了坚实基础。