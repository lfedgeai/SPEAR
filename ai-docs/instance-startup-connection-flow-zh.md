# 实例启动和连接建立流程

## 概述

本文档描述了 Spear 执行系统中实例启动和连接建立的完整流程。涵盖了从实例创建到 spearlet 与运行时实例之间成功建立通信通道的整个生命周期。

## 架构组件

### 核心组件
- **ProcessRuntime**: 管理基于进程的运行时实例
- **ConnectionManager**: 处理 TCP 连接和身份验证
- **TaskExecutionManager**: 协调任务执行和实例管理
- **MonitoringService**: 跟踪连接事件和性能指标

### 通信协议
- **SpearMessage**: 所有通信的统一消息格式
- **AuthRequest/AuthResponse**: 身份验证握手协议
- **Secret Validation**: 基于令牌的身份验证机制

## 实例启动流程

### 1. 实例创建阶段

```rust
// ProcessRuntime 创建新实例
let instance = ProcessRuntime::create_instance(
    task_id,
    instance_config,
    resource_limits
).await?;
```

**步骤:**
1. **资源分配**: 分配 CPU、内存和网络资源
2. **端口分配**: 查找可用的通信端口
3. **环境设置**: 准备环境变量和工作目录
4. **二进制准备**: 确保任务二进制文件可用且可执行

### 2. 连接管理器初始化

```rust
// 使用 secret 验证器初始化连接管理器
let connection_manager = ConnectionManager::new_with_validator(
    config,
    Box::new(|secret: &str| -> bool {
        // 增强的 secret 验证逻辑
        !secret.is_empty() && secret.len() >= 8
    })
);
```

**特性:**
- **端口管理**: 自动端口分配和冲突解决
- **Secret 验证**: 增强的基于令牌的身份验证
- **连接池**: 高效的连接资源管理
- **事件监控**: 实时连接事件跟踪

### 3. 进程启动

```rust
// 使用通信参数启动进程
let mut command = Command::new(&binary_path);
command
    .env("SPEAR_COMMUNICATION_PORT", port.to_string())
    .env("SPEAR_INSTANCE_ID", instance_id)
    .env("SPEAR_SECRET", secret)
    .env("SPEAR_COMMUNICATION_TYPE", "tcp");
```

**环境变量:**
- `SPEAR_COMMUNICATION_PORT`: 通信的 TCP 端口
- `SPEAR_INSTANCE_ID`: 唯一实例标识符
- `SPEAR_SECRET`: 身份验证令牌
- `SPEAR_COMMUNICATION_TYPE`: 通信协议类型

## 连接建立流程

### 1. 服务端监听

```rust
// ConnectionManager 开始监听
connection_manager.start_listening(port).await?;
```

**过程:**
1. **TCP 监听器**: 绑定到分配的端口
2. **事件处理器**: 启动连接事件处理
3. **身份验证设置**: 准备 secret 验证
4. **监控**: 初始化连接跟踪

### 2. 客户端连接

运行时实例发起连接:

```rust
// 实例连接到 spearlet
let stream = TcpStream::connect(format!("127.0.0.1:{}", port)).await?;
```

### 3. 身份验证握手

#### 步骤 1: 身份验证请求
```rust
let auth_request = AuthRequest {
    instance_id: "instance-123".to_string(),
    token: "secret-token".to_string(),
    client_version: "1.0.0".to_string(),
    client_type: "process".to_string(),
    extra_params: HashMap::new(),
};

let message = SpearMessage {
    message_type: MessageType::AuthRequest,
    request_id: 12345,
    timestamp: SystemTime::now(),
    payload: serde_json::to_vec(&auth_request)?,
    version: 1,
};
```

#### 步骤 2: Secret 验证
```rust
// 服务器验证 secret
let is_valid = secret_validator(&auth_request.token);
if !is_valid {
    return Err("Authentication failed");
}
```

#### 步骤 3: 身份验证响应
```rust
let auth_response = AuthResponse {
    success: true,
    session_id: "session-456".to_string(),
    error_message: None,
};
```

### 4. 连接状态管理

```rust
// 成功身份验证后更新连接状态
let connection_state = ConnectionState {
    connection_id: "conn-789".to_string(),
    instance_id: Some("instance-123".to_string()),
    remote_addr: peer_addr,
    connected_at: Instant::now(),
    last_activity: Instant::now(),
    authenticated: true,
    client_type: Some("process".to_string()),
    client_version: Some("1.0.0".to_string()),
    session_id: Some("session-456".to_string()),
    status: ConnectionStatus::Active,
    heartbeat_sequence: 0,
};
```

## 事件流程和监控

### 连接事件

1. **连接事件**
```rust
ConnectionEvent::Connected {
    connection_id: "conn-789".to_string(),
    remote_addr: peer_addr,
}
```

2. **身份验证事件**
```rust
ConnectionEvent::Authenticated {
    connection_id: "conn-789".to_string(),
    instance_id: "instance-123".to_string(),
    client_type: "process".to_string(),
}
```

3. **消息事件**
```rust
ConnectionEvent::MessageReceived {
    connection_id: "conn-789".to_string(),
    message: spear_message,
}
```

### 监控集成

```rust
// 记录连接事件用于监控
monitoring_service.record_connection_event(
    connection_id.clone(),
    ConnectionEvent::Connected { connection_id, remote_addr }
).await;

// 记录消息事件
monitoring_service.record_message_event(
    connection_id,
    MessageType::AuthRequest,
    MessageDirection::Incoming,
    message_size,
    processing_time
).await;
```

## 错误处理和恢复

### 连接失败

1. **端口冲突**: 自动端口重新分配
2. **身份验证失败**: 详细错误报告
3. **网络问题**: 连接重试机制
4. **超时处理**: 可配置的超时值

### 恢复机制

```rust
// 自动重连逻辑
if connection_lost {
    for attempt in 1..=max_retries {
        match establish_connection().await {
            Ok(_) => break,
            Err(e) => {
                warn!("连接尝试 {} 失败: {}", attempt, e);
                tokio::time::sleep(retry_delay).await;
            }
        }
    }
}
```

## 安全考虑

### 身份验证安全
- **令牌验证**: 最小长度和复杂性要求
- **会话管理**: 每个连接的唯一会话 ID
- **超时保护**: 自动会话过期

### 网络安全
- **本地绑定**: 默认绑定到 localhost 以确保安全
- **端口隔离**: 实例特定的端口分配
- **消息验证**: 协议级消息验证

## 性能优化

### 连接池
- **可重用连接**: 最小化连接开销
- **资源限制**: 可配置的连接限制
- **负载均衡**: 在实例间分配连接

### 监控优化
- **异步处理**: 非阻塞事件处理
- **批量操作**: 高效的指标聚合
- **内存管理**: 有界事件队列

## 配置

### ConnectionManagerConfig
```rust
pub struct ConnectionManagerConfig {
    pub max_connections: usize,
    pub connection_timeout: Duration,
    pub heartbeat_interval: Duration,
    pub max_message_size: usize,
    pub enable_monitoring: bool,
}
```

### MonitoringConfig
```rust
pub struct MonitoringConfig {
    pub enabled: bool,
    pub enable_connection_tracking: bool,
    pub enable_message_tracking: bool,
    pub enable_performance_tracking: bool,
    pub metrics_buffer_size: usize,
}
```

## 测试和验证

### 单元测试
- **连接管理器**: 端口分配、身份验证、状态管理
- **监控服务**: 事件记录、指标聚合
- **协议处理**: 消息序列化、验证

### 集成测试
- **端到端流程**: 从实例启动到通信的完整流程
- **错误场景**: 网络故障、身份验证错误
- **性能测试**: 连接吞吐量、延迟测量

## 结论

实例启动和连接建立流程为 spearlet 与运行时实例之间的通信提供了坚实的基础。实现包括:

- **可靠的连接管理**: 自动端口分配和连接处理
- **安全的身份验证**: 基于令牌的身份验证和验证
- **全面的监控**: 实时事件跟踪和性能指标
- **错误恢复**: 强大的错误处理和恢复机制
- **性能优化**: 高效的资源利用和连接池

这种架构确保了 Spear 执行系统的可靠、安全和高性能的通信通道。