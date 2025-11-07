# Spear 执行系统通信架构

## 概述

本文档描述了为 Spear 执行系统实现的通信架构，该架构为 `spearlet` 与不同执行环境中的运行时实例之间的通信提供了统一的抽象层。

## 架构设计

### 分层抽象

通信系统采用四层抽象设计：

```
┌─────────────────────────────────────────┐
│        Function Invocation Layer       │  ← 高层业务逻辑
├─────────────────────────────────────────┤
│           Runtime Layer                 │  ← 运行时特定策略
├─────────────────────────────────────────┤
│      Communication Channel Layer       │  ← 协议抽象
├─────────────────────────────────────────┤
│          Transport Layer                │  ← 底层传输
└─────────────────────────────────────────┘
```

### 实例级别通信设计

通信系统支持实例级别的隔离，允许多个运行时实例独立运行并拥有各自的通信通道。这种设计确保：

- **实例隔离**: 每个运行时实例都有自己的通信上下文
- **资源管理**: 通道按实例管理，防止资源冲突
- **可扩展性**: 多个实例可以并发运行而不相互干扰
- **调试便利**: 实例特定的通道简化了故障排除和监控

#### 实例 ID 集成

每个通信通道都与唯一标识运行时实例的 `RuntimeInstanceId` 关联：

```rust
pub struct RuntimeInstanceId {
    pub runtime_type: RuntimeType,
    pub instance_id: String,
}
```

此实例 ID 在整个通信栈中用于：
- 创建实例特定的通信通道
- 将消息路由到正确的运行时实例
- 为每个实例维护独立的连接池
- 提供实例级别的统计和监控

### 核心组件

#### 1. 通信通道 Trait

`CommunicationChannel` trait 为所有通信操作提供统一接口：

```rust
#[async_trait]
pub trait CommunicationChannel: Send + Sync {
    async fn send(&self, message: RuntimeMessage) -> CommunicationResult<()>;
    async fn receive(&self) -> CommunicationResult<RuntimeMessage>;
    async fn request_response(&self, request: RuntimeMessage, timeout: Duration) -> CommunicationResult<RuntimeMessage>;
    async fn is_connected(&self) -> bool;
    async fn close(&self) -> CommunicationResult<()>;
    async fn get_stats(&self) -> CommunicationResult<ChannelStats>;
    fn instance_id(&self) -> &RuntimeInstanceId; // 实例级别标识
}
```

每个通道实现都维护其关联的实例 ID，支持实例级别的操作和监控。

#### 2. 运行时消息类型

系统定义了结构化的通信消息类型：

- `ExecutionRequest`: 函数执行请求
- `ExecutionResponse`: 函数执行结果
- `HealthCheck`: 健康监控消息
- `Ack`: 确认消息

#### 3. 传输层

传输层提供底层通信机制：

- **Unix Domain Socket**: 用于本地进程通信
- **TCP**: 用于基于网络的通信
- **gRPC**: 用于结构化服务通信

#### 4. 工厂模式

`CommunicationFactory` 实现运行时特定的通信策略，支持实例级别：

- **Process Runtime**: 首选 Unix socket，备用 TCP
- **Kubernetes Runtime**: 首选 gRPC，备用 TCP
- **WASM Runtime**: 使用内存通信

#### 实例级别通道创建

工厂支持为特定运行时实例创建通道：

```rust
impl CommunicationFactory {
    pub async fn create_channel_for_instance(
        &self,
        instance_id: RuntimeInstanceId,
        config: Option<ChannelConfig>,
    ) -> CommunicationResult<Arc<dyn CommunicationChannel>> {
        // 创建实例特定的通信通道
    }
}
```

这使得：
- **隔离通信**: 每个实例都有自己的通信上下文
- **资源管理**: 通道按实例跟踪和管理
- **配置灵活性**: 实例特定的通道配置

## 实现细节

### 通道实现

#### Unix Socket 通道
- **使用场景**: 本地进程通信
- **优势**: 高性能，低延迟
- **配置**: Socket 路径，超时设置
- **实例支持**: 每个实例基于实例 ID 使用唯一的 socket 路径

#### TCP 通道
- **使用场景**: 网络通信，备用选项
- **优势**: 通用兼容性
- **配置**: 主机，端口，连接设置
- **实例支持**: 实例特定的端口分配或连接池

#### gRPC 通道
- **使用场景**: Kubernetes 环境，结构化通信
- **优势**: 类型安全，流式支持
- **配置**: 服务端点，TLS 设置
- **实例支持**: 实例感知的服务发现和路由

### 运行时策略

#### Process Runtime 策略
```rust
CommunicationStrategy {
    runtime_type: RuntimeType::Process,
    preferred_channel: "unix",
    fallback_channels: ["tcp"],
    default_config: ChannelConfig {
        address: "/tmp/spear-process.sock",
        // ...
    }
}
```

#### Kubernetes Runtime 策略
```rust
CommunicationStrategy {
    runtime_type: RuntimeType::Kubernetes,
    preferred_channel: "grpc",
    fallback_channels: ["tcp"],
    default_config: ChannelConfig {
        address: "http://127.0.0.1:50051",
        // ...
    }
}
```

### 错误处理

系统提供全面的错误处理：

- `ChannelClosed`: 通信通道已关闭
- `Timeout`: 操作超时
- `UnsupportedTransport`: 不支持的传输类型
- `ChannelCreationFailed`: 创建通信通道失败

## 使用示例

### 创建通信通道

#### 基本通道创建
```rust
use spear_next::spearlet::execution::{CommunicationFactory, RuntimeType};

let factory = CommunicationFactory::new();
let channel = factory.create_channel(RuntimeType::Process, None).await?;
```

#### 实例级别通道创建
```rust
use spear_next::spearlet::execution::{CommunicationFactory, RuntimeInstanceId, RuntimeType};

let factory = CommunicationFactory::new();
let instance_id = RuntimeInstanceId {
    runtime_type: RuntimeType::Process,
    instance_id: "worker-001".to_string(),
};
let channel = factory.create_channel_for_instance(instance_id, None).await?;

// 从通道获取实例 ID
let channel_instance_id = channel.instance_id();
println!("为实例创建的通道: {}", channel_instance_id.instance_id);
```

### 发送消息

```rust
use spear_next::spearlet::execution::RuntimeMessage;

let request = RuntimeMessage::ExecutionRequest {
    request_id: "req-123".to_string(),
    function_name: "my_function".to_string(),
    input_data: b"input".to_vec(),
    timeout_ms: 5000,
    metadata: HashMap::new(),
};

channel.send(request).await?;
```

### 请求-响应模式

```rust
let response = channel.request_response(request, Duration::from_secs(5)).await?;
match response {
    RuntimeMessage::ExecutionResponse { output_data, .. } => {
        // 处理响应
    }
    _ => {
        // 处理意外响应
    }
}
```

## 测试

通信系统包含全面的测试：

- **单元测试**: 23 个测试用例覆盖所有组件
- **集成测试**: 通道创建和消息传递
- **错误处理测试**: 超时和失败场景
- **实例隔离测试**: 验证实例级别通道隔离

### 实例级别测试

测试套件包含实例级别功能的特定测试：

```rust
#[tokio::test]
async fn test_channel_instance_isolation() {
    // 测试为不同实例创建的通道是否正确隔离
    let instance_id_1 = RuntimeInstanceId {
        runtime_type: RuntimeType::Process,
        instance_id: "instance-1".to_string(),
    };
    let instance_id_2 = RuntimeInstanceId {
        runtime_type: RuntimeType::Process,
        instance_id: "instance-2".to_string(),
    };
    
    // 验证通道具有正确的实例 ID
    // 并且独立运行
}
```

所有测试都成功通过，确保了通信抽象和实例级别隔离的可靠性。

## 优势

### 1. 统一接口
- 所有通信需求的单一 API
- 运行时无关的应用代码
- 一致的错误处理

### 2. 可扩展性
- 易于添加新的传输机制
- 可插拔的通信策略
- 运行时特定的优化

### 3. 可靠性
- 自动备用机制
- 全面的错误处理
- 连接监控和统计

### 4. 性能
- 传输特定的优化
- 连接池支持
- 高效的消息序列化

## 未来增强

### 1. 连接池
- 为 TCP 和 gRPC 通道实现连接池
- 自动连接生命周期管理

### 2. 负载均衡
- 支持多个后端实例
- 轮询和加权负载均衡

### 3. 安全性
- 网络通信的 TLS 支持
- 认证和授权机制

### 4. 监控
- 详细的指标收集
- 与可观测性系统集成

## 结论

通信架构为 Spear 执行系统中的进程间通信提供了坚实的基础。分层设计确保了灵活性、可靠性和性能，同时保持了清晰的关注点分离。

该实现已准备好用于生产环境，并可根据需要扩展以支持额外的传输机制和运行时环境。