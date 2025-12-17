# Secret 验证逻辑实现

## 概述

本文档描述了 Spear 执行系统中 secret 验证逻辑的实现，专门用于验证运行时进程与连接管理器之间的连接认证。

## 架构设计

### 核心组件

1. **ConnectionManager** (`src/spearlet/execution/communication/connection_manager.rs`)
   - 管理来自运行时进程的 WebSocket 连接
   - 使用实例 secret 处理认证请求
   - 支持基于 TaskExecutionManager 的验证和回退验证

2. **TaskInstance** (`src/spearlet/execution/instance.rs`)
   - 包含 `secret` 字段：`Arc<parking_lot::RwLock<Option<String>>>`
   - 存储每个任务实例的认证密钥

3. **TaskExecutionManager** (`src/spearlet/execution/manager.rs`)
   - 提供 `get_instance(instance_id)` 方法来检索 TaskInstance
   - 任务执行和实例生命周期的中央管理器

## 实现细节

### Secret 验证流程

1. **认证请求**：运行时进程发送包含 `instance_id` 和 `secret` 的认证请求
2. **实例查找**：ConnectionManager 尝试使用 TaskExecutionManager 查找 TaskInstance
3. **Secret 比较**：验证提供的 secret 与存储的实例 secret
4. **回退验证**：如果 TaskExecutionManager 不可用，使用基本的 secret 验证器

### 关键方法

#### `handle_auth_request_static`
```rust
pub fn handle_auth_request_static(
    connections: Arc<DashMap<String, Arc<ConnectionHandler>>>,
    instance_connections: Arc<DashMap<String, Vec<String>>>,
    secret_validator: Option<Box<dyn Fn(&str) -> bool + Send + Sync>>,
    execution_manager: Option<Arc<TaskExecutionManager>>,
    msg: AuthRequest,
    connection_id: String,
) -> Result<AuthResponse, Box<dyn std::error::Error + Send + Sync>>
```

**验证逻辑：**
1. 如果可用，尝试从 execution_manager 获取 TaskInstance
2. 比较提供的 secret 与实例 secret
3. 如果 execution_manager 为 None，回退到 secret_validator
4. 返回适当的 AuthResponse

#### 构造方法
- `new_with_execution_manager`：创建带有 TaskExecutionManager 引用的 ConnectionManager
- `new_with_validator`：创建带有自定义 secret 验证器的 ConnectionManager（回退方案）

### 安全特性

1. **实例特定的 Secret**：每个 TaskInstance 都有自己的唯一 secret
2. **安全存储**：Secret 存储在线程安全的 RwLock 中
3. **验证回退**：当 TaskExecutionManager 不可用时优雅降级
4. **基本验证**：最小 secret 长度和非空检查

## 集成点

### ProcessRuntime 集成
- 使用增强的 secret 验证的 `new_with_validator`
- 实现基本安全检查（长度、非空）
- 保持向后兼容性

### 未来增强
- ProcessRuntime 中的直接 TaskExecutionManager 集成
- 增强的 secret 生成和轮换
- 认证尝试的审计日志

## 配置

### ConnectionManager 设置
```rust
// 使用 TaskExecutionManager（首选）
let manager = ConnectionManager::new_with_execution_manager(config, execution_manager);

// 使用自定义验证器（回退）
let validator = Box::new(|secret: &str| {
    !secret.is_empty() && secret.len() >= 8
});
let manager = ConnectionManager::new_with_validator(config, Some(validator));
```

## 错误处理

- 无效的 instance_id：返回认证失败
- 缺少 secret：返回认证失败
- Secret 不匹配：返回认证失败
- 系统错误：记录日志并返回内部错误响应

## 测试考虑

1. **单元测试**：测试各种场景下的 secret 验证逻辑
2. **集成测试**：测试完整的认证流程
3. **安全测试**：测试无效/恶意输入
4. **性能测试**：验证并发连接下的性能

## 相关文件

- `src/spearlet/execution/communication/connection_manager.rs`
- `src/spearlet/execution/instance.rs`
- `src/spearlet/execution/manager.rs`
- `src/spearlet/execution/runtime/process.rs`