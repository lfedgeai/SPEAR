//! Execution Module - Artifact-Task-Instance Architecture
//! 执行模块 - Artifact-Task-Instance 架构
//!
//! This module implements a three-tier execution architecture:
//! 该模块实现了三层执行架构：
//!
//! - **Artifact**: A deployable unit containing one or more tasks
//! - **Artifact**: 包含一个或多个任务的可部署单元
//! - **Task**: A logical execution unit within an artifact
//! - **Task**: artifact 内的逻辑执行单元
//! - **Instance**: A physical execution instance of a task
//! - **Instance**: 任务的物理执行实例
//!
//! ## Architecture Overview / 架构概览
//!
//! ```text
//! Artifact (1) ──┬── Task (1) ──┬── Instance (1)
//!                │               ├── Instance (2)
//!                │               └── Instance (N)
//!                ├── Task (2) ──┬── Instance (1)
//!                │               └── Instance (M)
//!                └── Task (N)
//! ```
//!
//! ## Features / 特性
//!
//! - **Parallel Execution**: Multiple instances per task for load balancing
//! - **并行执行**: 每个任务多个实例用于负载均衡
//! - **Runtime Abstraction**: Support for Kubernetes, Process, and WASM runtimes
//! - **运行时抽象**: 支持 Kubernetes、Process 和 WASM 运行时
//! - **High Concurrency**: Lock-free data structures using DashMap and atomic operations
//! - **高并发**: 使用 DashMap 和原子操作的无锁数据结构
//! - **Resource Management**: Automatic scaling and lifecycle management
//! - **资源管理**: 自动扩缩容和生命周期管理

pub mod ai;
pub mod artifact;
pub mod artifact_fetch;
pub mod communication;
pub mod host_api;
pub mod http_adapter;
pub mod instance;
pub mod manager;
pub mod pool;
pub mod runtime;
pub mod scheduler;
pub mod task;

// Re-export commonly used types / 重新导出常用类型
pub use artifact::{Artifact, ArtifactId, ArtifactSpec, ArtifactStatus};
pub use communication::{
    ChannelConfig, ChannelStats, CommunicationChannel, CommunicationFactory, CommunicationStrategy,
    RuntimeMessage,
};
pub use instance::{InstanceId, InstanceMetrics, InstanceStatus, TaskInstance};
pub use manager::{ExecutionStatistics, TaskExecutionManager, TaskExecutionManagerConfig};
pub use pool::{InstancePool, InstancePoolConfig, PoolMetrics, ScalingAction, ScalingDecision};
pub use runtime::{Runtime, RuntimeConfig, RuntimeType};
pub use scheduler::{InstanceScheduler, SchedulingDecision, SchedulingMetrics, SchedulingPolicy};
pub use task::{Task, TaskId, TaskSpec, TaskStatus, TaskType};

use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::time::SystemTime;

// Execution context / 执行上下文
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ExecutionContext {
    /// Request ID / 请求 ID
    pub request_id: String,
    /// Input data / 输入数据
    pub input_data: Vec<u8>,
    /// Timeout in milliseconds / 超时时间（毫秒）
    pub timeout_ms: u64,
    /// Metadata / 元数据
    pub metadata: HashMap<String, String>,
}

// Execution response / 执行响应
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ExecutionResponse {
    /// Request ID / 请求 ID
    pub request_id: String,
    /// Output data / 输出数据
    pub output_data: Vec<u8>,
    /// Execution status / 执行状态
    pub status: String,
    /// Error message if any / 错误消息（如果有）
    pub error_message: Option<String>,
    /// Execution time in milliseconds / 执行时间（毫秒）
    pub execution_time_ms: u64,
    /// Metadata / 元数据
    pub metadata: HashMap<String, String>,
    /// Timestamp / 时间戳
    pub timestamp: SystemTime,
}

impl ExecutionResponse {
    /// Check if execution is completed / 检查执行是否完成
    pub fn is_completed(&self) -> bool {
        self.status == "completed" || self.status == "failed"
    }

    /// Check if execution is successful / 检查执行是否成功
    pub fn is_successful(&self) -> bool {
        self.status == "completed" && self.error_message.is_none()
    }
}

// Execution result type / 执行结果类型
pub type ExecutionResult<T> = Result<T, ExecutionError>;

/// Execution error types / 执行错误类型
#[derive(Debug, thiserror::Error)]
pub enum ExecutionError {
    #[error("Artifact not found: {id}")]
    ArtifactNotFound { id: String },

    #[error("Task not found: {id}")]
    TaskNotFound { id: String },

    #[error("Instance not found: {id}")]
    InstanceNotFound { id: String },

    #[error("Runtime not found: {runtime_type}")]
    RuntimeNotFound { runtime_type: String },

    #[error("Runtime error: {message}")]
    RuntimeError { message: String },

    #[error("Scheduling error: {message}")]
    SchedulingError { message: String },

    #[error("Resource exhausted: {message}")]
    ResourceExhausted { message: String },

    #[error("Invalid configuration: {message}")]
    InvalidConfiguration { message: String },

    #[error("Invalid request: {message}")]
    InvalidRequest { message: String },

    #[error("Execution timeout: {timeout_ms}ms")]
    ExecutionTimeout { timeout_ms: u64 },

    #[error("Instance creation failed: {message}")]
    InstanceCreationFailed { message: String },

    #[error("Instance startup failed: {message}")]
    InstanceStartupFailed { message: String },

    #[error("Health check failed: {message}")]
    HealthCheckFailed { message: String },

    #[error("Concurrent modification detected")]
    ConcurrentModification,

    #[error("Operation not supported: {operation}")]
    NotSupported { operation: String },

    #[error("IO error: {0}")]
    Io(#[from] std::io::Error),

    #[error("Serialization error: {0}")]
    Serialization(#[from] serde_json::Error),
}
