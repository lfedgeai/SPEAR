//! Communication abstraction layer for runtime execution
//! 运行时执行的通信抽象层
//!
//! This module provides a unified communication interface for different runtime types,
//! abstracting away the underlying transport mechanisms (Unix sockets, TCP, gRPC, etc.)
//!
//! 此模块为不同的运行时类型提供统一的通信接口，
//! 抽象了底层传输机制（Unix socket、TCP、gRPC 等）

use async_trait::async_trait;
use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::time::{Duration, SystemTime};

pub mod channel;
pub mod connection_manager;
pub mod factory;
pub mod monitoring;
pub mod protocol;
pub mod transport;

#[cfg(test)]
pub mod secret_validation_test;

// Re-export key types for convenience
// 为方便使用重新导出关键类型
pub use channel::{GrpcChannel, TcpChannel, UnixSocketChannel};
pub use connection_manager::{
    ConnectionEvent, ConnectionManager, ConnectionManagerConfig, ConnectionState,
};
pub use factory::{CommunicationFactory, CommunicationStrategy, CommunicationStrategyBuilder};
pub use monitoring::{
    ConnectionMetrics, MessageDirection, MessageMetrics, MonitoringConfig, MonitoringEvent,
    MonitoringService, PerformanceEvent, SystemMetrics,
};
pub use protocol::{
    AuthRequest, AuthResponse, ExecuteRequest, ExecuteResponse, MessageType, SpearMessage,
};
pub use transport::{Transport, TransportConfig, TransportFactory, TransportStats};

/// Core communication result type
/// 核心通信结果类型
pub type CommunicationResult<T> = Result<T, CommunicationError>;

/// Communication error types
/// 通信错误类型
#[derive(Debug, thiserror::Error)]
pub enum CommunicationError {
    #[error("Connection failed: {message}")]
    ConnectionFailed { message: String },

    #[error("Send failed: {message}")]
    SendFailed { message: String },

    #[error("Receive failed: {message}")]
    ReceiveFailed { message: String },

    #[error("Timeout after {timeout_ms}ms")]
    Timeout { timeout_ms: u64 },

    #[error("Channel closed")]
    ChannelClosed,

    #[error("Invalid message format: {message}")]
    InvalidMessage { message: String },

    #[error("Authentication failed: {message}")]
    AuthenticationFailed { message: String },

    #[error("Transport error: {message}")]
    TransportError { message: String },

    #[error("Configuration error: {message}")]
    ConfigurationError { message: String },

    #[error("IO error: {0}")]
    Io(#[from] std::io::Error),

    #[error("Serialization error: {0}")]
    Serialization(#[from] serde_json::Error),

    /// Unsupported transport type
    /// 不支持的传输类型
    #[error("Unsupported transport type: {transport_type}")]
    UnsupportedTransport { transport_type: String },

    /// Unsupported runtime type
    /// 不支持的运行时类型
    #[error("Unsupported runtime type: {runtime_type}")]
    UnsupportedRuntime { runtime_type: String },

    /// Unsupported channel type
    /// 不支持的通道类型
    #[error("Unsupported channel type: {channel_type}")]
    UnsupportedChannel { channel_type: String },

    /// Channel creation failed
    /// 通道创建失败
    #[error(
        "Failed to create channel for runtime {runtime_type}, attempted types: {attempted_types:?}"
    )]
    ChannelCreationFailed {
        runtime_type: String,
        attempted_types: Vec<String>,
    },

    /// Invalid configuration
    /// 无效配置
    #[error("Invalid configuration: {message}")]
    InvalidConfiguration { message: String },
}

/// Runtime message types for communication
/// 用于通信的运行时消息类型
#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum RuntimeMessage {
    /// Function execution request
    /// 函数执行请求
    ExecutionRequest {
        request_id: String,
        function_name: String,
        payload: Vec<u8>,
        timeout_ms: u64,
        metadata: HashMap<String, String>,
    },

    /// Function execution response
    /// 函数执行响应
    ExecutionResponse {
        request_id: String,
        result: ExecutionResult,
        execution_time_ms: u64,
        metadata: HashMap<String, String>,
    },

    /// Health check request
    /// 健康检查请求
    HealthCheck { request_id: String },

    /// Health check response
    /// 健康检查响应
    HealthResponse {
        request_id: String,
        status: HealthStatus,
        metrics: HealthMetrics,
    },

    /// Shutdown signal
    /// 关闭信号
    Shutdown { graceful: bool, timeout_ms: u64 },

    /// Acknowledgment message
    /// 确认消息
    Ack { request_id: String },

    /// Error message
    /// 错误消息
    Error {
        request_id: Option<String>,
        error: String,
        error_code: u32,
    },
}

/// Execution result for function calls
/// 函数调用的执行结果
#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum ExecutionResult {
    /// Successful execution with result data
    /// 成功执行并返回结果数据
    Success { data: Vec<u8> },

    /// Execution failed with error
    /// 执行失败并返回错误
    Error {
        message: String,
        error_code: u32,
        stack_trace: Option<String>,
    },

    /// Execution timeout
    /// 执行超时
    Timeout { timeout_ms: u64 },
}

/// Health status of runtime instance
/// 运行时实例的健康状态
#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum HealthStatus {
    /// Instance is healthy and ready
    /// 实例健康且就绪
    Healthy,

    /// Instance is starting up
    /// 实例正在启动
    Starting,

    /// Instance is degraded but functional
    /// 实例性能下降但仍可用
    Degraded { reason: String },

    /// Instance is unhealthy
    /// 实例不健康
    Unhealthy { reason: String },

    /// Instance is shutting down
    /// 实例正在关闭
    Shutting,
}

/// Health metrics for monitoring
/// 用于监控的健康指标
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct HealthMetrics {
    /// CPU usage percentage (0-100)
    /// CPU 使用率百分比 (0-100)
    pub cpu_usage: f64,

    /// Memory usage in bytes
    /// 内存使用量（字节）
    pub memory_usage: u64,

    /// Number of active requests
    /// 活跃请求数量
    pub active_requests: u32,

    /// Total requests processed
    /// 已处理的总请求数
    pub total_requests: u64,

    /// Average response time in milliseconds
    /// 平均响应时间（毫秒）
    pub avg_response_time_ms: f64,

    /// Last update timestamp
    /// 最后更新时间戳
    pub timestamp: SystemTime,
}

/// Runtime instance identifier
/// 运行时实例标识符
#[derive(Debug, Clone, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub struct RuntimeInstanceId {
    /// Runtime type
    /// 运行时类型
    pub runtime_type: crate::spearlet::execution::RuntimeType,

    /// Unique instance identifier within the runtime type
    /// 运行时类型内的唯一实例标识符
    pub instance_id: String,

    /// Optional namespace for grouping instances
    /// 用于分组实例的可选命名空间
    pub namespace: Option<String>,
}

impl RuntimeInstanceId {
    /// Create a new runtime instance identifier
    /// 创建新的运行时实例标识符
    pub fn new(runtime_type: crate::spearlet::execution::RuntimeType, instance_id: String) -> Self {
        Self {
            runtime_type,
            instance_id,
            namespace: None,
        }
    }

    /// Create a new runtime instance identifier with namespace
    /// 创建带命名空间的新运行时实例标识符
    pub fn with_namespace(
        runtime_type: crate::spearlet::execution::RuntimeType,
        instance_id: String,
        namespace: String,
    ) -> Self {
        Self {
            runtime_type,
            instance_id,
            namespace: Some(namespace),
        }
    }

    /// Get the full identifier string
    /// 获取完整的标识符字符串
    pub fn full_id(&self) -> String {
        match &self.namespace {
            Some(ns) => format!("{}:{}:{}", ns, self.runtime_type.as_str(), self.instance_id),
            None => format!("{}:{}", self.runtime_type.as_str(), self.instance_id),
        }
    }
}

impl std::fmt::Display for RuntimeInstanceId {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "{}", self.full_id())
    }
}

/// Runtime instance status
/// 运行时实例状态
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub enum InstanceStatus {
    /// Instance is starting up
    /// 实例正在启动
    Starting,

    /// Instance is running and healthy
    /// 实例正在运行且健康
    Running,

    /// Instance is degraded but still functional
    /// 实例性能下降但仍可运行
    Degraded { reason: String },

    /// Instance is unhealthy
    /// 实例不健康
    Unhealthy { reason: String },

    /// Instance is shutting down
    /// 实例正在关闭
    Stopping,

    /// Instance has stopped
    /// 实例已停止
    Stopped,
}

/// Runtime instance metadata
/// 运行时实例元数据
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct InstanceMetadata {
    /// Instance identifier
    /// 实例标识符
    pub instance_id: RuntimeInstanceId,

    /// Current status
    /// 当前状态
    pub status: InstanceStatus,

    /// Creation timestamp
    /// 创建时间戳
    pub created_at: SystemTime,

    /// Last health check timestamp
    /// 最后健康检查时间戳
    pub last_health_check: Option<SystemTime>,

    /// Instance configuration
    /// 实例配置
    pub config: ChannelConfig,

    /// Additional metadata
    /// 额外元数据
    pub extra_metadata: HashMap<String, String>,
}

impl InstanceMetadata {
    /// Create new instance metadata
    /// 创建新的实例元数据
    pub fn new(instance_id: RuntimeInstanceId, config: ChannelConfig) -> Self {
        Self {
            instance_id,
            status: InstanceStatus::Starting,
            created_at: SystemTime::now(),
            last_health_check: None,
            config,
            extra_metadata: HashMap::new(),
        }
    }

    /// Update instance status
    /// 更新实例状态
    pub fn update_status(&mut self, status: InstanceStatus) {
        self.status = status;
    }

    /// Update last health check timestamp
    /// 更新最后健康检查时间戳
    pub fn update_health_check(&mut self) {
        self.last_health_check = Some(SystemTime::now());
    }

    /// Check if instance is healthy
    /// 检查实例是否健康
    pub fn is_healthy(&self) -> bool {
        matches!(self.status, InstanceStatus::Running)
    }

    /// Check if instance is active (not stopped)
    /// 检查实例是否活跃（未停止）
    pub fn is_active(&self) -> bool {
        !matches!(self.status, InstanceStatus::Stopped)
    }
}

/// Communication channel configuration
/// 通信通道配置
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ChannelConfig {
    /// Runtime instance this channel connects to
    /// 此通道连接的运行时实例
    pub instance_id: RuntimeInstanceId,

    /// Channel type (unix_socket, tcp, grpc)
    /// 通道类型 (unix_socket, tcp, grpc)
    pub channel_type: String,

    /// Connection address or path
    /// 连接地址或路径
    pub address: String,

    /// Connection timeout in milliseconds
    /// 连接超时时间（毫秒）
    pub connect_timeout_ms: u64,

    /// Request timeout in milliseconds
    /// 请求超时时间（毫秒）
    pub request_timeout_ms: u64,

    /// Keep-alive interval in milliseconds
    /// 保活间隔时间（毫秒）
    pub keepalive_interval_ms: u64,

    /// Maximum retry attempts
    /// 最大重试次数
    pub max_retries: u32,

    /// Additional configuration parameters
    /// 额外的配置参数
    pub extra_config: HashMap<String, String>,
}

impl Default for ChannelConfig {
    fn default() -> Self {
        Self {
            instance_id: RuntimeInstanceId::new(
                crate::spearlet::execution::RuntimeType::Process,
                "default".to_string(),
            ),
            channel_type: "unix".to_string(),
            address: "/tmp/spear-default.sock".to_string(),
            connect_timeout_ms: 5000,
            request_timeout_ms: 30000,
            keepalive_interval_ms: 10000,
            max_retries: 3,
            extra_config: HashMap::new(),
        }
    }
}

/// Core communication channel trait
/// 核心通信通道 trait
#[async_trait]
pub trait CommunicationChannel: Send + Sync {
    /// Send a message through the channel
    /// 通过通道发送消息
    async fn send(&self, message: RuntimeMessage) -> CommunicationResult<()>;

    /// Receive a message from the channel
    /// 从通道接收消息
    async fn receive(&self) -> CommunicationResult<RuntimeMessage>;

    /// Send a request and wait for response
    /// 发送请求并等待响应
    async fn request_response(
        &self,
        request: RuntimeMessage,
        timeout: Duration,
    ) -> CommunicationResult<RuntimeMessage>;

    /// Check if the channel is connected
    /// 检查通道是否已连接
    async fn is_connected(&self) -> bool;

    /// Close the communication channel
    /// 关闭通信通道
    async fn close(&self) -> CommunicationResult<()>;

    /// Get channel statistics
    /// 获取通道统计信息
    async fn get_stats(&self) -> CommunicationResult<ChannelStats>;
}

/// Channel statistics for monitoring
/// 用于监控的通道统计信息
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ChannelStats {
    /// Total messages sent
    /// 已发送的总消息数
    pub messages_sent: u64,

    /// Total messages received
    /// 已接收的总消息数
    pub messages_received: u64,

    /// Total bytes sent
    /// 已发送的总字节数
    pub bytes_sent: u64,

    /// Total bytes received
    /// 已接收的总字节数
    pub bytes_received: u64,

    /// Number of connection errors
    /// 连接错误数量
    pub connection_errors: u32,

    /// Number of timeout errors
    /// 超时错误数量
    pub timeout_errors: u32,

    /// Average latency in milliseconds
    /// 平均延迟（毫秒）
    pub avg_latency_ms: f64,

    /// Channel uptime in seconds
    /// 通道运行时间（秒）
    pub uptime_seconds: u64,
}

impl Default for ChannelStats {
    fn default() -> Self {
        Self {
            messages_sent: 0,
            messages_received: 0,
            bytes_sent: 0,
            bytes_received: 0,
            connection_errors: 0,
            timeout_errors: 0,
            avg_latency_ms: 0.0,
            uptime_seconds: 0,
        }
    }
}
