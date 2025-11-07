//! Task Instance Implementation
//! Task Instance 实现
//!
//! A TaskInstance represents a physical execution instance of a task.
//! TaskInstance 表示任务的物理执行实例。

use super::{TaskId, RuntimeType};
use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::sync::atomic::{AtomicU64, Ordering};
use std::sync::Arc;
use std::time::{Duration, SystemTime};
use uuid::Uuid;

/// Unique identifier for a task instance / Task Instance 的唯一标识符
pub type InstanceId = String;

/// Instance status enumeration / Instance 状态枚举
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub enum InstanceStatus {
    /// Instance is being created / 正在创建 Instance
    Creating,
    /// Instance is starting up / Instance 正在启动
    Starting,
    /// Instance is ready to accept requests / Instance 准备接受请求
    Ready,
    /// Instance is currently processing requests / Instance 正在处理请求
    Running,
    /// Instance is busy (at capacity) / Instance 繁忙（达到容量上限）
    Busy,
    /// Instance is unhealthy / Instance 不健康
    Unhealthy,
    /// Instance is being stopped / 正在停止 Instance
    Stopping,
    /// Instance has stopped / Instance 已停止
    Stopped,
    /// Instance encountered an error / Instance 遇到错误
    Error(String),
}

/// Instance health status / Instance 健康状态
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub enum HealthStatus {
    /// Health status unknown / 健康状态未知
    Unknown,
    /// Instance is healthy / Instance 健康
    Healthy,
    /// Instance is unhealthy / Instance 不健康
    Unhealthy,
    /// Health check is in progress / 健康检查进行中
    Checking,
}

/// Instance metrics / Instance 指标
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct InstanceMetrics {
    /// Total number of requests processed / 处理的总请求数
    pub total_requests: u64,
    /// Number of successful requests / 成功请求数
    pub successful_requests: u64,
    /// Number of failed requests / 失败请求数
    pub failed_requests: u64,
    /// Current number of active requests / 当前活跃请求数
    pub active_requests: u32,
    /// Average request processing time in milliseconds / 平均请求处理时间（毫秒）
    pub avg_request_time_ms: f64,
    /// 95th percentile request processing time in milliseconds / 95分位请求处理时间（毫秒）
    pub p95_request_time_ms: f64,
    /// Current CPU usage percentage / 当前 CPU 使用率百分比
    pub cpu_usage_percent: f64,
    /// Current memory usage in bytes / 当前内存使用量（字节）
    pub memory_usage_bytes: u64,
    /// Network bytes received / 网络接收字节数
    pub network_bytes_in: u64,
    /// Network bytes sent / 网络发送字节数
    pub network_bytes_out: u64,
    /// Last request timestamp / 最后请求时间戳
    pub last_request_time: Option<SystemTime>,
    /// Last health check timestamp / 最后健康检查时间戳
    pub last_health_check_time: Option<SystemTime>,
    /// Health check consecutive failures / 健康检查连续失败次数
    pub health_check_failures: u32,
    /// Health check consecutive successes / 健康检查连续成功次数
    pub health_check_successes: u32,
}

impl Default for InstanceMetrics {
    fn default() -> Self {
        Self {
            total_requests: 0,
            successful_requests: 0,
            failed_requests: 0,
            active_requests: 0,
            avg_request_time_ms: 0.0,
            p95_request_time_ms: 0.0,
            cpu_usage_percent: 0.0,
            memory_usage_bytes: 0,
            network_bytes_in: 0,
            network_bytes_out: 0,
            last_request_time: None,
            last_health_check_time: None,
            health_check_failures: 0,
            health_check_successes: 0,
        }
    }
}

/// Instance configuration / Instance 配置
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct InstanceConfig {
    /// Runtime type / 运行时类型
    pub runtime_type: RuntimeType,
    /// Runtime-specific configuration / 运行时特定配置
    pub runtime_config: HashMap<String, serde_json::Value>,
    /// Environment variables / 环境变量
    pub environment: HashMap<String, String>,
    /// Resource limits / 资源限制
    pub resource_limits: InstanceResourceLimits,
    /// Network configuration / 网络配置
    pub network_config: NetworkConfig,
    /// Maximum concurrent requests / 最大并发请求数
    pub max_concurrent_requests: u32,
    /// Request timeout in milliseconds / 请求超时时间（毫秒）
    pub request_timeout_ms: u64,
}

/// Instance resource limits / Instance 资源限制
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct InstanceResourceLimits {
    /// Maximum CPU cores / 最大 CPU 核心数
    pub max_cpu_cores: f64,
    /// Maximum memory in bytes / 最大内存字节数
    pub max_memory_bytes: u64,
    /// Maximum disk space in bytes / 最大磁盘空间字节数
    pub max_disk_bytes: u64,
    /// Maximum network bandwidth in bytes per second / 最大网络带宽（字节/秒）
    pub max_network_bps: u64,
}

impl Default for InstanceResourceLimits {
    fn default() -> Self {
        Self {
            max_cpu_cores: 0.5,
            max_memory_bytes: 256 * 1024 * 1024, // 256MB
            max_disk_bytes: 512 * 1024 * 1024,   // 512MB
            max_network_bps: 50 * 1024 * 1024,   // 50MB/s
        }
    }
}

/// Network configuration / 网络配置
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct NetworkConfig {
    /// Bind address / 绑定地址
    pub bind_address: String,
    /// Port number / 端口号
    pub port: u16,
    /// Protocol (HTTP, gRPC, etc.) / 协议（HTTP、gRPC 等）
    pub protocol: String,
    /// TLS configuration / TLS 配置
    pub tls_enabled: bool,
    /// Additional network settings / 额外网络设置
    pub additional_settings: HashMap<String, String>,
}

impl Default for NetworkConfig {
    fn default() -> Self {
        Self {
            bind_address: "0.0.0.0".to_string(),
            port: 0, // Will be assigned dynamically / 将动态分配
            protocol: "HTTP".to_string(),
            tls_enabled: false,
            additional_settings: HashMap::new(),
        }
    }
}

/// Task instance implementation / Task Instance 实现
#[derive(Debug)]
pub struct TaskInstance {
    /// Unique identifier / 唯一标识符
    pub id: InstanceId,
    /// Parent task ID / 父 Task ID
    pub task_id: TaskId,
    /// Instance configuration / Instance 配置
    pub config: InstanceConfig,
    /// Current status / 当前状态
    pub status: Arc<parking_lot::RwLock<InstanceStatus>>,
    /// Health status / 健康状态
    pub health_status: Arc<parking_lot::RwLock<HealthStatus>>,
    /// Instance metrics / Instance 指标
    pub metrics: Arc<parking_lot::RwLock<InstanceMetrics>>,
    /// Runtime handle (opaque pointer to runtime-specific data) / 运行时句柄（指向运行时特定数据的不透明指针）
    pub runtime_handle: Arc<parking_lot::RwLock<Option<Box<dyn std::any::Any + Send + Sync>>>>,
    /// Instance secret for authentication / 实例认证密钥
    pub secret: Arc<parking_lot::RwLock<Option<String>>>,
    /// Listening address for communication / 通信监听地址
    pub listening_address: Arc<parking_lot::RwLock<Option<String>>>,
    /// Creation timestamp / 创建时间戳
    pub created_at: SystemTime,
    /// Last updated timestamp / 最后更新时间戳
    pub updated_at: Arc<parking_lot::RwLock<SystemTime>>,
    /// Request counter for generating unique request IDs / 请求计数器用于生成唯一请求ID
    request_counter: AtomicU64,
}

impl TaskInstance {
    /// Create a new task instance / 创建新的 Task Instance
    pub fn new(task_id: TaskId, config: InstanceConfig) -> Self {
        let id = format!("inst-{}", Uuid::new_v4());
        let now = SystemTime::now();
        
        Self {
            id,
            task_id,
            config,
            status: Arc::new(parking_lot::RwLock::new(InstanceStatus::Creating)),
            health_status: Arc::new(parking_lot::RwLock::new(HealthStatus::Unknown)),
            metrics: Arc::new(parking_lot::RwLock::new(InstanceMetrics::default())),
            runtime_handle: Arc::new(parking_lot::RwLock::new(None)),
            secret: Arc::new(parking_lot::RwLock::new(None)),
            listening_address: Arc::new(parking_lot::RwLock::new(None)),
            created_at: now,
            updated_at: Arc::new(parking_lot::RwLock::new(now)),
            request_counter: AtomicU64::new(0),
        }
    }

    /// Get instance ID / 获取 Instance ID
    pub fn id(&self) -> &str {
        &self.id
    }

    /// Get parent task ID / 获取父 Task ID
    pub fn task_id(&self) -> &str {
        &self.task_id
    }

    /// Get current status / 获取当前状态
    pub fn status(&self) -> InstanceStatus {
        self.status.read().clone()
    }

    /// Update status / 更新状态
    pub fn set_status(&self, status: InstanceStatus) {
        *self.status.write() = status;
        *self.updated_at.write() = SystemTime::now();
    }

    /// Get health status / 获取健康状态
    pub fn health_status(&self) -> HealthStatus {
        self.health_status.read().clone()
    }

    /// Update health status / 更新健康状态
    pub fn set_health_status(&self, health_status: HealthStatus) {
        *self.health_status.write() = health_status;
        *self.updated_at.write() = SystemTime::now();
    }

    /// Update metrics / 更新指标
    pub fn update_metrics<F>(&self, updater: F)
    where
        F: FnOnce(&mut InstanceMetrics),
    {
        let mut metrics = self.metrics.write();
        updater(&mut metrics);
        *self.updated_at.write() = SystemTime::now();
    }

    /// Get current metrics / 获取当前指标
    pub fn get_metrics(&self) -> InstanceMetrics {
        self.metrics.read().clone()
    }

    /// Set runtime handle / 设置运行时句柄
    pub fn set_runtime_handle<T: std::any::Any + Send + Sync>(&self, handle: T) {
        *self.runtime_handle.write() = Some(Box::new(handle));
    }

    /// Get runtime handle / 获取运行时句柄
    pub fn get_runtime_handle<T: std::any::Any + Send + Sync>(&self) -> Option<Arc<T>> {
        self.runtime_handle
            .read()
            .as_ref()
            .and_then(|handle| handle.downcast_ref::<Arc<T>>().cloned())
    }

    /// Set instance secret / 设置实例密钥
    pub fn set_secret(&self, secret: String) {
        *self.secret.write() = Some(secret);
        *self.updated_at.write() = SystemTime::now();
    }

    /// Get instance secret / 获取实例密钥
    pub fn get_secret(&self) -> Option<String> {
        self.secret.read().clone()
    }

    /// Set listening address / 设置监听地址
    pub fn set_listening_address(&self, address: String) {
        *self.listening_address.write() = Some(address);
        *self.updated_at.write() = SystemTime::now();
    }

    /// Get listening address / 获取监听地址
    pub fn get_listening_address(&self) -> Option<String> {
        self.listening_address.read().clone()
    }

    /// Generate a unique request ID / 生成唯一的请求 ID
    pub fn generate_request_id(&self) -> String {
        let counter = self.request_counter.fetch_add(1, Ordering::SeqCst);
        format!("{}-req-{}", self.id, counter)
    }

    /// Check if instance is ready to accept requests / 检查实例是否准备接受请求
    pub fn is_ready(&self) -> bool {
        matches!(self.status(), InstanceStatus::Ready | InstanceStatus::Running)
            && matches!(self.health_status(), HealthStatus::Healthy | HealthStatus::Unknown)
    }

    /// Check if instance is at capacity / 检查实例是否达到容量上限
    pub fn is_at_capacity(&self) -> bool {
        let metrics = self.get_metrics();
        metrics.active_requests >= self.config.max_concurrent_requests
    }

    /// Check if instance is healthy / 检查实例是否健康
    pub fn is_healthy(&self) -> bool {
        matches!(self.health_status(), HealthStatus::Healthy)
            && !matches!(self.status(), InstanceStatus::Error(_) | InstanceStatus::Unhealthy)
    }

    /// Get instance load (0.0 to 1.0) / 获取实例负载（0.0 到 1.0）
    pub fn get_load(&self) -> f64 {
        let metrics = self.get_metrics();
        if self.config.max_concurrent_requests == 0 {
            0.0
        } else {
            metrics.active_requests as f64 / self.config.max_concurrent_requests as f64
        }
    }

    /// Get instance age / 获取实例年龄
    pub fn age(&self) -> Duration {
        SystemTime::now()
            .duration_since(self.created_at)
            .unwrap_or_default()
    }

    /// Get time since last update / 获取自上次更新以来的时间
    pub fn time_since_update(&self) -> Duration {
        let last_update = *self.updated_at.read();
        SystemTime::now()
            .duration_since(last_update)
            .unwrap_or_default()
    }

    /// Get time since last request / 获取自上次请求以来的时间
    pub fn time_since_last_request(&self) -> Option<Duration> {
        let metrics = self.metrics.read();
        metrics.last_request_time.map(|last_request| {
            SystemTime::now()
                .duration_since(last_request)
                .unwrap_or_default()
        })
    }

    /// Check if instance should be considered for removal due to inactivity / 检查实例是否因不活跃而应被考虑移除
    pub fn is_idle(&self, idle_threshold: Duration) -> bool {
        if let Some(time_since_request) = self.time_since_last_request() {
            time_since_request > idle_threshold && self.get_metrics().active_requests == 0
        } else {
            // No requests processed yet, check creation time / 尚未处理任何请求，检查创建时间
            self.age() > idle_threshold
        }
    }

    /// Record a request start / 记录请求开始
    pub fn record_request_start(&self) {
        self.update_metrics(|metrics| {
            metrics.active_requests += 1;
            metrics.total_requests += 1;
            metrics.last_request_time = Some(SystemTime::now());
        });
    }

    /// Record a request completion / 记录请求完成
    pub fn record_request_completion(&self, success: bool, duration_ms: f64) {
        self.update_metrics(|metrics| {
            if metrics.active_requests > 0 {
                metrics.active_requests -= 1;
            }
            
            if success {
                metrics.successful_requests += 1;
            } else {
                metrics.failed_requests += 1;
            }
            
            // Update average request time using exponential moving average / 使用指数移动平均更新平均请求时间
            if metrics.avg_request_time_ms == 0.0 {
                metrics.avg_request_time_ms = duration_ms;
            } else {
                metrics.avg_request_time_ms = 0.9 * metrics.avg_request_time_ms + 0.1 * duration_ms;
            }
            
            // Update P95 (simplified approximation) / 更新 P95（简化近似）
            if duration_ms > metrics.p95_request_time_ms {
                metrics.p95_request_time_ms = 0.95 * metrics.p95_request_time_ms + 0.05 * duration_ms;
            }
        });
    }

    /// Record a health check result / 记录健康检查结果
    pub fn record_health_check(&self, success: bool) {
        self.update_metrics(|metrics| {
            metrics.last_health_check_time = Some(SystemTime::now());
            
            if success {
                metrics.health_check_successes += 1;
                metrics.health_check_failures = 0;
            } else {
                metrics.health_check_failures += 1;
                metrics.health_check_successes = 0;
            }
        });
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_instance_creation() {
        let config = InstanceConfig {
            runtime_type: RuntimeType::Kubernetes,
            runtime_config: HashMap::new(),
            environment: HashMap::new(),
            resource_limits: InstanceResourceLimits::default(),
            network_config: NetworkConfig::default(),
            max_concurrent_requests: 100,
            request_timeout_ms: 30000,
        };

        let instance = TaskInstance::new("task-123".to_string(), config);
        assert!(instance.id.starts_with("inst-"));
        assert_eq!(instance.task_id, "task-123");
        assert_eq!(instance.status(), InstanceStatus::Creating);
        assert_eq!(instance.health_status(), HealthStatus::Unknown);
    }

    #[test]
    fn test_request_tracking() {
        let config = InstanceConfig {
            runtime_type: RuntimeType::Kubernetes,
            runtime_config: HashMap::new(),
            environment: HashMap::new(),
            resource_limits: InstanceResourceLimits::default(),
            network_config: NetworkConfig::default(),
            max_concurrent_requests: 100,
            request_timeout_ms: 30000,
        };

        let instance = TaskInstance::new("task-123".to_string(), config);
        
        // Record request start / 记录请求开始
        instance.record_request_start();
        let metrics = instance.get_metrics();
        assert_eq!(metrics.active_requests, 1);
        assert_eq!(metrics.total_requests, 1);
        
        // Record request completion / 记录请求完成
        instance.record_request_completion(true, 150.0);
        let metrics = instance.get_metrics();
        assert_eq!(metrics.active_requests, 0);
        assert_eq!(metrics.successful_requests, 1);
        assert_eq!(metrics.failed_requests, 0);
        assert_eq!(metrics.avg_request_time_ms, 150.0);
    }

    #[test]
    fn test_load_calculation() {
        let config = InstanceConfig {
            runtime_type: RuntimeType::Kubernetes,
            runtime_config: HashMap::new(),
            environment: HashMap::new(),
            resource_limits: InstanceResourceLimits::default(),
            network_config: NetworkConfig::default(),
            max_concurrent_requests: 10,
            request_timeout_ms: 30000,
        };

        let instance = TaskInstance::new("task-123".to_string(), config);
        
        // No active requests / 没有活跃请求
        assert_eq!(instance.get_load(), 0.0);
        
        // 5 active requests out of 10 max / 10个最大请求中有5个活跃请求
        instance.update_metrics(|metrics| {
            metrics.active_requests = 5;
        });
        assert_eq!(instance.get_load(), 0.5);
        
        // At capacity / 达到容量上限
        instance.update_metrics(|metrics| {
            metrics.active_requests = 10;
        });
        assert_eq!(instance.get_load(), 1.0);
        assert!(instance.is_at_capacity());
    }

    #[test]
    fn test_health_check_tracking() {
        let config = InstanceConfig {
            runtime_type: RuntimeType::Kubernetes,
            runtime_config: HashMap::new(),
            environment: HashMap::new(),
            resource_limits: InstanceResourceLimits::default(),
            network_config: NetworkConfig::default(),
            max_concurrent_requests: 100,
            request_timeout_ms: 30000,
        };

        let instance = TaskInstance::new("task-123".to_string(), config);
        
        // Record successful health check / 记录成功的健康检查
        instance.record_health_check(true);
        let metrics = instance.get_metrics();
        assert_eq!(metrics.health_check_successes, 1);
        assert_eq!(metrics.health_check_failures, 0);
        
        // Record failed health check / 记录失败的健康检查
        instance.record_health_check(false);
        let metrics = instance.get_metrics();
        assert_eq!(metrics.health_check_successes, 0);
        assert_eq!(metrics.health_check_failures, 1);
    }
}