//! Task Execution Manager
//! 任务执行管理器
//!
//! This module provides the central task execution management system that coordinates
//! artifacts, tasks, instances, and runtime execution.
//! 该模块提供中央任务执行管理系统，协调 artifact、任务、实例和运行时执行。

use super::{
    artifact::{Artifact, ArtifactId},
    instance::{InstanceId, InstanceStatus, TaskInstance},
    runtime::{RuntimeManager, ExecutionContext},
    scheduler::{InstanceScheduler, SchedulingPolicy},
    task::{Task, TaskId, TaskType},
    ExecutionError, ExecutionResult,
};
use crate::proto::spearlet::{
    ArtifactSpec as ProtoArtifactSpec, InvokeFunctionRequest,
};
use dashmap::DashMap;
use parking_lot::RwLock;
use serde::{Deserialize, Serialize};
use std::sync::atomic::{AtomicU64, Ordering};
use std::sync::Arc;
use std::time::{Duration, Instant, SystemTime};
use tokio::sync::{mpsc, oneshot, Semaphore};
use tokio::time::timeout;
use tracing::{debug, info, warn};

/// Task execution manager configuration / 任务执行管理器配置
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TaskExecutionManagerConfig {
    /// Maximum concurrent executions / 最大并发执行数
    pub max_concurrent_executions: usize,
    /// Maximum artifacts / 最大 artifact 数
    pub max_artifacts: usize,
    /// Maximum tasks per artifact / 每个 artifact 的最大任务数
    pub max_tasks_per_artifact: usize,
    /// Maximum instances per task / 每个任务的最大实例数
    pub max_instances_per_task: usize,
    /// Instance creation timeout / 实例创建超时
    pub instance_creation_timeout_ms: u64,
    /// Health check interval / 健康检查间隔
    pub health_check_interval_ms: u64,
    /// Metrics collection interval / 指标收集间隔
    pub metrics_collection_interval_ms: u64,
    /// Cleanup interval / 清理间隔
    pub cleanup_interval_ms: u64,
    /// Artifact idle timeout / Artifact 空闲超时
    pub artifact_idle_timeout_ms: u64,
    /// Task idle timeout / 任务空闲超时
    pub task_idle_timeout_ms: u64,
    /// Instance idle timeout / 实例空闲超时
    pub instance_idle_timeout_ms: u64,
}

impl Default for TaskExecutionManagerConfig {
    fn default() -> Self {
        Self {
            max_concurrent_executions: 1000,
            max_artifacts: 100,
            max_tasks_per_artifact: 10,
            max_instances_per_task: 50,
            instance_creation_timeout_ms: 30000,
            health_check_interval_ms: 10000,
            metrics_collection_interval_ms: 5000,
            cleanup_interval_ms: 60000,
            artifact_idle_timeout_ms: 300000,  // 5 minutes
            task_idle_timeout_ms: 180000,      // 3 minutes
            instance_idle_timeout_ms: 120000,  // 2 minutes
        }
    }
}

/// Execution request / 执行请求
/// Execution request queue entry / 执行请求队列条目
#[derive(Debug, Clone)]
pub struct ExecutionRequestQueueEntry {
    /// Request ID / 请求 ID
    pub request_id: String,
    /// Artifact specification / Artifact 规范
    pub artifact_spec: ProtoArtifactSpec,
    /// Execution context / 执行上下文
    pub execution_context: ExecutionContext,
    /// Request timestamp / 请求时间戳
    pub timestamp: SystemTime,
}

/// Execution request / 执行请求
#[derive(Debug)]
pub struct ExecutionRequest {
    /// Request ID / 请求 ID
    pub request_id: String,
    /// Artifact specification / Artifact 规范
    pub artifact_spec: ProtoArtifactSpec,
    /// Execution context / 执行上下文
    pub execution_context: ExecutionContext,
    /// Response sender / 响应发送器
    pub response_sender: oneshot::Sender<ExecutionResult<super::ExecutionResponse>>,
    /// Request timestamp / 请求时间戳
    pub timestamp: SystemTime,
}

/// Execution statistics / 执行统计
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ExecutionStatistics {
    /// Total executions / 总执行次数
    pub total_executions: u64,
    /// Successful executions / 成功执行次数
    pub successful_executions: u64,
    /// Failed executions / 失败执行次数
    pub failed_executions: u64,
    /// Average execution time / 平均执行时间
    pub average_execution_time_ms: f64,
    /// Total execution time / 总执行时间
    pub total_execution_time_ms: u64,
    /// Active artifacts / 活跃 artifact 数
    pub active_artifacts: u64,
    /// Active tasks / 活跃任务数
    pub active_tasks: u64,
    /// Active instances / 活跃实例数
    pub active_instances: u64,
    /// Queue size / 队列大小
    pub queue_size: u64,
    /// Running executions / 正在运行的执行数量
    pub running_executions: u64,
    /// Pending executions / 等待执行的数量
    pub pending_executions: u64,
    /// Completed executions / 已完成的执行数量
    pub completed_executions: u64,
    /// Success rate percentage / 成功率百分比
    pub success_rate_percent: f64,
}

impl Default for ExecutionStatistics {
    fn default() -> Self {
        Self {
            total_executions: 0,
            successful_executions: 0,
            failed_executions: 0,
            average_execution_time_ms: 0.0,
            total_execution_time_ms: 0,
            active_artifacts: 0,
            active_tasks: 0,
            active_instances: 0,
            queue_size: 0,
            running_executions: 0,
            pending_executions: 0,
            completed_executions: 0,
            success_rate_percent: 0.0,
        }
    }
}

/// Task execution manager / 任务执行管理器
pub struct TaskExecutionManager {
    /// Configuration / 配置
    config: TaskExecutionManagerConfig,
    /// Runtime manager / 运行时管理器
    runtime_manager: Arc<RuntimeManager>,
    /// Instance scheduler / 实例调度器
    scheduler: Arc<InstanceScheduler>,
    /// Artifacts storage / Artifact 存储
    artifacts: Arc<DashMap<ArtifactId, Arc<Artifact>>>,
    /// Tasks storage / 任务存储
    tasks: Arc<DashMap<TaskId, Arc<Task>>>,
    /// Instances storage / 实例存储
    instances: Arc<DashMap<InstanceId, Arc<TaskInstance>>>,
    /// Execution request queue / 执行请求队列
    request_queue: Arc<DashMap<String, ExecutionRequestQueueEntry>>,
    /// Execution semaphore / 执行信号量
    execution_semaphore: Arc<Semaphore>,
    /// Statistics / 统计信息
    statistics: Arc<RwLock<ExecutionStatistics>>,
    /// Request counter / 请求计数器
    request_counter: AtomicU64,
    /// Execution request sender / 执行请求发送器
    request_sender: mpsc::UnboundedSender<ExecutionRequest>,
    /// Shutdown signal / 关闭信号
    shutdown_sender: Option<oneshot::Sender<()>>,
}

impl TaskExecutionManager {
    /// Create a new task execution manager / 创建新的任务执行管理器
    pub async fn new(
        config: TaskExecutionManagerConfig,
        runtime_manager: Arc<RuntimeManager>,
    ) -> ExecutionResult<Arc<Self>> {
        let scheduler = Arc::new(InstanceScheduler::new(SchedulingPolicy::RoundRobin));
        let execution_semaphore = Arc::new(Semaphore::new(config.max_concurrent_executions));

        let (request_sender, request_receiver) = mpsc::unbounded_channel();
        let (shutdown_sender, shutdown_receiver) = oneshot::channel();

        let manager = Arc::new(Self {
            config: config.clone(),
            runtime_manager,
            scheduler,
            artifacts: Arc::new(DashMap::new()),
            tasks: Arc::new(DashMap::new()),
            instances: Arc::new(DashMap::new()),
            request_queue: Arc::new(DashMap::new()),
            execution_semaphore,
            statistics: Arc::new(RwLock::new(ExecutionStatistics::default())),
            request_counter: AtomicU64::new(0),
            request_sender,
            shutdown_sender: Some(shutdown_sender),
        });

        // Start background tasks / 启动后台任务
        let manager_clone = manager.clone();
        tokio::spawn(async move {
            manager_clone.run_execution_loop(request_receiver, shutdown_receiver).await;
        });

        let manager_clone = manager.clone();
        tokio::spawn(async move {
            manager_clone.run_health_check_loop().await;
        });

        let manager_clone = manager.clone();
        tokio::spawn(async move {
            manager_clone.run_metrics_collection_loop().await;
        });

        let manager_clone = manager.clone();
        tokio::spawn(async move {
            manager_clone.run_cleanup_loop().await;
        });

        info!("TaskExecutionManager started with config: {:?}", config);
        Ok(manager)
    }

    /// Submit execution request / 提交执行请求
    pub async fn submit_execution(
        &self,
        request: InvokeFunctionRequest,
    ) -> ExecutionResult<super::ExecutionResponse> {
        let request_id = format!(
            "req-{}",
            self.request_counter.fetch_add(1, Ordering::SeqCst)
        );

        let artifact_spec = request.artifact_spec.ok_or_else(|| ExecutionError::InvalidRequest {
            message: "Missing artifact specification".to_string(),
        })?;

        let execution_context = ExecutionContext {
            execution_id: request_id.clone(),
            payload: Vec::new(), // TODO: Extract payload from request
            headers: std::collections::HashMap::new(), // TODO: Extract headers from request
            timeout_ms: 30000, // TODO: Extract timeout from request context
            context_data: std::collections::HashMap::new(), // TODO: Extract context data from request
        };

        let (response_sender, response_receiver) = oneshot::channel();

        let timestamp = SystemTime::now();

        // Create queue entry / 创建队列条目
        let queue_entry = ExecutionRequestQueueEntry {
            request_id: request_id.clone(),
            artifact_spec: artifact_spec.clone(),
            execution_context: execution_context.clone(),
            timestamp,
        };

        // Add to queue / 添加到队列
        self.request_queue.insert(request_id.clone(), queue_entry);

        let execution_request = ExecutionRequest {
            request_id: request_id.clone(),
            artifact_spec,
            execution_context,
            response_sender,
            timestamp,
        };

        // Send to execution loop / 发送到执行循环
        self.request_sender.send(execution_request).map_err(|_| {
            ExecutionError::RuntimeError {
                message: "Failed to submit execution request".to_string(),
            }
        })?;

        // Update statistics / 更新统计信息
        {
            let mut stats = self.statistics.write();
            stats.queue_size += 1;
        }

        // Wait for response / 等待响应
        response_receiver.await.map_err(|_| ExecutionError::RuntimeError {
            message: "Execution request was cancelled".to_string(),
        })?
    }

    /// Get artifact by ID / 根据 ID 获取 artifact
    pub fn get_artifact(&self, artifact_id: &ArtifactId) -> Option<Arc<Artifact>> {
        self.artifacts.get(artifact_id).map(|entry| entry.clone())
    }

    /// Get task by ID / 根据 ID 获取任务
    pub fn get_task(&self, task_id: &TaskId) -> Option<Arc<Task>> {
        self.tasks.get(task_id).map(|entry| entry.clone())
    }

    /// Get instance by ID / 根据 ID 获取实例
    pub fn get_instance(&self, instance_id: &InstanceId) -> Option<Arc<TaskInstance>> {
        self.instances.get(instance_id).map(|entry| entry.clone())
    }

    /// List all artifacts / 列出所有 artifact
    pub fn list_artifacts(&self) -> Vec<Arc<Artifact>> {
        self.artifacts.iter().map(|entry| entry.clone()).collect()
    }

    /// List all tasks / 列出所有任务
    pub fn list_tasks(&self) -> Vec<Arc<Task>> {
        self.tasks.iter().map(|entry| entry.clone()).collect()
    }

    /// List all instances / 列出所有实例
    pub fn list_instances(&self) -> Vec<Arc<TaskInstance>> {
        self.instances.iter().map(|entry| entry.clone()).collect()
    }

    /// Get execution statistics / 获取执行统计信息
    pub fn get_statistics(&self) -> ExecutionStatistics {
        self.statistics.read().clone()
    }

    /// Get execution status by execution ID / 根据执行ID获取执行状态
    pub async fn get_execution_status(&self, execution_id: &str) -> ExecutionResult<Option<super::ExecutionResponse>> {
        // TODO: Implement proper execution status tracking
        // For now, return None to indicate execution not found
        // 待办：实现适当的执行状态跟踪
        // 目前返回 None 表示未找到执行
        Ok(None)
    }

    /// Shutdown the manager / 关闭管理器
    pub async fn shutdown(&mut self) -> ExecutionResult<()> {
        if let Some(sender) = self.shutdown_sender.take() {
            let _ = sender.send(());
        }

        // Stop all instances / 停止所有实例
        for instance_entry in self.instances.iter() {
            let instance = instance_entry.value();
            if let Err(e) = self.stop_instance(instance).await {
                warn!("Failed to stop instance {}: {}", instance.id(), e);
            }
        }

        info!("TaskExecutionManager shutdown completed");
        Ok(())
    }

    /// Main execution loop / 主执行循环
    async fn run_execution_loop(
        &self,
        mut request_receiver: mpsc::UnboundedReceiver<ExecutionRequest>,
        mut shutdown_receiver: oneshot::Receiver<()>,
    ) {
        info!("Starting execution loop");

        loop {
            tokio::select! {
                Some(request) = request_receiver.recv() => {
                    let manager = self.clone();
                    tokio::spawn(async move {
                        manager.handle_execution_request(request).await;
                    });
                }
                _ = &mut shutdown_receiver => {
                    info!("Execution loop shutting down");
                    break;
                }
            }
        }
    }

    /// Handle execution request / 处理执行请求
    async fn handle_execution_request(&self, request: ExecutionRequest) {
        let start_time = Instant::now();
        let request_id = request.request_id.clone();

        // Remove from queue / 从队列中移除
        self.request_queue.remove(&request_id);

        // Update statistics / 更新统计信息
        {
            let mut stats = self.statistics.write();
            stats.queue_size = stats.queue_size.saturating_sub(1);
            stats.total_executions += 1;
        }

        // Acquire execution permit / 获取执行许可
        let _permit = match self.execution_semaphore.acquire().await {
            Ok(permit) => permit,
            Err(_) => {
                let _ = request.response_sender.send(Err(ExecutionError::RuntimeError {
                    message: "Failed to acquire execution permit".to_string(),
                }));
                return;
            }
        };

        let result = self.execute_request(request.artifact_spec, request.execution_context).await;

        let execution_time = start_time.elapsed();
        let execution_time_ms = execution_time.as_millis() as u64;

        // Update statistics / 更新统计信息
        {
            let mut stats = self.statistics.write();
            stats.total_execution_time_ms += execution_time_ms;
            stats.average_execution_time_ms = 
                stats.total_execution_time_ms as f64 / stats.total_executions as f64;

            match &result {
                Ok(_) => stats.successful_executions += 1,
                Err(_) => stats.failed_executions += 1,
            }
        }

        // Send response / 发送响应
        let _ = request.response_sender.send(result);
    }

    /// Execute request / 执行请求
    async fn execute_request(
        &self,
        artifact_spec: ProtoArtifactSpec,
        execution_context: ExecutionContext,
    ) -> ExecutionResult<super::ExecutionResponse> {
        // Get or create artifact / 获取或创建 artifact
        let artifact = self.get_or_create_artifact(artifact_spec).await?;

        // Get or create task / 获取或创建任务
        let task = self.get_or_create_task(&artifact).await?;

        // Get or create instance / 获取或创建实例
        let instance = self.get_or_create_instance(&task).await?;

        // Execute on instance / 在实例上执行
        let runtime = self.runtime_manager.get_runtime(&task.spec.runtime_type)
            .ok_or_else(|| ExecutionError::RuntimeError {
                message: format!("Runtime not found for type: {:?}", task.spec.runtime_type),
            })?;
        let execution_id = execution_context.execution_id.clone();
        let runtime_response = runtime.execute(&instance, execution_context).await?;
        
        // Convert RuntimeExecutionResponse to ExecutionResponse / 转换运行时响应到执行响应
        let is_successful = runtime_response.is_successful();
        let has_failed = runtime_response.has_failed();
        let error_message = runtime_response.error.as_ref().map(|e| Self::extract_error_message(e));
        let duration_ms = runtime_response.duration_ms;
        let metadata = runtime_response.metadata.into_iter()
            .map(|(k, v)| (k, v.to_string()))
            .collect();
        let data = runtime_response.data;
        
        Ok(super::ExecutionResponse {
            request_id: execution_id,
            output_data: data,
            status: if is_successful { 
                "completed".to_string() 
            } else if has_failed { 
                "failed".to_string() 
            } else { 
                "pending".to_string() 
            },
            error_message,
            execution_time_ms: duration_ms,
            metadata,
            timestamp: SystemTime::now(),
        })
    }

    /// Get or create artifact / 获取或创建 artifact
    async fn get_or_create_artifact(
        &self,
        artifact_spec: ProtoArtifactSpec,
    ) -> ExecutionResult<Arc<Artifact>> {
        let artifact_id = artifact_spec.artifact_id.clone();

        if let Some(artifact) = self.artifacts.get(&artifact_id) {
            return Ok(artifact.clone());
        }

        // Check artifact limit / 检查 artifact 限制
        if self.artifacts.len() >= self.config.max_artifacts {
            return Err(ExecutionError::ResourceExhausted {
                message: format!("Maximum artifacts limit reached: {}", self.config.max_artifacts),
            });
        }

        let artifact_spec_local = super::artifact::ArtifactSpec::from(artifact_spec);
        let artifact = Arc::new(Artifact::new(artifact_spec_local));
        self.artifacts.insert(artifact_id.clone(), artifact.clone());

        // Update statistics / 更新统计信息
        {
            let mut stats = self.statistics.write();
            stats.active_artifacts = self.artifacts.len() as u64;
        }

        info!("Created new artifact: {}", artifact_id);
        Ok(artifact)
    }

    /// Get or create task / 获取或创建任务
    async fn get_or_create_task(&self, artifact: &Arc<Artifact>) -> ExecutionResult<Arc<Task>> {
        let task_id = format!("task-{}-{:?}", artifact.id(), TaskType::HttpHandler);

        if let Some(task) = self.tasks.get(&task_id) {
            return Ok(task.clone());
        }

        // Check task limit / 检查任务限制
        if artifact.task_count() >= self.config.max_tasks_per_artifact {
            return Err(ExecutionError::ResourceExhausted {
                message: format!(
                    "Maximum tasks per artifact limit reached: {}",
                    self.config.max_tasks_per_artifact
                ),
            });
        }

        // Create TaskSpec from ArtifactSpec / 从 ArtifactSpec 创建 TaskSpec
        use super::task::{TaskSpec, ScalingConfig, HealthCheckConfig, TimeoutConfig};
        use std::collections::HashMap;
        
        let task_spec = TaskSpec {
            name: artifact.spec.name.clone(),
            task_type: TaskType::HttpHandler,
            runtime_type: artifact.spec.runtime_type.clone(),
            entry_point: "main".to_string(), // Default entry point / 默认入口点
            handler_config: HashMap::new(),
            environment: artifact.spec.environment.clone(),
            invocation_type: artifact.spec.invocation_type.clone(),
            min_instances: 1,
            max_instances: 10,
            target_concurrency: 100,
            scaling_config: ScalingConfig::default(),
            health_check: HealthCheckConfig::default(),
            timeout_config: TimeoutConfig::default(),
        };

        let task = Arc::new(Task::new(artifact.id().to_string(), task_spec));
        self.tasks.insert(task_id.clone(), task.clone());
        artifact.add_task(task.clone())?;

        // Update statistics / 更新统计信息
        {
            let mut stats = self.statistics.write();
            stats.active_tasks = self.tasks.len() as u64;
        }

        info!("Created new task: {}", task_id);
        Ok(task)
    }

    /// Get or create instance / 获取或创建实例
    async fn get_or_create_instance(&self, task: &Arc<Task>) -> ExecutionResult<Arc<TaskInstance>> {
        // Try to find an available instance / 尝试找到可用实例
        if let Some(instance) = self.scheduler.select_instance(task).await? {
            return Ok(instance);
        }

        // Check instance limit / 检查实例限制
        if task.instance_count() >= self.config.max_instances_per_task {
            return Err(ExecutionError::ResourceExhausted {
                message: format!(
                    "Maximum instances per task limit reached: {}",
                    self.config.max_instances_per_task
                ),
            });
        }

        // Create new instance / 创建新实例
        let instance_id = task.generate_instance_id();
        let runtime = self.runtime_manager.get_runtime(&task.spec.runtime_type)
            .ok_or_else(|| ExecutionError::RuntimeError {
                message: format!("Runtime not found for type: {:?}", task.spec.runtime_type),
            })?;

        let instance_config = task.create_instance_config();
        let instance = timeout(
            Duration::from_millis(self.config.instance_creation_timeout_ms),
            runtime.create_instance(&instance_config),
        )
        .await
        .map_err(|_| ExecutionError::ExecutionTimeout {
            timeout_ms: self.config.instance_creation_timeout_ms,
        })??;

        // Start the instance / 启动实例
        runtime.start_instance(&instance).await?;

        // Register instance / 注册实例
        self.instances.insert(instance_id.clone(), instance.clone());
        task.add_instance(instance.clone())?;
        self.scheduler.add_instance(instance.clone()).await?;

        // Update statistics / 更新统计信息
        {
            let mut stats = self.statistics.write();
            stats.active_instances = self.instances.len() as u64;
        }

        info!("Created new instance: {}", instance_id);
        Ok(instance)
    }

    /// Stop instance / 停止实例
    async fn stop_instance(&self, instance: &Arc<TaskInstance>) -> ExecutionResult<()> {
        let runtime = self.runtime_manager.get_runtime(&instance.config.runtime_type)
            .ok_or_else(|| ExecutionError::RuntimeError {
                message: format!("Runtime not found for type: {:?}", instance.config.runtime_type),
            })?;
        runtime.stop_instance(instance).await?;
        
        self.instances.remove(instance.id());
        self.scheduler.remove_instance(&instance.id).await?;

        // Update statistics / 更新统计信息
        {
            let mut stats = self.statistics.write();
            stats.active_instances = self.instances.len() as u64;
        }

        info!("Stopped instance: {}", instance.id());
        Ok(())
    }

    /// Health check loop / 健康检查循环
    async fn run_health_check_loop(&self) {
        let mut interval = tokio::time::interval(Duration::from_millis(
            self.config.health_check_interval_ms,
        ));

        loop {
            interval.tick().await;

            for instance_entry in self.instances.iter() {
                let instance = instance_entry.value();
                if let Some(runtime) = self.runtime_manager.get_runtime(&instance.config.runtime_type) {
                    if let Err(e) = runtime.health_check(instance).await {
                        warn!("Health check failed for instance {}: {}", instance.id(), e);
                        
                        // Mark instance as unhealthy / 标记实例为不健康
                        instance.set_status(InstanceStatus::Unhealthy);
                    }
                }
            }
        }
    }

    /// Metrics collection loop / 指标收集循环
    async fn run_metrics_collection_loop(&self) {
        let mut interval = tokio::time::interval(Duration::from_millis(
            self.config.metrics_collection_interval_ms,
        ));

        loop {
            interval.tick().await;

            for instance_entry in self.instances.iter() {
                let instance = instance_entry.value();
                if let Some(runtime) = self.runtime_manager.get_runtime(&instance.config.runtime_type) {
                    if let Ok(metrics) = runtime.get_metrics(instance).await {
                        debug!("Collected metrics for instance {}: {:?}", instance.id(), metrics);
                    }
                }
            }
        }
    }

    /// Cleanup loop / 清理循环
    async fn run_cleanup_loop(&self) {
        let mut interval = tokio::time::interval(Duration::from_millis(
            self.config.cleanup_interval_ms,
        ));

        loop {
            interval.tick().await;

            let _now = SystemTime::now();

            // Cleanup idle instances / 清理空闲实例
            let mut instances_to_remove = Vec::new();
            let idle_threshold = Duration::from_millis(self.config.instance_idle_timeout_ms);
            for instance_entry in self.instances.iter() {
                let instance = instance_entry.value();
                if instance.is_idle(idle_threshold) {
                    instances_to_remove.push(instance.clone());
                }
            }

            for instance in instances_to_remove {
                if let Err(e) = self.stop_instance(&instance).await {
                    warn!("Failed to cleanup idle instance {}: {}", instance.id(), e);
                }
            }

            // Cleanup idle tasks / 清理空闲任务
            let mut tasks_to_remove = Vec::new();
            for task_entry in self.tasks.iter() {
                let task_id = task_entry.key().clone();
                let task = task_entry.value();
                if task.instance_count() == 0 {
                    let idle_duration = task.time_since_update();
                    if idle_duration.as_millis() > self.config.task_idle_timeout_ms as u128 {
                        tasks_to_remove.push(task_id);
                    }
                }
            }

            for task_id in tasks_to_remove {
                self.tasks.remove(&task_id);
                info!("Cleaned up idle task: {}", task_id);
            }

            // Cleanup idle artifacts / 清理空闲 artifact
            let mut artifacts_to_remove = Vec::new();
            for artifact_entry in self.artifacts.iter() {
                let artifact_id = artifact_entry.key().clone();
                let artifact = artifact_entry.value();
                if artifact.task_count() == 0 {
                    let idle_duration = artifact.time_since_update();
                    if idle_duration.as_millis() > self.config.artifact_idle_timeout_ms as u128 {
                        artifacts_to_remove.push(artifact_id);
                    }
                }
            }

            for artifact_id in artifacts_to_remove {
                if let Some((_, _artifact)) = self.artifacts.remove(&artifact_id) {
                    info!("Cleaned up idle artifact: {}", artifact_id);
                }
            }

            // Update statistics / 更新统计信息
            {
                let mut stats = self.statistics.write();
                stats.active_artifacts = self.artifacts.len() as u64;
                stats.active_tasks = self.tasks.len() as u64;
                stats.active_instances = self.instances.len() as u64;
            }
        }
    }

    /// Extract error message from RuntimeExecutionError enum / 从RuntimeExecutionError枚举中提取错误消息
    fn extract_error_message(error: &super::runtime::RuntimeExecutionError) -> String {
        use super::runtime::RuntimeExecutionError;
        match error {
            RuntimeExecutionError::InstanceNotFound { instance_id } => format!("Instance not found: {}", instance_id),
            RuntimeExecutionError::InstanceNotReady { instance_id } => format!("Instance not ready: {}", instance_id),
            RuntimeExecutionError::ExecutionTimeout { timeout_ms } => format!("Execution timeout after {} ms", timeout_ms),
            RuntimeExecutionError::ResourceLimitExceeded { resource, limit } => format!("Resource limit exceeded: {} (limit: {})", resource, limit),
            RuntimeExecutionError::ConfigurationError { message } => message.clone(),
            RuntimeExecutionError::RuntimeError { message } => message.clone(),
            RuntimeExecutionError::IoError { message } => message.clone(),
            RuntimeExecutionError::SerializationError { message } => message.clone(),
            RuntimeExecutionError::UnsupportedOperation { operation, runtime_type } => format!("Unsupported operation: {} for runtime: {}", operation, runtime_type),
        }
    }
}

impl Clone for TaskExecutionManager {
    fn clone(&self) -> Self {
        Self {
            config: self.config.clone(),
            runtime_manager: self.runtime_manager.clone(),
            scheduler: self.scheduler.clone(),
            artifacts: self.artifacts.clone(),
            tasks: self.tasks.clone(),
            instances: self.instances.clone(),
            request_queue: self.request_queue.clone(),
            execution_semaphore: self.execution_semaphore.clone(),
            statistics: self.statistics.clone(),
            request_counter: AtomicU64::new(self.request_counter.load(Ordering::SeqCst)),
            request_sender: self.request_sender.clone(),
            shutdown_sender: None, // Clone doesn't get shutdown sender / 克隆不获取关闭发送器
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::spearlet::execution::runtime::{RuntimeConfig, RuntimeType};

    #[tokio::test]
    async fn test_task_execution_manager_creation() {
        let config = TaskExecutionManagerConfig::default();
        let runtime_manager = Arc::new(RuntimeManager::new());
        
        let manager = TaskExecutionManager::new(config, runtime_manager).await;
        assert!(manager.is_ok());
    }

    #[tokio::test]
    async fn test_execution_statistics() {
        let mut stats = ExecutionStatistics::default();
        assert_eq!(stats.total_executions, 0);
        assert_eq!(stats.successful_executions, 0);
        assert_eq!(stats.failed_executions, 0);
        
        stats.total_executions = 10;
        stats.successful_executions = 8;
        stats.failed_executions = 2;
        stats.total_execution_time_ms = 5000;
        stats.average_execution_time_ms = 500.0;
        
        assert_eq!(stats.total_executions, 10);
        assert_eq!(stats.successful_executions, 8);
        assert_eq!(stats.failed_executions, 2);
    }

    #[test]
    fn test_task_execution_manager_config() {
        let config = TaskExecutionManagerConfig::default();
        assert_eq!(config.max_concurrent_executions, 1000);
        assert_eq!(config.max_artifacts, 100);
        assert_eq!(config.max_tasks_per_artifact, 10);
        assert_eq!(config.max_instances_per_task, 50);
    }
}