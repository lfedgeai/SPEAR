//! Process Runtime Implementation
//! 进程运行时实现
//!
//! This module provides native process-based execution runtime.
//! 该模块提供基于原生进程的执行运行时。

use super::{
    ExecutionContext, ListeningStatus, MessageHandler, Runtime, RuntimeCapabilities, RuntimeConfig,
    RuntimeExecutionResponse, RuntimeListeningConfig, RuntimeType,
};
use crate::spearlet::execution::{
    communication::{
        ConnectionManager, ConnectionManagerConfig, MessageDirection, MessageType,
        MonitoringConfig, MonitoringService, SpearMessage,
    },
    instance::{InstanceConfig, InstanceResourceLimits, TaskInstance},
    ExecutionError, ExecutionResult, InstanceStatus,
};
use async_trait::async_trait;
use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::net::SocketAddr;
use std::process::Stdio;
use std::sync::Arc;
use std::time::{Duration, Instant};
use tracing::{debug, info};

use tokio::process::{Child as TokioChild, Command};
use tokio::sync::{Mutex, RwLock};
use tokio::time::timeout;

/// Process runtime configuration / 进程运行时配置
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ProcessConfig {
    /// Working directory for processes / 进程工作目录
    pub working_directory: String,
    /// Default executable path / 默认可执行文件路径
    pub default_executable: String,
    /// Process isolation configuration / 进程隔离配置
    pub isolation_config: ProcessIsolationConfig,
    /// Resource monitoring configuration / 资源监控配置
    pub monitoring_config: ProcessMonitoringConfig,
    /// Security configuration / 安全配置
    pub security_config: ProcessSecurityConfig,
}

/// Process isolation configuration / 进程隔离配置
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ProcessIsolationConfig {
    /// Use process groups / 使用进程组
    pub use_process_groups: bool,
    /// Use separate namespaces (Linux only) / 使用独立命名空间（仅限 Linux）
    pub use_namespaces: bool,
    /// Use chroot jail / 使用 chroot 监狱
    pub use_chroot: bool,
    /// Chroot directory / chroot 目录
    pub chroot_directory: Option<String>,
    /// Use resource limits (ulimit) / 使用资源限制（ulimit）
    pub use_resource_limits: bool,
}

/// Process monitoring configuration / 进程监控配置
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ProcessMonitoringConfig {
    /// Monitor CPU usage / 监控 CPU 使用率
    pub monitor_cpu: bool,
    /// Monitor memory usage / 监控内存使用
    pub monitor_memory: bool,
    /// Monitor file descriptors / 监控文件描述符
    pub monitor_file_descriptors: bool,
    /// Monitoring interval in milliseconds / 监控间隔（毫秒）
    pub monitoring_interval_ms: u64,
}

/// Process security configuration / 进程安全配置
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ProcessSecurityConfig {
    /// Run as specific user / 以特定用户运行
    pub run_as_user: Option<String>,
    /// Run as specific group / 以特定组运行
    pub run_as_group: Option<String>,
    /// Drop privileges / 放弃权限
    pub drop_privileges: bool,
    /// Allowed system calls (seccomp) / 允许的系统调用（seccomp）
    pub allowed_syscalls: Vec<String>,
    /// Environment variable whitelist / 环境变量白名单
    pub env_whitelist: Vec<String>,
}

impl Default for ProcessConfig {
    fn default() -> Self {
        Self {
            working_directory: "/tmp/spearlet".to_string(),
            default_executable: "/bin/sh".to_string(),
            isolation_config: ProcessIsolationConfig {
                use_process_groups: true,
                use_namespaces: false,
                use_chroot: false,
                chroot_directory: None,
                use_resource_limits: true,
            },
            monitoring_config: ProcessMonitoringConfig {
                monitor_cpu: true,
                monitor_memory: true,
                monitor_file_descriptors: true,
                monitoring_interval_ms: 1000,
            },
            security_config: ProcessSecurityConfig {
                run_as_user: None,
                run_as_group: None,
                drop_privileges: true,
                allowed_syscalls: vec![],
                env_whitelist: vec!["PATH".to_string(), "HOME".to_string(), "USER".to_string()],
            },
        }
    }
}

/// Process handle / 进程句柄
#[derive(Debug)]
pub struct ProcessHandle {
    /// Process ID / 进程 ID
    pub pid: u32,
    /// Process command / 进程命令
    pub command: String,
    /// Process arguments / 进程参数
    pub args: Vec<String>,
    /// Process working directory / 进程工作目录
    pub working_directory: String,
    /// Process environment / 进程环境变量
    pub environment: HashMap<String, String>,
    /// Process start time / 进程启动时间
    pub start_time: std::time::SystemTime,
    /// Process child handle / 进程子句柄
    pub child: Arc<Mutex<Option<TokioChild>>>,
}

/// Process runtime implementation / 进程运行时实现
pub struct ProcessRuntime {
    /// Process configuration / 进程配置
    config: ProcessConfig,
    /// Runtime configuration / 运行时配置
    runtime_config: RuntimeConfig,
    /// Connection manager for listening mode / 监听模式的连接管理器
    connection_manager: Arc<RwLock<Option<Arc<ConnectionManager>>>>,
    /// Monitoring service / 监控服务
    monitoring_service: Arc<RwLock<Option<Arc<MonitoringService>>>>,
    /// Listening status / 监听状态
    listening_status: Arc<RwLock<ListeningStatus>>,
    /// Message handlers / 消息处理器
    message_handlers: Arc<RwLock<Vec<Box<dyn MessageHandler>>>>,
}

impl std::fmt::Debug for ProcessRuntime {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("ProcessRuntime")
            .field("config", &self.config)
            .field("runtime_config", &self.runtime_config)
            .field("listening_status", &"<listening_status>")
            .finish()
    }
}

impl ProcessRuntime {
    /// Create a new Process runtime / 创建新的进程运行时
    pub fn new(runtime_config: &RuntimeConfig) -> ExecutionResult<Self> {
        let process_config = if let Some(process_settings) = runtime_config.settings.get("process")
        {
            serde_json::from_value(process_settings.clone()).map_err(|e| {
                ExecutionError::InvalidConfiguration {
                    message: format!("Invalid Process configuration: {}", e),
                }
            })?
        } else {
            ProcessConfig::default()
        };

        Ok(Self {
            config: process_config,
            runtime_config: runtime_config.clone(),
            connection_manager: Arc::new(RwLock::new(None)),
            monitoring_service: Arc::new(RwLock::new(None)),
            listening_status: Arc::new(RwLock::new(ListeningStatus::Stopped)),
            message_handlers: Arc::new(RwLock::new(Vec::new())),
        })
    }

    /// Build process command / 构建进程命令
    fn build_process_command(&self, instance_config: &InstanceConfig) -> Command {
        let executable = instance_config
            .runtime_config
            .get("executable")
            .and_then(|v| v.as_str())
            .unwrap_or(&self.config.default_executable);

        let mut command = Command::new(executable);

        // Set working directory / 设置工作目录
        command.current_dir(&self.config.working_directory);

        // Set environment variables / 设置环境变量
        command.env_clear();

        // Add whitelisted environment variables / 添加白名单环境变量
        for env_var in &self.config.security_config.env_whitelist {
            if let Ok(value) = std::env::var(env_var) {
                command.env(env_var, value);
            }
        }

        // Add instance-specific environment variables / 添加实例特定的环境变量
        for (key, value) in &instance_config.environment {
            command.env(key, value);
        }

        // Add global environment variables / 添加全局环境变量
        for (key, value) in &self.runtime_config.global_environment {
            command.env(key, value);
        }

        // Configure stdio / 配置标准输入输出
        command.stdin(Stdio::piped());
        command.stdout(Stdio::piped());
        command.stderr(Stdio::piped());

        // Set process group (for better process management) / 设置进程组（更好的进程管理）
        if self.config.isolation_config.use_process_groups {
            #[cfg(unix)]
            {
                #[allow(unused_imports)]
                use std::os::unix::process::CommandExt;
                command.process_group(0);
            }
        }

        command
    }

    /// Build process command with listening configuration / 构建带有监听配置的进程命令
    #[allow(dead_code)]
    fn _build_process_command_with_listening(
        &self,
        instance_config: &InstanceConfig,
        service_addr: &str,
        secret: &str,
    ) -> Command {
        let executable = instance_config
            .runtime_config
            .get("executable")
            .and_then(|v| v.as_str())
            .unwrap_or(&self.config.default_executable);

        let mut command = Command::new(executable);

        // Set working directory / 设置工作目录
        command.current_dir(&self.config.working_directory);

        // Set environment variables / 设置环境变量
        command.env_clear();

        // Add whitelisted environment variables / 添加白名单环境变量
        for env_var in &self.config.security_config.env_whitelist {
            if let Ok(value) = std::env::var(env_var) {
                command.env(env_var, value);
            }
        }

        // Add instance-specific environment variables / 添加实例特定的环境变量
        for (key, value) in &instance_config.environment {
            command.env(key, value);
        }

        // Add global environment variables / 添加全局环境变量
        for (key, value) in &self.runtime_config.global_environment {
            command.env(key, value);
        }

        // Add listening mode environment variables / 添加监听模式环境变量
        command.env("SERVICE_ADDR", service_addr);
        command.env("SECRET", secret);

        // Configure stdio / 配置标准输入输出
        command.stdin(Stdio::piped());
        command.stdout(Stdio::piped());
        command.stderr(Stdio::piped());

        // Set process group (for better process management) / 设置进程组（更好的进程管理）
        if self.config.isolation_config.use_process_groups {
            #[cfg(unix)]
            {
                #[allow(unused_imports)]
                use std::os::unix::process::CommandExt;
                command.process_group(0);
            }
        }

        command
    }

    /// Monitor process resources / 监控进程资源
    async fn monitor_process_resources(
        &self,
        pid: u32,
    ) -> ExecutionResult<HashMap<String, serde_json::Value>> {
        let mut metrics = HashMap::new();

        #[cfg(unix)]
        {
            // Read process status from /proc/[pid]/stat / 从 /proc/[pid]/stat 读取进程状态
            if let Ok(stat_content) = tokio::fs::read_to_string(format!("/proc/{}/stat", pid)).await
            {
                let fields: Vec<&str> = stat_content.split_whitespace().collect();
                if fields.len() > 23 {
                    // CPU time (user + system) / CPU 时间（用户 + 系统）
                    let utime: u64 = fields[13].parse().unwrap_or(0);
                    let stime: u64 = fields[14].parse().unwrap_or(0);
                    metrics.insert(
                        "cpu_time_ms".to_string(),
                        serde_json::Value::Number(serde_json::Number::from((utime + stime) * 10)), // Convert to ms / 转换为毫秒
                    );

                    // Virtual memory size / 虚拟内存大小
                    let vsize: u64 = fields[22].parse().unwrap_or(0);
                    metrics.insert(
                        "virtual_memory_bytes".to_string(),
                        serde_json::Value::Number(serde_json::Number::from(vsize)),
                    );

                    // Resident set size / 常驻集大小
                    let rss: u64 = fields[23].parse().unwrap_or(0);
                    let page_size = 4096; // Assume 4KB pages / 假设 4KB 页面
                    metrics.insert(
                        "resident_memory_bytes".to_string(),
                        serde_json::Value::Number(serde_json::Number::from(rss * page_size)),
                    );
                }
            }

            // Read process status from /proc/[pid]/status / 从 /proc/[pid]/status 读取进程状态
            if let Ok(status_content) =
                tokio::fs::read_to_string(format!("/proc/{}/status", pid)).await
            {
                for line in status_content.lines() {
                    if line.starts_with("FDSize:") {
                        if let Some(fd_count) = line.split_whitespace().nth(1) {
                            if let Ok(count) = fd_count.parse::<u32>() {
                                metrics.insert(
                                    "file_descriptors".to_string(),
                                    serde_json::Value::Number(serde_json::Number::from(count)),
                                );
                            }
                        }
                    }
                }
            }
        }

        #[cfg(not(unix))]
        {
            // For non-Unix systems, provide basic metrics / 对于非 Unix 系统，提供基本指标
            metrics.insert(
                "platform".to_string(),
                serde_json::Value::String("non-unix".to_string()),
            );
        }

        Ok(metrics)
    }

    /// Kill process and its children / 终止进程及其子进程
    async fn kill_process_tree(&self, pid: u32) -> ExecutionResult<()> {
        #[cfg(unix)]
        {
            // Kill process group if using process groups / 如果使用进程组则终止进程组
            if self.config.isolation_config.use_process_groups {
                unsafe {
                    libc::killpg(pid as i32, libc::SIGTERM);
                    tokio::time::sleep(Duration::from_millis(100)).await;
                    libc::killpg(pid as i32, libc::SIGKILL);
                }
            } else {
                unsafe {
                    libc::kill(pid as i32, libc::SIGTERM);
                    tokio::time::sleep(Duration::from_millis(100)).await;
                    libc::kill(pid as i32, libc::SIGKILL);
                }
            }
        }

        #[cfg(not(unix))]
        {
            // For non-Unix systems, use standard process termination / 对于非 Unix 系统，使用标准进程终止
            if let Ok(mut child) = std::process::Command::new("taskkill")
                .args(&["/F", "/PID", &pid.to_string()])
                .spawn()
            {
                let _ = child.wait();
            }
        }

        Ok(())
    }

    /// Generate a secret for instance authentication / 为实例认证生成密钥
    fn generate_instance_secret(&self, instance_id: &str) -> String {
        use std::collections::hash_map::DefaultHasher;
        use std::hash::{Hash, Hasher};

        // Create a deterministic secret based on instance ID and current time / 基于实例ID和当前时间创建确定性密钥
        let mut hasher = DefaultHasher::new();
        instance_id.hash(&mut hasher);
        std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .unwrap_or_default()
            .as_secs()
            .hash(&mut hasher);

        // Generate a hex string from the hash / 从哈希生成十六进制字符串
        format!("{:x}", hasher.finish())
    }
}

#[async_trait]
impl Runtime for ProcessRuntime {
    fn runtime_type(&self) -> RuntimeType {
        RuntimeType::Process
    }

    async fn create_instance(&self, config: &InstanceConfig) -> ExecutionResult<Arc<TaskInstance>> {
        debug!("ProcessRuntime::create_instance task_id={}", config.task_id);
        let instance = Arc::new(TaskInstance::new(config.task_id.clone(), config.clone()));

        // Generate and store secret for this instance / 为此实例生成并存储密钥
        let secret = self.generate_instance_secret(instance.id());
        instance.set_secret(secret);

        let mut command = self.build_process_command(config);

        // Add arguments if specified / 如果指定了参数则添加
        if let Some(args) = config.runtime_config.get("args") {
            if let Some(args_array) = args.as_array() {
                for arg in args_array {
                    if let Some(arg_str) = arg.as_str() {
                        command.arg(arg_str);
                    }
                }
            }
        }

        let child = command.spawn().map_err(|e| ExecutionError::RuntimeError {
            message: format!("Failed to spawn process: {}", e),
        })?;

        let pid = child.id().unwrap_or(0);

        let process_handle = ProcessHandle {
            pid,
            command: self.config.default_executable.clone(),
            args: vec![],
            working_directory: self.config.working_directory.clone(),
            environment: config.environment.clone(),
            start_time: std::time::SystemTime::now(),
            child: Arc::new(Mutex::new(Some(child))),
        };

        instance.set_runtime_handle(Arc::new(process_handle));
        instance.set_status(InstanceStatus::Ready);

        Ok(instance)
    }

    async fn start_instance(&self, instance: &Arc<TaskInstance>) -> ExecutionResult<()> {
        debug!(
            "ProcessRuntime::start_instance instance_id={}",
            instance.id()
        );
        // Process is already started when created / 进程在创建时已经启动
        instance.set_status(InstanceStatus::Running);

        // Get the secret that was generated during instance creation / 获取在实例创建时生成的secret
        let secret = instance
            .get_secret()
            .ok_or_else(|| ExecutionError::RuntimeError {
                message: "Instance secret not found".to_string(),
            })?;

        // Start listening mode for this instance / 为此实例启动监听模式
        let listening_config = RuntimeListeningConfig {
            enabled: true,
            connection_config: ConnectionManagerConfig::default(),
            auth_config: super::AuthConfig {
                required: true,
                timeout_secs: 30,
                valid_tokens: vec![secret.clone()],
                validation_strategy: super::TokenValidationStrategy::Static,
            },
            message_config: super::MessageHandlingConfig::default(),
        };

        // Start listening and get the address / 启动监听并获取地址
        if let Some(listening_addr) = self.start_listening(&listening_config).await? {
            // Store the listening address in the instance / 将监听地址存储在实例中
            instance.set_listening_address(listening_addr.to_string());

            info!(
                "Instance {} listening on {} with secret authentication enabled",
                instance.id, listening_addr
            );
        }

        Ok(())
    }

    async fn stop_instance(&self, instance: &Arc<TaskInstance>) -> ExecutionResult<()> {
        debug!(
            "ProcessRuntime::stop_instance instance_id={}",
            instance.id()
        );
        let handle = instance
            .get_runtime_handle::<ProcessHandle>()
            .ok_or_else(|| ExecutionError::RuntimeError {
                message: "No process handle found".to_string(),
            })?;

        instance.set_status(InstanceStatus::Stopping);

        // Kill the process / 终止进程
        self.kill_process_tree(handle.pid).await?;

        // Wait for process to exit / 等待进程退出
        if let Some(mut child) = handle.child.lock().await.take() {
            let _ = timeout(Duration::from_secs(5), child.wait()).await;
        }

        instance.set_status(InstanceStatus::Stopped);
        Ok(())
    }

    async fn execute(
        &self,
        instance: &Arc<TaskInstance>,
        _context: ExecutionContext,
    ) -> ExecutionResult<RuntimeExecutionResponse> {
        debug!(
            "ProcessRuntime::execute instance_id={} execution_id={}",
            instance.id(),
            _context.execution_id
        );
        let start_time = Instant::now();
        instance.record_request_start();

        let handle = instance
            .get_runtime_handle::<ProcessHandle>()
            .ok_or_else(|| ExecutionError::RuntimeError {
                message: "No process handle found".to_string(),
            })?;

        // For process runtime, we'll execute by sending data to stdin and reading from stdout
        // 对于进程运行时，我们通过向 stdin 发送数据并从 stdout 读取来执行
        let child_guard = handle.child.lock().await;
        if let Some(_child) = child_guard.as_ref() {
            // This is a simplified implementation / 这是一个简化的实现
            // In a real implementation, you would have a proper protocol for communication
            // 在真实实现中，您需要有一个适当的通信协议

            let duration = start_time.elapsed();
            let duration_ms = duration.as_millis() as u64;

            instance.record_request_completion(true, duration_ms as f64);

            Ok(RuntimeExecutionResponse::new_sync(
                _context.execution_id,
                b"Process execution completed".to_vec(),
                duration_ms,
            ))
        } else {
            let duration = start_time.elapsed();
            let duration_ms = duration.as_millis() as u64;
            instance.record_request_completion(false, duration_ms as f64);

            Err(ExecutionError::RuntimeError {
                message: "Process not available".to_string(),
            })
        }
    }

    async fn health_check(&self, instance: &Arc<TaskInstance>) -> ExecutionResult<bool> {
        debug!("ProcessRuntime::health_check instance_id={}", instance.id());
        let handle = instance
            .get_runtime_handle::<ProcessHandle>()
            .ok_or_else(|| ExecutionError::RuntimeError {
                message: "No process handle found".to_string(),
            })?;

        // Check if process is still running / 检查进程是否仍在运行
        #[cfg(unix)]
        {
            let is_alive = unsafe { libc::kill(handle.pid as i32, 0) == 0 };
            instance.record_health_check(is_alive);
            Ok(is_alive)
        }

        #[cfg(not(unix))]
        {
            // For non-Unix systems, check if child process is still running / 对于非 Unix 系统，检查子进程是否仍在运行
            let child_guard = handle.child.lock().await;
            if let Some(child) = child_guard.as_ref() {
                match child.try_wait() {
                    Ok(None) => {
                        instance.record_health_check(true);
                        Ok(true)
                    }
                    _ => {
                        instance.record_health_check(false);
                        Ok(false)
                    }
                }
            } else {
                instance.record_health_check(false);
                Ok(false)
            }
        }
    }

    async fn get_metrics(
        &self,
        instance: &Arc<TaskInstance>,
    ) -> ExecutionResult<HashMap<String, serde_json::Value>> {
        debug!("ProcessRuntime::get_metrics instance_id={}", instance.id());
        let handle = instance
            .get_runtime_handle::<ProcessHandle>()
            .ok_or_else(|| ExecutionError::RuntimeError {
                message: "No process handle found".to_string(),
            })?;

        self.monitor_process_resources(handle.pid).await
    }

    async fn scale_instance(
        &self,
        _instance: &Arc<TaskInstance>,
        _new_limits: &InstanceResourceLimits,
    ) -> ExecutionResult<()> {
        debug!(
            "ProcessRuntime::scale_instance instance_id={}",
            _instance.id()
        );
        // Process runtime supports limited scaling through ulimit / 进程运行时通过 ulimit 支持有限的扩缩容
        // This is a simplified implementation / 这是一个简化的实现
        Ok(())
    }

    async fn cleanup_instance(&self, instance: &Arc<TaskInstance>) -> ExecutionResult<()> {
        debug!(
            "ProcessRuntime::cleanup_instance instance_id={}",
            instance.id()
        );
        if let Some(handle) = instance.get_runtime_handle::<ProcessHandle>() {
            self.kill_process_tree(handle.pid).await?;
        }
        Ok(())
    }

    fn validate_config(&self, config: &InstanceConfig) -> ExecutionResult<()> {
        debug!("ProcessRuntime::validate_config task_id={}", config.task_id);
        if config.runtime_type != RuntimeType::Process {
            return Err(ExecutionError::InvalidConfiguration {
                message: "Runtime type must be Process".to_string(),
            });
        }

        // Validate executable path / 验证可执行文件路径
        if let Some(executable) = config.runtime_config.get("executable") {
            if let Some(exec_str) = executable.as_str() {
                if !std::path::Path::new(exec_str).exists() {
                    return Err(ExecutionError::InvalidConfiguration {
                        message: format!("Executable not found: {}", exec_str),
                    });
                }
            }
        }

        // Validate resource limits / 验证资源限制
        let limits = &config.resource_limits;
        if limits.max_cpu_cores <= 0.0 {
            return Err(ExecutionError::InvalidConfiguration {
                message: "CPU cores must be greater than 0".to_string(),
            });
        }

        if limits.max_memory_bytes == 0 {
            return Err(ExecutionError::InvalidConfiguration {
                message: "Memory limit must be greater than 0".to_string(),
            });
        }

        Ok(())
    }

    fn get_capabilities(&self) -> RuntimeCapabilities {
        RuntimeCapabilities {
            supports_scaling: true, // Limited scaling through ulimit / 通过 ulimit 进行有限扩缩容
            supports_health_checks: true,
            supports_metrics: true,
            supports_hot_reload: false,
            supports_persistent_storage: true,
            supports_network_isolation: false, // Limited network isolation / 有限的网络隔离
            max_concurrent_instances: self.runtime_config.resource_pool.max_concurrent_instances,
            supported_protocols: vec!["HTTP".to_string(), "gRPC".to_string(), "Custom".to_string()],
        }
    }

    // 监听模式相关方法实现 / Listening mode related method implementations

    fn supports_listening_mode(&self) -> bool {
        true
    }

    async fn start_listening(
        &self,
        config: &RuntimeListeningConfig,
    ) -> ExecutionResult<Option<SocketAddr>> {
        if !config.enabled {
            return Ok(None);
        }

        // Update listening status to starting / 更新监听状态为启动中
        *self.listening_status.write().await = ListeningStatus::Starting;

        // Create monitoring service / 创建监控服务
        let monitoring_config = MonitoringConfig {
            enabled: true,
            collection_interval_secs: 30,
            max_history_size: 1000,
            enable_message_tracking: true,
            enable_connection_tracking: true,
            enable_performance_profiling: false,
        };
        let monitoring_service = Arc::new(MonitoringService::new(monitoring_config));

        // Start monitoring service / 启动监控服务
        monitoring_service
            .start()
            .await
            .map_err(|e| ExecutionError::RuntimeError {
                message: format!("Failed to start monitoring service: {}", e),
            })?;

        // Create connection manager with secret validator
        // 创建带有 secret 验证器的连接管理器
        // Note: For ProcessRuntime, we use a simple validator since we don't have access to TaskExecutionManager
        // 注意：对于 ProcessRuntime，我们使用简单验证器，因为我们无法访问 TaskExecutionManager
        let secret_validator = Arc::new(|instance_id: &str, secret: &str| -> bool {
            // Basic validation: secret should not be empty and should be at least 8 characters
            // 基本验证：secret 不应为空且至少 8 个字符
            !secret.is_empty() && secret.len() >= 8 && !instance_id.is_empty()
        });

        let connection_manager = Arc::new(ConnectionManager::new_with_validator(
            config.connection_config.clone(),
            Some(secret_validator),
        ));

        // Start connection manager / 启动连接管理器
        let listening_addr =
            connection_manager
                .start()
                .await
                .map_err(|e| ExecutionError::RuntimeError {
                    message: format!("Failed to start connection manager: {}", e),
                })?;

        // Store services / 存储服务
        *self.connection_manager.write().await = Some(Arc::clone(&connection_manager));
        *self.monitoring_service.write().await = Some(Arc::clone(&monitoring_service));

        // Update listening status to active / 更新监听状态为活跃
        *self.listening_status.write().await = ListeningStatus::Active {
            address: listening_addr,
            active_connections: 0,
            started_at: std::time::SystemTime::now(),
        };

        // Setup message handling / 设置消息处理
        // TODO: Implement message processing task
        // TODO: 实现消息处理任务

        Ok(Some(listening_addr))
    }

    async fn stop_listening(&self) -> ExecutionResult<()> {
        // Stop connection manager / 停止连接管理器
        if let Some(connection_manager) = self.connection_manager.read().await.as_ref() {
            // Close all connections / 关闭所有连接
            connection_manager.close_all_connections();
        }

        // Clear services / 清理服务
        *self.connection_manager.write().await = None;
        *self.monitoring_service.write().await = None;

        // Update listening status / 更新监听状态
        *self.listening_status.write().await = ListeningStatus::Stopped;

        Ok(())
    }

    async fn get_listening_status(&self) -> ExecutionResult<ListeningStatus> {
        Ok(self.listening_status.read().await.clone())
    }

    async fn handle_agent_message(
        &self,
        instance_id: &str,
        message: SpearMessage,
    ) -> ExecutionResult<Option<SpearMessage>> {
        // Record message event / 记录消息事件
        if let Some(monitoring) = self.monitoring_service.read().await.as_ref() {
            monitoring
                .record_message_event(
                    instance_id.to_string(),
                    &message,
                    MessageDirection::Incoming,
                    None,
                )
                .await;
        }

        // Simple echo response for now / 目前简单回显响应
        let response_payload = serde_json::json!({
            "status": "received",
            "original_type": format!("{:?}", message.message_type)
        });
        let payload_bytes =
            serde_json::to_vec(&response_payload).map_err(ExecutionError::Serialization)?;

        let response_message = SpearMessage {
            message_type: MessageType::ExecuteResponse,
            request_id: message.request_id,
            timestamp: std::time::SystemTime::now(),
            payload: payload_bytes,
            version: message.version,
        };

        Ok(Some(response_message))
    }

    async fn get_connection_manager(&self) -> ExecutionResult<Option<Arc<ConnectionManager>>> {
        Ok(self.connection_manager.read().await.clone())
    }

    async fn register_message_handler(
        &self,
        handler: Box<dyn MessageHandler>,
    ) -> ExecutionResult<()> {
        self.message_handlers.write().await.push(handler);
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::spearlet::execution::instance::{
        InstanceConfig, InstanceResourceLimits, NetworkConfig,
    };

    #[test]
    fn test_process_config_default() {
        let config = ProcessConfig::default();
        assert_eq!(config.working_directory, "/tmp/spearlet");
        assert_eq!(config.default_executable, "/bin/sh");
        assert!(config.isolation_config.use_process_groups);
        assert!(config.security_config.drop_privileges);
    }

    #[test]
    fn test_process_runtime_creation() {
        let runtime_config = RuntimeConfig {
            runtime_type: RuntimeType::Process,
            settings: HashMap::new(),
            global_environment: HashMap::new(),
            spearlet_config: None,
            resource_pool: super::super::ResourcePoolConfig::default(),
        };

        let runtime = ProcessRuntime::new(&runtime_config);
        assert!(runtime.is_ok());

        let runtime = runtime.unwrap();
        assert_eq!(runtime.runtime_type(), RuntimeType::Process);
    }

    #[test]
    fn test_validate_config() {
        let runtime_config = RuntimeConfig {
            runtime_type: RuntimeType::Process,
            settings: HashMap::new(),
            global_environment: HashMap::new(),
            spearlet_config: None,
            resource_pool: super::super::ResourcePoolConfig::default(),
        };

        let runtime = ProcessRuntime::new(&runtime_config).unwrap();

        let valid_config = InstanceConfig {
            task_id: "task-xyz".to_string(),
            artifact_id: "artifact-xyz".to_string(),
            runtime_type: RuntimeType::Process,
            runtime_config: HashMap::new(),
            task_config: HashMap::new(),
            artifact: None,
            environment: HashMap::new(),
            resource_limits: InstanceResourceLimits::default(),
            network_config: NetworkConfig::default(),
            max_concurrent_requests: 100,
            request_timeout_ms: 30000,
        };

        assert!(runtime.validate_config(&valid_config).is_ok());

        let invalid_config = InstanceConfig {
            task_id: "task-xyz".to_string(),
            artifact_id: "artifact-xyz".to_string(),
            runtime_type: RuntimeType::Kubernetes, // Different runtime type for testing / 用于测试的不同运行时类型
            runtime_config: HashMap::new(),
            task_config: HashMap::new(),
            artifact: None,
            environment: HashMap::new(),
            resource_limits: InstanceResourceLimits::default(),
            network_config: NetworkConfig::default(),
            max_concurrent_requests: 100,
            request_timeout_ms: 30000,
        };

        assert!(runtime.validate_config(&invalid_config).is_err());
    }

    #[tokio::test]
    async fn test_monitor_process_resources() {
        let runtime_config = RuntimeConfig {
            runtime_type: RuntimeType::Process,
            settings: HashMap::new(),
            global_environment: HashMap::new(),
            spearlet_config: None,
            resource_pool: super::super::ResourcePoolConfig::default(),
        };

        let runtime = ProcessRuntime::new(&runtime_config).unwrap();

        // Use current process PID for testing / 使用当前进程 PID 进行测试
        let pid = std::process::id();
        let metrics = runtime.monitor_process_resources(pid).await;

        // On Unix systems, we should get some metrics / 在 Unix 系统上，我们应该得到一些指标
        #[cfg(unix)]
        assert!(metrics.is_ok());

        #[cfg(not(unix))]
        {
            if let Ok(metrics_map) = metrics {
                assert!(metrics_map.contains_key("platform"));
            }
        }
    }
}
