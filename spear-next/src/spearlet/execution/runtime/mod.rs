//! Runtime Module
//! 运行时模块
//!
//! This module provides runtime abstractions and implementations for different execution environments.
//! 该模块为不同的执行环境提供运行时抽象和实现。
//!
//! ## Supported Runtimes / 支持的运行时
//! - **Process**: Native process execution / 原生进程执行  
//! - **WASM**: WebAssembly execution / WebAssembly 执行
//! - **Kubernetes**: Kubernetes-based execution / 基于 Kubernetes 的执行
//!
//! ## Features / 特性
//! - Runtime abstraction with common interface / 具有通用接口的运行时抽象
//! - Resource management and monitoring / 资源管理和监控
//! - Health checks and metrics collection / 健康检查和指标收集
//! - Dynamic scaling and configuration / 动态扩缩容和配置

use crate::spearlet::execution::communication::{
    ConnectionManager, ConnectionManagerConfig, MessageType, SpearMessage,
};
use crate::spearlet::execution::instance::{InstanceConfig, InstanceResourceLimits, TaskInstance};
use crate::spearlet::execution::{ExecutionError, ExecutionResult};
use async_trait::async_trait;
use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::net::SocketAddr;
use std::sync::Arc;
use tracing::info;

// Re-export runtime implementations / 重新导出运行时实现
pub mod kubernetes;
pub mod process;
pub mod wasm;
#[cfg(feature = "wasmedge")]
pub mod wasm_hostcalls;

pub use kubernetes::{KubernetesConfig, KubernetesRuntime};
pub use process::{ProcessConfig, ProcessRuntime};
pub use wasm::{WasmConfig, WasmRuntime};

// Note: ExecutionMode, ExecutionStatus, RuntimeExecutionResponse, and ExecutionContext are defined in this module
// 注意：ExecutionMode、ExecutionStatus、RuntimeExecutionResponse 和 ExecutionContext 在此模块中定义

/// Runtime type enumeration / 运行时类型枚举
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub enum RuntimeType {
    /// Native process runtime / 原生进程运行时
    Process,
    /// WebAssembly runtime / WebAssembly 运行时
    Wasm,
    /// Kubernetes runtime / Kubernetes 运行时
    Kubernetes,
}

impl RuntimeType {
    /// Convert runtime type to string representation
    /// 将运行时类型转换为字符串表示
    pub fn as_str(&self) -> &'static str {
        match self {
            RuntimeType::Process => "process",
            RuntimeType::Wasm => "wasm",
            RuntimeType::Kubernetes => "kubernetes",
        }
    }
}

/// Runtime configuration / 运行时配置
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct RuntimeConfig {
    /// Runtime type / 运行时类型
    pub runtime_type: RuntimeType,
    /// Runtime-specific settings / 运行时特定设置
    pub settings: HashMap<String, serde_json::Value>,
    /// Global environment variables / 全局环境变量
    pub global_environment: HashMap<String, String>,
    /// Full spearlet configuration snapshot / 完整的Spearlet配置快照
    pub spearlet_config: Option<crate::spearlet::config::SpearletConfig>,
    /// Resource pool configuration / 资源池配置
    pub resource_pool: ResourcePoolConfig,
}

// Removed RuntimeEndpoints in favor of passing full SpearletConfig

/// Resource pool configuration / 资源池配置
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ResourcePoolConfig {
    /// Maximum number of concurrent instances / 最大并发实例数
    pub max_concurrent_instances: u32,
    /// Instance creation timeout in milliseconds / 实例创建超时时间（毫秒）
    pub instance_creation_timeout_ms: u64,
    /// Instance cleanup timeout in milliseconds / 实例清理超时时间（毫秒）
    pub instance_cleanup_timeout_ms: u64,
    /// Resource monitoring interval in milliseconds / 资源监控间隔（毫秒）
    pub monitoring_interval_ms: u64,
}

impl Default for ResourcePoolConfig {
    fn default() -> Self {
        Self {
            max_concurrent_instances: 100,
            instance_creation_timeout_ms: 30000, // 30 seconds
            instance_cleanup_timeout_ms: 10000,  // 10 seconds
            monitoring_interval_ms: 5000,        // 5 seconds
        }
    }
}

/// Runtime execution context / 运行时执行上下文
#[derive(Debug, Clone)]
pub struct ExecutionContext {
    /// Execution ID / 执行 ID
    pub execution_id: String,
    /// Request payload / 请求负载
    pub payload: Vec<u8>,
    /// Request headers / 请求头
    pub headers: HashMap<String, String>,
    /// Execution timeout in milliseconds / 执行超时时间（毫秒）
    pub timeout_ms: u64,
    /// Additional context data / 额外上下文数据
    pub context_data: HashMap<String, serde_json::Value>,
}

/// Runtime listening configuration / 运行时监听配置
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct RuntimeListeningConfig {
    /// Whether to enable listening mode / 是否启用监听模式
    pub enabled: bool,
    /// Connection manager configuration / 连接管理器配置
    pub connection_config: ConnectionManagerConfig,
    /// Authentication configuration / 认证配置
    pub auth_config: AuthConfig,
    /// Message handling configuration / 消息处理配置
    pub message_config: MessageHandlingConfig,
}

/// Authentication configuration / 认证配置
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AuthConfig {
    /// Whether authentication is required / 是否需要认证
    pub required: bool,
    /// Authentication timeout in seconds / 认证超时时间（秒）
    pub timeout_secs: u64,
    /// Valid tokens / 有效令牌
    pub valid_tokens: Vec<String>,
    /// Token validation strategy / 令牌验证策略
    pub validation_strategy: TokenValidationStrategy,
}

/// Token validation strategy / 令牌验证策略
#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum TokenValidationStrategy {
    /// Static token list / 静态令牌列表
    Static,
    /// Dynamic validation via callback / 通过回调动态验证
    Dynamic,
    /// External service validation / 外部服务验证
    External { endpoint: String },
}

/// Message handling configuration / 消息处理配置
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct MessageHandlingConfig {
    /// Maximum concurrent messages / 最大并发消息数
    pub max_concurrent_messages: u32,
    /// Message processing timeout in seconds / 消息处理超时时间（秒）
    pub processing_timeout_secs: u64,
    /// Whether to enable message queuing / 是否启用消息队列
    pub enable_queuing: bool,
    /// Queue size limit / 队列大小限制
    pub queue_size_limit: usize,
}

impl Default for RuntimeListeningConfig {
    fn default() -> Self {
        Self {
            enabled: false,
            connection_config: ConnectionManagerConfig::default(),
            auth_config: AuthConfig::default(),
            message_config: MessageHandlingConfig::default(),
        }
    }
}

impl Default for AuthConfig {
    fn default() -> Self {
        Self {
            required: true,
            timeout_secs: 30,
            valid_tokens: Vec::new(),
            validation_strategy: TokenValidationStrategy::Static,
        }
    }
}

impl Default for MessageHandlingConfig {
    fn default() -> Self {
        Self {
            max_concurrent_messages: 100,
            processing_timeout_secs: 300,
            enable_queuing: true,
            queue_size_limit: 1000,
        }
    }
}

/// Function execution mode / 函数执行模式
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub enum ExecutionMode {
    /// Unknown mode / 未知模式
    Unknown = 0,
    /// Synchronous execution / 同步执行
    Sync = 1,
    /// Asynchronous execution / 异步执行
    Async = 2,
    /// Streaming execution / 流式执行
    Stream = 3,
}

impl Default for ExecutionMode {
    fn default() -> Self {
        ExecutionMode::Unknown
    }
}

/// Function execution status / 函数执行状态
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub enum ExecutionStatus {
    /// Unknown status / 未知状态
    Unknown = 0,
    /// Pending execution / 等待执行
    Pending = 1,
    /// Currently running / 正在运行
    Running = 2,
    /// Completed successfully / 成功完成
    Completed = 3,
    /// Failed with error / 执行失败
    Failed = 4,
    /// Cancelled by user / 用户取消
    Cancelled = 5,
    /// Execution timeout / 执行超时
    Timeout = 6,
}

impl Default for ExecutionStatus {
    fn default() -> Self {
        ExecutionStatus::Unknown
    }
}

/// Runtime execution result / 运行时执行结果
/// This represents the pure execution result from a runtime, without any transport-layer concerns
/// 这表示来自运行时的纯执行结果，不涉及任何传输层关注点
#[derive(Debug, Clone)]
pub struct RuntimeExecutionResponse {
    /// Execution result data / 执行结果数据
    pub data: Vec<u8>,
    /// Execution duration in milliseconds / 执行持续时间（毫秒）
    pub duration_ms: u64,
    /// Runtime-specific metadata / 运行时特定的元数据
    pub metadata: HashMap<String, serde_json::Value>,

    // === Execution lifecycle fields === / === 执行生命周期字段 ===
    /// Execution mode (sync/async/stream) / 执行模式（同步/异步/流式）
    pub execution_mode: ExecutionMode,
    /// Current execution status / 当前执行状态
    pub execution_status: ExecutionStatus,
    /// Unique execution identifier / 唯一执行标识符
    pub execution_id: String,
    /// Task identifier for tracking / 任务标识符用于跟踪
    pub task_id: Option<String>,

    // === Async execution fields === / === 异步执行字段 ===
    /// Status tracking endpoint for async execution / 异步执行的状态跟踪端点
    pub status_endpoint: Option<String>,
    /// Estimated completion time in milliseconds / 预计完成时间（毫秒）
    pub estimated_completion_ms: Option<u64>,

    // === Error handling === / === 错误处理 ===
    /// Error information if execution failed / 执行失败时的错误信息
    pub error: Option<RuntimeExecutionError>,
}

/// Runtime execution error / 运行时执行错误
#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum RuntimeExecutionError {
    /// Instance not found / 实例未找到
    InstanceNotFound { instance_id: String },
    /// Instance not ready / 实例未就绪
    InstanceNotReady { instance_id: String },
    /// Execution timeout / 执行超时
    ExecutionTimeout { timeout_ms: u64 },
    /// Resource limit exceeded / 资源限制超出
    ResourceLimitExceeded { resource: String, limit: String },
    /// Configuration error / 配置错误
    ConfigurationError { message: String },
    /// Runtime error / 运行时错误
    RuntimeError { message: String },
    /// IO error / IO错误
    IoError { message: String },
    /// Serialization error / 序列化错误
    SerializationError { message: String },
    /// Unsupported operation / 不支持的操作
    UnsupportedOperation {
        operation: String,
        runtime_type: String,
    },
}

impl Default for RuntimeExecutionResponse {
    fn default() -> Self {
        Self {
            data: Vec::new(),
            duration_ms: 0,
            metadata: HashMap::new(),
            execution_mode: ExecutionMode::default(),
            execution_status: ExecutionStatus::default(),
            execution_id: String::new(),
            task_id: None,
            status_endpoint: None,
            estimated_completion_ms: None,
            error: None,
        }
    }
}

impl RuntimeExecutionResponse {
    /// Create a new sync execution response / 创建新的同步执行响应
    pub fn new_sync(execution_id: String, data: Vec<u8>, duration_ms: u64) -> Self {
        Self {
            execution_id,
            data,
            duration_ms,
            execution_mode: ExecutionMode::Sync,
            execution_status: ExecutionStatus::Completed,
            ..Default::default()
        }
    }

    /// Create a new async execution response / 创建新的异步执行响应
    pub fn new_async(
        execution_id: String,
        task_id: Option<String>,
        status_endpoint: String,
        estimated_completion_ms: Option<u64>,
    ) -> Self {
        Self {
            execution_id,
            task_id,
            status_endpoint: Some(status_endpoint),
            estimated_completion_ms,
            execution_mode: ExecutionMode::Async,
            execution_status: ExecutionStatus::Pending,
            ..Default::default()
        }
    }

    /// Create a failed execution response / 创建失败的执行响应
    pub fn new_failed(
        execution_id: String,
        execution_mode: ExecutionMode,
        error: RuntimeExecutionError,
        duration_ms: u64,
    ) -> Self {
        Self {
            execution_id,
            execution_mode,
            execution_status: ExecutionStatus::Failed,
            duration_ms,
            error: Some(error),
            ..Default::default()
        }
    }

    /// Check if execution is in progress / 检查执行是否正在进行
    pub fn is_in_progress(&self) -> bool {
        matches!(
            self.execution_status,
            ExecutionStatus::Pending | ExecutionStatus::Running
        )
    }

    /// Check if execution has failed / 检查执行是否失败
    pub fn has_failed(&self) -> bool {
        self.error.is_some()
            || matches!(
                self.execution_status,
                ExecutionStatus::Failed | ExecutionStatus::Cancelled | ExecutionStatus::Timeout
            )
    }

    /// Check if execution completed successfully / 检查执行是否成功完成
    pub fn is_successful(&self) -> bool {
        self.execution_status == ExecutionStatus::Completed && self.error.is_none()
    }

    /// Check if execution is completed (either successfully or with error) / 检查执行是否已完成（成功或失败）
    pub fn is_completed(&self) -> bool {
        matches!(
            self.execution_status,
            ExecutionStatus::Completed
                | ExecutionStatus::Failed
                | ExecutionStatus::Cancelled
                | ExecutionStatus::Timeout
        )
    }
}

/// Runtime trait for execution environments / 执行环境的运行时特征
#[async_trait]
pub trait Runtime: Send + Sync {
    /// Get runtime type / 获取运行时类型
    fn runtime_type(&self) -> RuntimeType;

    /// Create a new instance / 创建新实例
    async fn create_instance(&self, config: &InstanceConfig) -> ExecutionResult<Arc<TaskInstance>>;

    /// Start an instance / 启动实例
    async fn start_instance(&self, instance: &Arc<TaskInstance>) -> ExecutionResult<()>;

    /// Stop an instance / 停止实例
    async fn stop_instance(&self, instance: &Arc<TaskInstance>) -> ExecutionResult<()>;

    /// Execute a request on an instance / 在实例上执行请求
    async fn execute(
        &self,
        instance: &Arc<TaskInstance>,
        context: ExecutionContext,
    ) -> ExecutionResult<RuntimeExecutionResponse>;

    /// Perform health check on an instance / 对实例执行健康检查
    async fn health_check(&self, instance: &Arc<TaskInstance>) -> ExecutionResult<bool>;

    /// Get instance metrics / 获取实例指标
    async fn get_metrics(
        &self,
        instance: &Arc<TaskInstance>,
    ) -> ExecutionResult<HashMap<String, serde_json::Value>>;

    /// Scale instance resources / 扩缩容实例资源
    async fn scale_instance(
        &self,
        instance: &Arc<TaskInstance>,
        new_limits: &InstanceResourceLimits,
    ) -> ExecutionResult<()>;

    /// Cleanup instance resources / 清理实例资源
    async fn cleanup_instance(&self, instance: &Arc<TaskInstance>) -> ExecutionResult<()>;

    /// Validate runtime configuration / 验证运行时配置
    fn validate_config(&self, config: &InstanceConfig) -> ExecutionResult<()>;

    /// Get runtime capabilities / 获取运行时能力
    fn get_capabilities(&self) -> RuntimeCapabilities;

    /// Get currently running function name if any / 获取当前正在运行的函数名（如有）
    async fn get_running_function(
        &self,
        _instance: &Arc<TaskInstance>,
    ) -> ExecutionResult<Option<String>> {
        Ok(None)
    }

    // 监听模式相关方法 / Listening mode related methods

    /// Check if runtime supports listening mode / 检查运行时是否支持监听模式
    fn supports_listening_mode(&self) -> bool {
        false
    }

    /// Start listening mode for the runtime / 启动运行时的监听模式
    /// Returns the listening address if successful / 成功时返回监听地址
    async fn start_listening(
        &self,
        _config: &RuntimeListeningConfig,
    ) -> ExecutionResult<Option<SocketAddr>> {
        if !self.supports_listening_mode() {
            return Err(ExecutionError::NotSupported {
                operation: format!(
                    "listening mode for runtime type '{}'",
                    self.runtime_type().as_str()
                ),
            });
        }
        Ok(None)
    }

    /// Stop listening mode / 停止监听模式
    async fn stop_listening(&self) -> ExecutionResult<()> {
        if !self.supports_listening_mode() {
            return Err(ExecutionError::NotSupported {
                operation: format!(
                    "listening mode for runtime type '{}'",
                    self.runtime_type().as_str()
                ),
            });
        }
        Ok(())
    }

    /// Get listening status / 获取监听状态
    async fn get_listening_status(&self) -> ExecutionResult<ListeningStatus> {
        if !self.supports_listening_mode() {
            return Ok(ListeningStatus::NotSupported);
        }
        Ok(ListeningStatus::Stopped)
    }

    /// Handle incoming message from agent / 处理来自agent的消息
    async fn handle_agent_message(
        &self,
        _instance_id: &str,
        _message: SpearMessage,
    ) -> ExecutionResult<Option<SpearMessage>> {
        if !self.supports_listening_mode() {
            return Err(ExecutionError::NotSupported {
                operation: format!(
                    "message handling for runtime type '{}'",
                    self.runtime_type().as_str()
                ),
            });
        }
        Ok(None)
    }

    /// Get connection manager if available / 获取连接管理器（如果可用）
    async fn get_connection_manager(&self) -> ExecutionResult<Option<Arc<ConnectionManager>>> {
        Ok(None)
    }

    /// Register message handler / 注册消息处理器
    async fn register_message_handler(
        &self,
        _handler: Box<dyn MessageHandler>,
    ) -> ExecutionResult<()> {
        if !self.supports_listening_mode() {
            return Err(ExecutionError::NotSupported {
                operation: format!(
                    "message handler registration for runtime type '{}'",
                    self.runtime_type().as_str()
                ),
            });
        }
        Ok(())
    }
}

/// Listening status / 监听状态
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub enum ListeningStatus {
    /// Not supported / 不支持
    NotSupported,
    /// Stopped / 已停止
    Stopped,
    /// Starting / 启动中
    Starting,
    /// Active / 活跃
    Active {
        /// Listening address / 监听地址
        address: SocketAddr,
        /// Number of active connections / 活跃连接数
        active_connections: usize,
        /// Start time / 启动时间
        started_at: std::time::SystemTime,
    },
    /// Error / 错误
    Error {
        /// Error message / 错误消息
        message: String,
    },
}

/// Message handler trait / 消息处理器特征
#[async_trait]
pub trait MessageHandler: Send + Sync {
    /// Handle incoming message / 处理传入消息
    async fn handle_message(
        &self,
        instance_id: &str,
        message: SpearMessage,
    ) -> ExecutionResult<Option<SpearMessage>>;

    /// Get handler name / 获取处理器名称
    fn handler_name(&self) -> &str;

    /// Check if handler can process message type / 检查处理器是否能处理消息类型
    fn can_handle(&self, message_type: &MessageType) -> bool;
}

/// Runtime capabilities / 运行时能力
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct RuntimeCapabilities {
    /// Supports dynamic scaling / 支持动态扩缩容
    pub supports_scaling: bool,
    /// Supports health checks / 支持健康检查
    pub supports_health_checks: bool,
    /// Supports metrics collection / 支持指标收集
    pub supports_metrics: bool,
    /// Supports hot reloading / 支持热重载
    pub supports_hot_reload: bool,
    /// Supports persistent storage / 支持持久化存储
    pub supports_persistent_storage: bool,
    /// Supports network isolation / 支持网络隔离
    pub supports_network_isolation: bool,
    /// Maximum concurrent instances / 最大并发实例数
    pub max_concurrent_instances: u32,
    /// Supported protocols / 支持的协议
    pub supported_protocols: Vec<String>,
}

impl Default for RuntimeCapabilities {
    fn default() -> Self {
        Self {
            supports_scaling: true,
            supports_health_checks: true,
            supports_metrics: true,
            supports_hot_reload: false,
            supports_persistent_storage: false,
            supports_network_isolation: false,
            max_concurrent_instances: 100,
            supported_protocols: vec!["HTTP".to_string()],
        }
    }
}

/// Runtime factory for creating runtime instances / 用于创建运行时实例的运行时工厂
pub struct RuntimeFactory;

impl RuntimeFactory {
    /// Create a runtime instance based on configuration / 根据配置创建运行时实例
    pub fn create_runtime(config: &RuntimeConfig) -> ExecutionResult<Box<dyn Runtime>> {
        match config.runtime_type {
            RuntimeType::Process => {
                let process_runtime = process::ProcessRuntime::new(config)?;
                Ok(Box::new(process_runtime))
            }
            RuntimeType::Wasm => {
                let wasm_runtime = wasm::WasmRuntime::new(config)?;
                Ok(Box::new(wasm_runtime))
            }
            RuntimeType::Kubernetes => {
                let kubernetes_runtime = kubernetes::KubernetesRuntime::new(config)?;
                Ok(Box::new(kubernetes_runtime))
            }
        }
    }

    /// Get available runtime types / 获取可用的运行时类型
    pub fn available_runtimes() -> Vec<RuntimeType> {
        vec![
            RuntimeType::Process,
            RuntimeType::Wasm,
            RuntimeType::Kubernetes,
        ]
    }

    /// Check if a runtime type is supported / 检查是否支持某个运行时类型
    pub fn is_runtime_supported(runtime_type: &RuntimeType) -> bool {
        Self::available_runtimes().contains(runtime_type)
    }
}

/// Runtime manager for managing multiple runtime instances / 用于管理多个运行时实例的运行时管理器
pub struct RuntimeManager {
    /// Runtime instances by type / 按类型分类的运行时实例
    runtimes: HashMap<RuntimeType, Box<dyn Runtime>>,
}

impl std::fmt::Debug for RuntimeManager {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("RuntimeManager")
            .field("runtimes", &self.runtimes.keys().collect::<Vec<_>>())
            .finish()
    }
}

impl RuntimeManager {
    /// Create a new runtime manager / 创建新的运行时管理器
    pub fn new() -> Self {
        Self {
            runtimes: HashMap::new(),
        }
    }

    /// Register a runtime / 注册运行时
    pub fn register_runtime(
        &mut self,
        runtime_type: RuntimeType,
        runtime: Box<dyn Runtime>,
    ) -> ExecutionResult<()> {
        if self.runtimes.contains_key(&runtime_type) {
            return Err(ExecutionError::InvalidConfiguration {
                message: format!("Runtime {:?} is already registered", runtime_type),
            });
        }

        self.runtimes.insert(runtime_type, runtime);
        Ok(())
    }

    /// Get a runtime by type / 根据类型获取运行时
    pub fn get_runtime(&self, runtime_type: &RuntimeType) -> Option<&dyn Runtime> {
        self.runtimes.get(runtime_type).map(|r| r.as_ref())
    }

    /// List all registered runtime types / 列出所有已注册的运行时类型
    pub fn list_runtime_types(&self) -> Vec<RuntimeType> {
        self.runtimes.keys().cloned().collect()
    }

    /// Initialize all runtimes with their configurations / 使用配置初始化所有运行时
    pub fn initialize_runtimes(&mut self, configs: Vec<RuntimeConfig>) -> ExecutionResult<()> {
        info!(
            "Initializing runtimes: count={} types={:?}",
            configs.len(),
            configs.iter().map(|c| c.runtime_type).collect::<Vec<_>>()
        );
        for config in configs {
            let runtime = RuntimeFactory::create_runtime(&config)?;
            self.register_runtime(config.runtime_type, runtime)?;
        }
        info!("Initialized runtimes: {:?}", self.list_runtime_types());
        Ok(())
    }
}

impl Default for RuntimeManager {
    fn default() -> Self {
        Self::new()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_runtime_factory() {
        assert!(RuntimeFactory::is_runtime_supported(&RuntimeType::Process));
        assert!(RuntimeFactory::is_runtime_supported(&RuntimeType::Wasm));
        assert!(RuntimeFactory::is_runtime_supported(
            &RuntimeType::Kubernetes
        ));

        let available = RuntimeFactory::available_runtimes();
        assert_eq!(available.len(), 3);
        assert!(available.contains(&RuntimeType::Process));
        assert!(available.contains(&RuntimeType::Wasm));
        assert!(available.contains(&RuntimeType::Kubernetes));
    }

    #[test]
    fn test_runtime_manager() {
        let mut manager = RuntimeManager::new();
        assert!(manager.list_runtime_types().is_empty());
        assert!(manager.get_runtime(&RuntimeType::Process).is_none());
    }
}
