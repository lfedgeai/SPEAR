//! Task Execution Manager
//! 任务执行管理器
//!
//! This module provides the central task execution management system that coordinates
//! artifacts, tasks, instances, and runtime execution.
//! 该模块提供中央任务执行管理系统，协调 artifact、任务、实例和运行时执行。

use super::runtime::RuntimeType;
use super::{
    artifact::{Artifact, ArtifactId},
    instance::{InstanceId, InstanceStatus, TaskInstance},
    runtime::{ExecutionContext, RuntimeManager},
    scheduler::{InstanceScheduler, SchedulingPolicy},
    task::{Task, TaskId},
    ExecutionError, ExecutionResult,
};
use crate::proto::spearlet::{ArtifactSpec as ProtoArtifactSpec, InvokeFunctionRequest};
use dashmap::DashMap;
use parking_lot::RwLock;
use serde::{Deserialize, Serialize};
use std::sync::atomic::{AtomicU64, Ordering};
use std::sync::Arc;
use std::time::{Duration, Instant, SystemTime};
use tokio::sync::{mpsc, oneshot, Semaphore};
use tokio::time::timeout;
use tonic::transport::Channel;
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
            artifact_idle_timeout_ms: 300000, // 5 minutes
            task_idle_timeout_ms: 180000,     // 3 minutes
            instance_idle_timeout_ms: 120000, // 2 minutes
        }
    }
}

/// Execution request / 执行请求
#[derive(Debug)]
pub struct ExecutionRequest {
    /// Execution ID / 执行 ID
    pub execution_id: String,
    /// Artifact specification / Artifact 规范
    pub artifact_spec: ProtoArtifactSpec,
    /// Execution context / 执行上下文
    pub execution_context: ExecutionContext,
    /// Desired task ID from request (if any) / 来自请求的期望任务ID（如有）
    pub desired_task_id: Option<String>,
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
    /// Spearlet application configuration / SPEARlet应用配置
    spearlet_config: Arc<crate::spearlet::config::SpearletConfig>,
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
    /// Execution status storage / 执行状态存储
    executions: Arc<DashMap<String, super::ExecutionResponse>>,
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
        spearlet_config: Arc<crate::spearlet::config::SpearletConfig>,
    ) -> ExecutionResult<Arc<Self>> {
        let scheduler = Arc::new(InstanceScheduler::new(SchedulingPolicy::RoundRobin));
        let execution_semaphore = Arc::new(Semaphore::new(config.max_concurrent_executions));

        let (request_sender, request_receiver) = mpsc::unbounded_channel();
        let (shutdown_sender, shutdown_receiver) = oneshot::channel();

        let manager = Arc::new(Self {
            config: config.clone(),
            spearlet_config,
            runtime_manager,
            scheduler,
            artifacts: Arc::new(DashMap::new()),
            tasks: Arc::new(DashMap::new()),
            instances: Arc::new(DashMap::new()),
            executions: Arc::new(DashMap::new()),
            execution_semaphore,
            statistics: Arc::new(RwLock::new(ExecutionStatistics::default())),
            request_counter: AtomicU64::new(0),
            request_sender,
            shutdown_sender: Some(shutdown_sender),
        });

        // Start background tasks / 启动后台任务
        let manager_clone = manager.clone();
        tokio::spawn(async move {
            manager_clone
                .run_execution_loop(request_receiver, shutdown_receiver)
                .await;
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

    pub fn list_runtime_types(&self) -> Vec<RuntimeType> {
        self.runtime_manager.list_runtime_types()
    }

    /// Submit execution request / 提交执行请求
    pub async fn submit_execution(
        &self,
        request: InvokeFunctionRequest,
    ) -> ExecutionResult<super::ExecutionResponse> {
        let execution_id = request
            .execution_id
            .clone()
            .filter(|id| !id.is_empty())
            .unwrap_or_else(|| {
                format!(
                    "req-{}",
                    self.request_counter.fetch_add(1, Ordering::SeqCst)
                )
            });

        let (execution_mode, wait) = {
            let m = request.execution_mode();
            let wait = request.wait;
            let m2 = match m {
                crate::proto::spearlet::ExecutionMode::Sync => {
                    crate::spearlet::execution::runtime::ExecutionMode::Sync
                }
                crate::proto::spearlet::ExecutionMode::Async => {
                    crate::spearlet::execution::runtime::ExecutionMode::Async
                }
                crate::proto::spearlet::ExecutionMode::Stream => {
                    crate::spearlet::execution::runtime::ExecutionMode::Stream
                }
                crate::proto::spearlet::ExecutionMode::Unknown => {
                    crate::spearlet::execution::runtime::ExecutionMode::Unknown
                }
            };
            (m2, wait)
        };

        let artifact_spec =
            request
                .artifact_spec
                .clone()
                .ok_or_else(|| ExecutionError::InvalidRequest {
                    message: "Missing artifact specification".to_string(),
                })?;

        let execution_context = ExecutionContext {
            execution_id: execution_id.clone(),
            payload: Vec::new(), // TODO: Extract payload from request
            headers: std::collections::HashMap::new(), // TODO: Extract headers from request
            timeout_ms: 30000,   // TODO: Extract timeout from request context
            execution_mode,
            wait,
            context_data: std::collections::HashMap::new(), // TODO: Extract context data from request
        };

        let (response_sender, response_receiver) = oneshot::channel();

        let timestamp = SystemTime::now();

        // Desired task id from request / 从请求提取期望task id
        let desired_task_id = if request.task_id.is_empty() {
            None
        } else {
            Some(request.task_id.clone())
        };

        let mut meta = std::collections::HashMap::new();
        if let Some(task_id) = desired_task_id.clone() {
            if !task_id.is_empty() {
                meta.insert("task_id".to_string(), task_id);
            }
        }
        meta.insert("artifact_id".to_string(), artifact_spec.artifact_id.clone());

        self.executions.insert(
            execution_id.clone(),
            super::ExecutionResponse {
                execution_id: execution_id.clone(),
                output_data: Vec::new(),
                status: "pending".to_string(),
                error_message: None,
                execution_time_ms: 0,
                metadata: meta,
                timestamp: SystemTime::now(),
            },
        );

        let execution_request = ExecutionRequest {
            execution_id: execution_id.clone(),
            artifact_spec,
            execution_context,
            desired_task_id,
            response_sender,
            timestamp,
        };

        // Send to execution loop / 发送到执行循环
        self.request_sender
            .send(execution_request)
            .map_err(|_| ExecutionError::RuntimeError {
                message: "Failed to submit execution request".to_string(),
            })?;

        // Wait for response / 等待响应
        response_receiver
            .await
            .map_err(|_| ExecutionError::RuntimeError {
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
        let mut stats = self.statistics.read().clone();

        let mut pending = 0u64;
        let mut running = 0u64;
        for entry in self.executions.iter() {
            match entry.value().status.as_str() {
                "pending" => pending += 1,
                "running" => running += 1,
                _ => {}
            }
        }

        stats.queue_size = pending;
        stats.pending_executions = pending;
        stats.running_executions = running;
        stats
    }

    /// Get execution status by execution ID / 根据执行ID获取执行状态
    pub async fn get_execution_status(
        &self,
        execution_id: &str,
    ) -> ExecutionResult<Option<super::ExecutionResponse>> {
        Ok(self
            .executions
            .get(execution_id)
            .map(|entry| entry.value().clone()))
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
                    tokio::spawn(async move { manager.handle_execution_request(request).await });
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
        let execution_id = request.execution_id.clone();
        debug!(execution_id = %execution_id, "Execution request received");

        // Update statistics / 更新统计信息
        {
            let mut stats = self.statistics.write();
            stats.total_executions += 1;
            stats.running_executions += 1;
        }

        self.executions
            .entry(execution_id.clone())
            .and_modify(|e| {
                e.status = "running".to_string();
                e.timestamp = SystemTime::now();
            })
            .or_insert_with(|| super::ExecutionResponse {
                execution_id: execution_id.clone(),
                output_data: Vec::new(),
                status: "running".to_string(),
                error_message: None,
                execution_time_ms: 0,
                metadata: std::collections::HashMap::new(),
                timestamp: SystemTime::now(),
            });

        // Acquire execution permit / 获取执行许可
        let _permit = match self.execution_semaphore.acquire().await {
            Ok(permit) => permit,
            Err(_) => {
                let _ = request
                    .response_sender
                    .send(Err(ExecutionError::RuntimeError {
                        message: "Failed to acquire execution permit".to_string(),
                    }));
                warn!(execution_id = %execution_id, "Failed to acquire execution permit");
                return;
            }
        };
        let artifact_id = request.artifact_spec.artifact_id.clone();
        debug!(execution_id = %execution_id, artifact_id = %artifact_id, "Starting execution");
        let result = self
            .execute_request(
                request.artifact_spec,
                request.execution_context,
                request.desired_task_id.clone(),
            )
            .await;

        let execution_time = start_time.elapsed();
        let execution_time_ms = execution_time.as_millis() as u64;
        match &result {
            Ok(resp) => {
                debug!(execution_id = %execution_id, status = %resp.status, duration_ms = execution_time_ms, "Execution finished");
            }
            Err(e) => {
                warn!(execution_id = %execution_id, error = %e.to_string(), duration_ms = execution_time_ms, "Execution failed");
            }
        }

        // Update statistics / 更新统计信息
        {
            let mut stats = self.statistics.write();
            stats.total_execution_time_ms += execution_time_ms;
            stats.average_execution_time_ms =
                stats.total_execution_time_ms as f64 / stats.total_executions as f64;

            stats.running_executions = stats.running_executions.saturating_sub(1);
            stats.completed_executions += 1;

            match &result {
                Ok(resp) if resp.is_completed() && resp.is_successful() => {
                    stats.successful_executions += 1
                }
                Ok(resp) if resp.is_completed() && !resp.is_successful() => {
                    stats.failed_executions += 1
                }
                Err(_) => stats.failed_executions += 1,
                _ => {}
            }
        }

        match &result {
            Ok(resp) => {
                self.executions.insert(execution_id.clone(), resp.clone());
            }
            Err(e) => {
                self.executions.insert(
                    execution_id.clone(),
                    super::ExecutionResponse {
                        execution_id: execution_id.clone(),
                        output_data: Vec::new(),
                        status: "failed".to_string(),
                        error_message: Some(e.to_string()),
                        execution_time_ms,
                        metadata: std::collections::HashMap::new(),
                        timestamp: SystemTime::now(),
                    },
                );
            }
        }

        // Publish task result to SMS / 将任务结果回写到SMS
        match &result {
            Ok(resp) if resp.is_completed() => {
                let result_status = resp.status.clone();
                let completed_at = chrono::Utc::now().timestamp();
                let mut meta = resp.metadata.clone();
                meta.insert(
                    "execution_time_ms".to_string(),
                    resp.execution_time_ms.to_string(),
                );
                meta.insert("execution_id".to_string(), resp.execution_id.clone());
                if let Some(err) = &resp.error_message {
                    meta.insert("error_message".to_string(), err.clone());
                }
                let task_id = request.desired_task_id.clone().unwrap_or_default();
                if !task_id.is_empty() {
                    self.publish_task_result(
                        &task_id,
                        "".to_string(),
                        result_status,
                        completed_at,
                        meta,
                    )
                    .await;
                }
            }
            Err(e) => {
                let completed_at = chrono::Utc::now().timestamp();
                let mut meta = std::collections::HashMap::new();
                meta.insert(
                    "execution_time_ms".to_string(),
                    execution_time_ms.to_string(),
                );
                meta.insert("execution_id".to_string(), execution_id.clone());
                meta.insert("error_message".to_string(), e.to_string());
                let task_id = request.desired_task_id.clone().unwrap_or_default();
                if !task_id.is_empty() {
                    self.publish_task_result(
                        &task_id,
                        "".to_string(),
                        "failed".to_string(),
                        completed_at,
                        meta,
                    )
                    .await;
                }
            }
            _ => {}
        }

        // Send response / 发送响应
        let _ = request.response_sender.send(result);
    }

    /// Execute request / 执行请求
    async fn execute_request(
        &self,
        _artifact_spec: ProtoArtifactSpec,
        execution_context: ExecutionContext,
        desired_task_id: Option<String>,
    ) -> ExecutionResult<super::ExecutionResponse> {
        let task = if let Some(id) = &desired_task_id {
            if let Some(t) = self.tasks.get(id) {
                t.clone()
            } else {
                self.ensure_task_available_from_sms(id).await?
            }
        } else {
            return Err(ExecutionError::NotSupported {
                operation: "new_task_invocation_via_execute_request_disabled".to_string(),
            });
        };

        // Get or create instance / 获取或创建实例
        let instance = self.get_or_create_instance(&task).await?;

        // Execute on instance / 在实例上执行
        let runtime = self
            .runtime_manager
            .get_runtime(&task.spec.runtime_type)
            .ok_or_else(|| ExecutionError::RuntimeError {
                message: format!("Runtime not found for type: {:?}", task.spec.runtime_type),
            })?;
        let execution_id = execution_context.execution_id.clone();
        let runtime_response = runtime.execute(&instance, execution_context).await?;

        // Convert RuntimeExecutionResponse to ExecutionResponse / 转换运行时响应到执行响应
        let is_successful = runtime_response.is_successful();
        let has_failed = runtime_response.has_failed();
        let is_running = matches!(
            runtime_response.execution_status,
            crate::spearlet::execution::runtime::ExecutionStatus::Running
        );
        let error_message = runtime_response
            .error
            .as_ref()
            .map(Self::extract_error_message);
        let duration_ms = runtime_response.duration_ms;
        let metadata = runtime_response
            .metadata
            .into_iter()
            .map(|(k, v)| (k, v.to_string()))
            .collect();
        let data = runtime_response.data;

        Ok(super::ExecutionResponse {
            execution_id,
            output_data: data,
            status: if is_successful {
                "completed".to_string()
            } else if has_failed {
                "failed".to_string()
            } else if is_running {
                "running".to_string()
            } else {
                "pending".to_string()
            },
            error_message,
            execution_time_ms: duration_ms,
            metadata,
            timestamp: SystemTime::now(),
        })
    }

    pub fn get_artifact_by_id(&self, artifact_id: &str) -> Option<Arc<Artifact>> {
        self.artifacts.get(artifact_id).map(|a| a.clone())
    }

    pub fn create_artifact_with_id(
        &self,
        artifact_id: String,
        spec: super::artifact::ArtifactSpec,
    ) -> ExecutionResult<Arc<Artifact>> {
        if self.artifacts.len() >= self.config.max_artifacts {
            return Err(ExecutionError::ResourceExhausted {
                message: format!(
                    "Maximum artifacts limit reached: {}",
                    self.config.max_artifacts
                ),
            });
        }
        let artifact = Arc::new(Artifact::new_with_id(artifact_id.clone(), spec));
        self.artifacts.insert(artifact_id.clone(), artifact.clone());
        {
            let mut stats = self.statistics.write();
            stats.active_artifacts = self.artifacts.len() as u64;
        }
        Ok(artifact)
    }

    pub fn ensure_artifact_with_id(
        &self,
        artifact_id: String,
        spec: super::artifact::ArtifactSpec,
    ) -> ExecutionResult<Arc<Artifact>> {
        if let Some(existing) = self.get_artifact_by_id(&artifact_id) {
            return Ok(existing);
        }
        self.create_artifact_with_id(artifact_id, spec)
    }

    /// Ensure artifact exists from SMS Task / 从 SMS Task 确保 Artifact 存在
    pub async fn ensure_artifact_from_sms(
        &self,
        sms_task: &crate::proto::sms::Task,
    ) -> ExecutionResult<Arc<Artifact>> {
        let (runtime_type, location_opt, checksum_opt, env) = if let Some(ex) = &sms_task.executable
        {
            let rt = match ex.r#type {
                3 => super::RuntimeType::Kubernetes,
                4 => super::RuntimeType::Wasm,
                _ => super::RuntimeType::Process,
            };
            let loc = if ex.uri.is_empty() {
                None
            } else {
                Some(ex.uri.clone())
            };
            let chk = if ex.checksum_sha256.is_empty() {
                None
            } else {
                Some(ex.checksum_sha256.clone())
            };
            (rt, loc, chk, ex.env.clone())
        } else {
            (
                super::RuntimeType::Process,
                None,
                None,
                std::collections::HashMap::new(),
            )
        };

        let artifact_id = if let Some(chk) = &checksum_opt {
            chk.clone()
        } else if let Some(loc) = &location_opt {
            use sha2::Digest;
            let d = sha2::Sha256::digest(loc.as_bytes());
            d.iter().map(|b| format!("{:02x}", b)).collect()
        } else {
            uuid::Uuid::new_v4().to_string()
        };

        use super::artifact::{ArtifactSpec, InvocationType, ResourceLimits};
        let spec = ArtifactSpec {
            name: artifact_id.clone(),
            version: sms_task.version.clone(),
            description: None,
            runtime_type,
            runtime_config: std::collections::HashMap::new(),
            location: location_opt,
            checksum_sha256: checksum_opt,
            environment: env,
            resource_limits: ResourceLimits::default(),
            invocation_type: InvocationType::ExistingTask,
            max_execution_timeout_ms: 30000,
            labels: sms_task.metadata.clone(),
        };
        self.ensure_artifact_with_id(artifact_id, spec)
    }

    /// Task helpers / 任务相关辅助方法
    pub fn get_task_by_id(&self, task_id: &str) -> Option<Arc<Task>> {
        self.tasks.get(task_id).map(|t| t.clone())
    }

    pub fn create_task_with_id(
        &self,
        task_id: String,
        artifact: &Arc<Artifact>,
        spec: super::task::TaskSpec,
    ) -> ExecutionResult<Arc<Task>> {
        if artifact.task_count() >= self.config.max_tasks_per_artifact {
            return Err(ExecutionError::ResourceExhausted {
                message: format!(
                    "Maximum tasks per artifact limit reached: {}",
                    self.config.max_tasks_per_artifact
                ),
            });
        }
        let task = Arc::new(Task::new_with_id(
            task_id.clone(),
            artifact.id().to_string(),
            spec,
        ));
        self.tasks.insert(task_id.clone(), task.clone());
        artifact.add_task(task.clone())?;
        {
            let mut stats = self.statistics.write();
            stats.active_tasks = self.tasks.len() as u64;
        }
        Ok(task)
    }

    pub fn ensure_task_with_id(
        &self,
        task_id: String,
        artifact: &Arc<Artifact>,
        spec: super::task::TaskSpec,
    ) -> ExecutionResult<Arc<Task>> {
        // Fast path: task already exists locally.
        // 快速路径：本地已存在 task 直接返回。
        if let Some(existing) = self.get_task_by_id(&task_id) {
            return Ok(existing);
        }
        // Slow path: create task and attach it under the artifact.
        // 慢路径：创建 task 并挂到 artifact 下。
        self.create_task_with_id(task_id, artifact, spec)
    }

    /// Ensure task exists from SMS Task using provided artifact / 使用提供的Artifact从 SMS Task 确保 Task 存在
    pub async fn ensure_task_from_sms(
        &self,
        sms_task: &crate::proto::sms::Task,
        artifact: &Arc<Artifact>,
    ) -> ExecutionResult<Arc<Task>> {
        // Convert SMS task model into Spearlet TaskSpec.
        // 将 SMS 的 task 模型转换成 Spearlet 侧的 TaskSpec。
        use super::task::{
            ExecutionKind, HealthCheckConfig, ScalingConfig, TaskSpec, TimeoutConfig,
        };
        use std::collections::HashMap;
        let env = if let Some(ex) = &sms_task.executable {
            ex.env.clone()
        } else {
            std::collections::HashMap::new()
        };
        let runtime_type = artifact.spec.runtime_type;
        let task_spec = TaskSpec {
            name: sms_task.name.clone(),
            task_type: super::task::TaskType::HttpHandler,
            runtime_type,
            entry_point: "main".to_string(),
            handler_config: HashMap::new(),
            environment: env,
            invocation_type: super::artifact::InvocationType::ExistingTask,
            min_instances: 1,
            max_instances: 10,
            target_concurrency: 100,
            scaling_config: ScalingConfig::default(),
            health_check: HealthCheckConfig::default(),
            timeout_config: TimeoutConfig::default(),
            execution_kind: match sms_task.execution_kind {
                x if x == crate::proto::sms::TaskExecutionKind::LongRunning as i32 => {
                    ExecutionKind::LongRunning
                }
                _ => ExecutionKind::ShortRunning,
            },
        };
        self.ensure_task_with_id(sms_task.task_id.clone(), artifact, task_spec)
    }

    async fn ensure_task_available_from_sms(&self, task_id: &str) -> ExecutionResult<Arc<Task>> {
        // When an invocation lands on a node that doesn't have the task yet,
        // fetch task metadata from SMS and materialize it locally.
        //
        // 当调用落到一个尚未持有该 task 的节点时，从 SMS 拉取 task 元数据并在本地补齐。
        let addr = self.spearlet_config.sms_grpc_addr.clone();
        let url = format!("http://{}", addr);
        let mut client = crate::proto::sms::task_service_client::TaskServiceClient::new(
            Channel::from_shared(url)
                .map_err(|e| ExecutionError::RuntimeError {
                    message: e.to_string(),
                })?
                .connect()
                .await
                .map_err(|e| ExecutionError::RuntimeError {
                    message: e.to_string(),
                })?,
        );
        // Query SMS for the task definition.
        // 向 SMS 查询 task 定义。
        let resp = client
            .get_task(crate::proto::sms::GetTaskRequest {
                task_id: task_id.to_string(),
            })
            .await
            .map_err(|e| ExecutionError::RuntimeError {
                message: e.to_string(),
            })?
            .into_inner();
        if !resp.found {
            return Err(ExecutionError::TaskNotFound {
                id: task_id.to_string(),
            });
        }
        let sms_task = resp.task.ok_or_else(|| ExecutionError::TaskNotFound {
            id: task_id.to_string(),
        })?;
        // Ensure artifact/task are present locally before execution.
        // 执行前确保本地已有 artifact/task。
        let artifact = self.ensure_artifact_from_sms(&sms_task).await?;
        self.ensure_task_from_sms(&sms_task, &artifact).await
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
        let runtime = self
            .runtime_manager
            .get_runtime(&task.spec.runtime_type)
            .ok_or_else(|| ExecutionError::RuntimeError {
                message: format!("Runtime not found for type: {:?}", task.spec.runtime_type),
            })?;

        let mut instance_config = task.create_instance_config();
        // Inject ArtifactSnapshot into InstanceConfig / 在实例配置中注入 ArtifactSnapshot
        if let Some(artifact_entry) = self.artifacts.get(task.artifact_id()) {
            let artifact = artifact_entry.value();
            instance_config.artifact = Some(super::instance::ArtifactSnapshot {
                location: artifact.spec.location.clone(),
                checksum_sha256: artifact.spec.checksum_sha256.clone(),
            });
            debug!(
                task_id = %task.id(),
                artifact_id = %task.artifact_id(),
                location = %artifact.spec.location.clone().unwrap_or_default(),
                checksum = %artifact.spec.checksum_sha256.clone().unwrap_or_default(),
                "Injected artifact snapshot into instance config"
            );
        } else {
            debug!(
                task_id = %task.id(),
                artifact_id = %task.artifact_id(),
                "Artifact not found in manager when preparing instance; snapshot injection skipped"
            );
        }
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
        self.instances
            .insert(instance.id().to_string(), instance.clone());
        task.add_instance(instance.clone())?;
        self.scheduler.add_instance(instance.clone()).await?;

        // Report ACTIVE status / 上报ACTIVE状态
        self.publish_task_status(
            task.id(),
            crate::proto::sms::TaskStatus::Active,
            Some("instance initialized".to_string()),
        )
        .await;

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
        let runtime = self
            .runtime_manager
            .get_runtime(&instance.config.runtime_type)
            .ok_or_else(|| ExecutionError::RuntimeError {
                message: format!(
                    "Runtime not found for type: {:?}",
                    instance.config.runtime_type
                ),
            })?;
        runtime.stop_instance(instance).await?;

        self.instances.remove(instance.id());
        self.scheduler.remove_instance(&instance.id).await?;

        let task_id = instance.task_id().to_string();
        if let Some(task_entry) = self
            .tasks
            .iter()
            .find(|entry| entry.value().id() == task_id)
        {
            let task = task_entry.value();
            if let Err(e) = task.remove_instance(instance.id()) {
                warn!(
                    "Failed to remove instance {} from task {}: {}",
                    instance.id(),
                    task_id,
                    e
                );
            }
            if task.instance_count() == 0 {
                self.publish_task_status(
                    &task_id,
                    crate::proto::sms::TaskStatus::Inactive,
                    Some("no instances".to_string()),
                )
                .await;
            }
        }

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
        let mut interval =
            tokio::time::interval(Duration::from_millis(self.config.health_check_interval_ms));

        loop {
            interval.tick().await;
            self.process_health_checks_once().await;
        }
    }

    async fn process_health_checks_once(&self) {
        let instances: Vec<Arc<TaskInstance>> =
            self.instances.iter().map(|e| e.value().clone()).collect();
        for instance in instances {
            if let Some(runtime) = self
                .runtime_manager
                .get_runtime(&instance.config.runtime_type)
            {
                match runtime.health_check(&instance).await {
                    Ok(true) => {
                        instance.update_metrics(|m| {
                            m.health_check_successes = m.health_check_successes.saturating_add(1);
                            m.health_check_failures = 0;
                            m.last_health_check_time = Some(SystemTime::now());
                        });
                    }
                    Ok(false) | Err(_) => {
                        instance.set_status(InstanceStatus::Unhealthy);
                        instance.update_metrics(|m| {
                            m.health_check_failures = m.health_check_failures.saturating_add(1);
                            m.last_health_check_time = Some(SystemTime::now());
                        });

                        let threshold = self
                            .tasks
                            .iter()
                            .find(|t| t.value().id() == instance.task_id())
                            .map(|t| t.value().spec.health_check.failure_threshold)
                            .unwrap_or(1);

                        if instance.get_metrics().health_check_failures >= threshold {
                            let _ = self.stop_instance(&instance).await;
                        }
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
                if let Some(runtime) = self
                    .runtime_manager
                    .get_runtime(&instance.config.runtime_type)
                {
                    if let Ok(metrics) = runtime.get_metrics(instance).await {
                        debug!(
                            "Collected metrics for instance {}: {:?}",
                            instance.id(),
                            metrics
                        );
                    }
                }
            }
        }
    }

    /// Cleanup loop / 清理循环
    async fn run_cleanup_loop(&self) {
        let mut interval =
            tokio::time::interval(Duration::from_millis(self.config.cleanup_interval_ms));

        loop {
            interval.tick().await;

            let now = SystemTime::now();

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
                    let not_active = !matches!(
                        task.status(),
                        super::task::TaskStatus::Ready | super::task::TaskStatus::Running
                    );
                    if not_active
                        || idle_duration.as_millis() > self.config.task_idle_timeout_ms as u128
                    {
                        tasks_to_remove.push(task_id);
                    }
                }
            }

            for task_id in tasks_to_remove {
                if let Some((_, task)) = self.tasks.remove(&task_id) {
                    // Publish UNREGISTERED before removal / 移除前上报UNREGISTERED状态
                    self.publish_task_status(
                        task.id(),
                        crate::proto::sms::TaskStatus::Unregistered,
                        Some("cleanup".to_string()),
                    )
                    .await;
                    if let Some(artifact_entry) = self.artifacts.get(task.artifact_id()) {
                        let artifact = artifact_entry.value();
                        if let Err(e) = artifact.remove_task(task.id()) {
                            warn!(
                                "Failed to remove task {} from artifact {}: {}",
                                task_id,
                                task.artifact_id(),
                                e
                            );
                        }
                    }
                    info!("Cleaned up idle task: {}", task_id);
                } else {
                    info!("Cleaned up idle task: {}", task_id);
                }
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

            let completed_execution_ttl = Duration::from_millis(self.config.task_idle_timeout_ms);
            let mut executions_to_remove = Vec::new();
            for entry in self.executions.iter() {
                let e = entry.value();
                if !e.is_completed() {
                    continue;
                }
                if let Ok(age) = now.duration_since(e.timestamp) {
                    if age > completed_execution_ttl {
                        executions_to_remove.push(entry.key().clone());
                    }
                }
            }
            for execution_id in executions_to_remove {
                self.executions.remove(&execution_id);
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

    async fn publish_task_status(
        &self,
        task_id: &str,
        status: crate::proto::sms::TaskStatus,
        reason: Option<String>,
    ) {
        let addr = self.spearlet_config.sms_grpc_addr.clone();
        let url = format!("http://{}", addr);
        let node_uuid = {
            let cfg = &self.spearlet_config;
            let base = format!(
                "{}:{}:{}",
                cfg.grpc.addr.ip(),
                cfg.grpc.addr.port(),
                cfg.node_name
            );
            uuid::Uuid::new_v5(&uuid::Uuid::NAMESPACE_OID, base.as_bytes()).to_string()
        };
        let req = crate::proto::sms::UpdateTaskStatusRequest {
            task_id: task_id.to_string(),
            status: status as i32,
            node_uuid,
            status_version: 0,
            updated_at: chrono::Utc::now().timestamp(),
            reason: reason.unwrap_or_default(),
        };
        debug!(task_id = %req.task_id, node_uuid = %req.node_uuid, status = req.status, updated_at = req.updated_at, reason = %req.reason, url = %url, "publish_task_status: sending request");
        tokio::spawn(async move {
            match Channel::from_shared(url).unwrap().connect().await {
                Ok(channel) => {
                    let mut client =
                        crate::proto::sms::task_service_client::TaskServiceClient::new(channel);
                    match client.update_task_status(req).await {
                        Ok(resp) => {
                            let inner = resp.into_inner();
                            debug!(success = inner.success, message = %inner.message, "publish_task_status: response received");
                        }
                        Err(e) => {
                            warn!(error = %e.to_string(), "publish_task_status: rpc call failed");
                        }
                    }
                }
                Err(e) => {
                    warn!(error = %e.to_string(), "publish_task_status: connect failed");
                }
            }
        });
    }

    async fn publish_task_result(
        &self,
        task_id: &str,
        result_uri: String,
        result_status: String,
        completed_at: i64,
        result_metadata: std::collections::HashMap<String, String>,
    ) {
        let addr = self.spearlet_config.sms_grpc_addr.clone();
        let url = format!("http://{}", addr);
        let req = crate::proto::sms::UpdateTaskResultRequest {
            task_id: task_id.to_string(),
            result_uri,
            result_status,
            completed_at,
            result_metadata,
        };
        tokio::spawn(async move {
            match Channel::from_shared(url).unwrap().connect().await {
                Ok(channel) => {
                    let mut client =
                        crate::proto::sms::task_service_client::TaskServiceClient::new(channel);
                    if let Err(e) = client.update_task_result(req).await {
                        warn!(error = %e.to_string(), "publish_task_result: rpc call failed");
                    }
                }
                Err(e) => {
                    warn!(error = %e.to_string(), "publish_task_result: connect failed");
                }
            }
        });
    }

    /// Extract error message from RuntimeExecutionError enum / 从RuntimeExecutionError枚举中提取错误消息
    fn extract_error_message(error: &super::runtime::RuntimeExecutionError) -> String {
        use super::runtime::RuntimeExecutionError;
        match error {
            RuntimeExecutionError::InstanceNotFound { instance_id } => {
                format!("Instance not found: {}", instance_id)
            }
            RuntimeExecutionError::InstanceNotReady { instance_id } => {
                format!("Instance not ready: {}", instance_id)
            }
            RuntimeExecutionError::ExecutionTimeout { timeout_ms } => {
                format!("Execution timeout after {} ms", timeout_ms)
            }
            RuntimeExecutionError::ResourceLimitExceeded { resource, limit } => {
                format!("Resource limit exceeded: {} (limit: {})", resource, limit)
            }
            RuntimeExecutionError::ConfigurationError { message } => message.clone(),
            RuntimeExecutionError::RuntimeError { message } => message.clone(),
            RuntimeExecutionError::IoError { message } => message.clone(),
            RuntimeExecutionError::SerializationError { message } => message.clone(),
            RuntimeExecutionError::UnsupportedOperation {
                operation,
                runtime_type,
            } => format!(
                "Unsupported operation: {} for runtime: {}",
                operation, runtime_type
            ),
        }
    }
}

impl Clone for TaskExecutionManager {
    fn clone(&self) -> Self {
        Self {
            config: self.config.clone(),
            spearlet_config: self.spearlet_config.clone(),
            runtime_manager: self.runtime_manager.clone(),
            scheduler: self.scheduler.clone(),
            artifacts: self.artifacts.clone(),
            tasks: self.tasks.clone(),
            instances: self.instances.clone(),
            executions: self.executions.clone(),
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
    use crate::spearlet::execution::instance;
    use crate::spearlet::execution::runtime;
    use crate::spearlet::execution::runtime::{
        Runtime, RuntimeCapabilities, RuntimeConfig, RuntimeType,
    };
    use async_trait::async_trait;
    use std::collections::HashMap as StdHashMap;
    use tokio::time::sleep;

    struct DummyRuntime {
        ty: RuntimeType,
    }

    #[async_trait]
    impl Runtime for DummyRuntime {
        fn runtime_type(&self) -> RuntimeType {
            self.ty
        }
        async fn create_instance(
            &self,
            config: &instance::InstanceConfig,
        ) -> super::ExecutionResult<Arc<instance::TaskInstance>> {
            Ok(Arc::new(instance::TaskInstance::new(
                config.task_id.clone(),
                config.clone(),
            )))
        }
        async fn start_instance(
            &self,
            _instance: &Arc<instance::TaskInstance>,
        ) -> super::ExecutionResult<()> {
            Ok(())
        }
        async fn stop_instance(
            &self,
            _instance: &Arc<instance::TaskInstance>,
        ) -> super::ExecutionResult<()> {
            Ok(())
        }
        async fn execute(
            &self,
            _instance: &Arc<instance::TaskInstance>,
            _context: runtime::ExecutionContext,
        ) -> super::ExecutionResult<runtime::RuntimeExecutionResponse> {
            Ok(runtime::RuntimeExecutionResponse::default())
        }
        async fn health_check(
            &self,
            _instance: &Arc<instance::TaskInstance>,
        ) -> super::ExecutionResult<bool> {
            Ok(true)
        }
        async fn get_metrics(
            &self,
            _instance: &Arc<instance::TaskInstance>,
        ) -> super::ExecutionResult<StdHashMap<String, serde_json::Value>> {
            Ok(StdHashMap::new())
        }
        async fn scale_instance(
            &self,
            _instance: &Arc<instance::TaskInstance>,
            _new_limits: &instance::InstanceResourceLimits,
        ) -> super::ExecutionResult<()> {
            Ok(())
        }
        async fn cleanup_instance(
            &self,
            _instance: &Arc<instance::TaskInstance>,
        ) -> super::ExecutionResult<()> {
            Ok(())
        }
        fn validate_config(
            &self,
            _config: &instance::InstanceConfig,
        ) -> super::ExecutionResult<()> {
            Ok(())
        }
        fn get_capabilities(&self) -> RuntimeCapabilities {
            RuntimeCapabilities::default()
        }
    }

    struct DelayedRuntime {
        ty: RuntimeType,
        delay_ms: u64,
        payload: Vec<u8>,
    }

    #[async_trait]
    impl Runtime for DelayedRuntime {
        fn runtime_type(&self) -> RuntimeType {
            self.ty
        }
        async fn create_instance(
            &self,
            config: &instance::InstanceConfig,
        ) -> super::ExecutionResult<Arc<instance::TaskInstance>> {
            Ok(Arc::new(instance::TaskInstance::new(
                config.task_id.clone(),
                config.clone(),
            )))
        }
        async fn start_instance(
            &self,
            _instance: &Arc<instance::TaskInstance>,
        ) -> super::ExecutionResult<()> {
            Ok(())
        }
        async fn stop_instance(
            &self,
            _instance: &Arc<instance::TaskInstance>,
        ) -> super::ExecutionResult<()> {
            Ok(())
        }
        async fn execute(
            &self,
            _instance: &Arc<instance::TaskInstance>,
            context: runtime::ExecutionContext,
        ) -> super::ExecutionResult<runtime::RuntimeExecutionResponse> {
            sleep(Duration::from_millis(self.delay_ms)).await;
            Ok(runtime::RuntimeExecutionResponse::new_sync(
                context.execution_id,
                self.payload.clone(),
                self.delay_ms,
            ))
        }
        async fn health_check(
            &self,
            _instance: &Arc<instance::TaskInstance>,
        ) -> super::ExecutionResult<bool> {
            Ok(true)
        }
        async fn get_metrics(
            &self,
            _instance: &Arc<instance::TaskInstance>,
        ) -> super::ExecutionResult<StdHashMap<String, serde_json::Value>> {
            Ok(StdHashMap::new())
        }
        async fn scale_instance(
            &self,
            _instance: &Arc<instance::TaskInstance>,
            _new_limits: &instance::InstanceResourceLimits,
        ) -> super::ExecutionResult<()> {
            Ok(())
        }
        async fn cleanup_instance(
            &self,
            _instance: &Arc<instance::TaskInstance>,
        ) -> super::ExecutionResult<()> {
            Ok(())
        }
        fn validate_config(
            &self,
            _config: &instance::InstanceConfig,
        ) -> super::ExecutionResult<()> {
            Ok(())
        }
        fn get_capabilities(&self) -> RuntimeCapabilities {
            RuntimeCapabilities::default()
        }
    }

    #[tokio::test]
    async fn test_task_execution_manager_creation() {
        let config = TaskExecutionManagerConfig::default();
        let runtime_manager = Arc::new(RuntimeManager::new());

        let manager = TaskExecutionManager::new(
            config,
            runtime_manager,
            Arc::new(crate::spearlet::config::SpearletConfig::default()),
        )
        .await;
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

    #[tokio::test]
    async fn test_long_running_execution_status_tracking() {
        let mut rm = RuntimeManager::new();
        rm.register_runtime(
            RuntimeType::Process,
            Box::new(DelayedRuntime {
                ty: RuntimeType::Process,
                delay_ms: 200,
                payload: b"ok".to_vec(),
            }),
        )
        .unwrap();
        let rm = Arc::new(rm);

        let cfg = TaskExecutionManagerConfig {
            max_concurrent_executions: 1,
            ..Default::default()
        };
        let manager = TaskExecutionManager::new(
            cfg,
            rm,
            Arc::new(crate::spearlet::config::SpearletConfig::default()),
        )
        .await
        .unwrap();

        let proto = crate::proto::spearlet::ArtifactSpec {
            artifact_id: "artifact-long".to_string(),
            artifact_type: "process".to_string(),
            location: "".to_string(),
            version: "1.0.0".to_string(),
            checksum: "".to_string(),
            metadata: StdHashMap::new(),
        };
        let spec_local = crate::spearlet::execution::artifact::ArtifactSpec::from(proto.clone());
        let artifact = manager
            .ensure_artifact_with_id("artifact-long".to_string(), spec_local)
            .unwrap();

        use crate::spearlet::execution::task::{
            ExecutionKind, HealthCheckConfig, ScalingConfig, TaskSpec, TaskType, TimeoutConfig,
        };
        let task_spec = TaskSpec {
            name: "task-long".to_string(),
            task_type: TaskType::HttpHandler,
            runtime_type: artifact.spec.runtime_type,
            entry_point: "main".to_string(),
            handler_config: StdHashMap::new(),
            environment: artifact.spec.environment.clone(),
            invocation_type: artifact.spec.invocation_type.clone(),
            min_instances: 1,
            max_instances: 10,
            target_concurrency: 100,
            scaling_config: ScalingConfig::default(),
            health_check: HealthCheckConfig::default(),
            timeout_config: TimeoutConfig::default(),
            execution_kind: ExecutionKind::LongRunning,
        };
        manager
            .ensure_task_with_id("task-long".to_string(), &artifact, task_spec)
            .unwrap();

        let req = InvokeFunctionRequest {
            invocation_type: crate::proto::spearlet::InvocationType::ExistingTask as i32,
            task_id: "task-long".to_string(),
            artifact_spec: Some(proto),
            execution_mode: crate::proto::spearlet::ExecutionMode::Async as i32,
            wait: false,
            execution_id: Some("exec-long-1".to_string()),
            ..Default::default()
        };

        let mgr2 = manager.clone();
        let h = tokio::spawn(async move { mgr2.submit_execution(req).await });

        tokio::time::timeout(Duration::from_secs(1), async {
            loop {
                if let Ok(Some(s)) = manager.get_execution_status("exec-long-1").await {
                    if s.status == "pending" || s.status == "running" {
                        break;
                    }
                }
                sleep(Duration::from_millis(10)).await;
            }
        })
        .await
        .unwrap();

        let final_resp = h.await.unwrap().unwrap();
        assert_eq!(final_resp.execution_id, "exec-long-1");
        assert_eq!(final_resp.status, "completed");
        assert_eq!(final_resp.output_data, b"ok".to_vec());

        let stored = manager
            .get_execution_status("exec-long-1")
            .await
            .unwrap()
            .unwrap();
        assert_eq!(stored.status, "completed");
    }

    #[tokio::test]
    async fn test_stop_instance_removes_from_task_and_manager() {
        let mut rm = RuntimeManager::new();
        rm.register_runtime(
            RuntimeType::Process,
            Box::new(DummyRuntime {
                ty: RuntimeType::Process,
            }),
        )
        .unwrap();
        let rm = Arc::new(rm);

        let config = TaskExecutionManagerConfig::default();
        let manager = TaskExecutionManager::new(
            config,
            rm,
            Arc::new(crate::spearlet::config::SpearletConfig::default()),
        )
        .await
        .unwrap();

        let proto = crate::proto::spearlet::ArtifactSpec {
            artifact_id: "artifact-test".to_string(),
            artifact_type: "process".to_string(),
            location: "".to_string(),
            version: "1.0.0".to_string(),
            checksum: "".to_string(),
            metadata: StdHashMap::new(),
        };

        let spec_local = crate::spearlet::execution::artifact::ArtifactSpec::from(proto);
        let artifact = manager
            .ensure_artifact_with_id("artifact-test".to_string(), spec_local)
            .unwrap();
        use crate::spearlet::execution::task::{
            ExecutionKind, HealthCheckConfig, ScalingConfig, TaskSpec, TaskType, TimeoutConfig,
        };
        let task_spec = TaskSpec {
            name: "task-test".to_string(),
            task_type: TaskType::HttpHandler,
            runtime_type: artifact.spec.runtime_type,
            entry_point: "main".to_string(),
            handler_config: StdHashMap::new(),
            environment: artifact.spec.environment.clone(),
            invocation_type: artifact.spec.invocation_type.clone(),
            min_instances: 1,
            max_instances: 10,
            target_concurrency: 100,
            scaling_config: ScalingConfig::default(),
            health_check: HealthCheckConfig::default(),
            timeout_config: TimeoutConfig::default(),
            execution_kind: ExecutionKind::ShortRunning,
        };
        let task = manager
            .ensure_task_with_id("task-test".to_string(), &artifact, task_spec)
            .unwrap();
        let instance = manager.get_or_create_instance(&task).await.unwrap();

        assert_eq!(task.instance_count(), 1);
        assert!(manager.get_instance(&instance.id().to_string()).is_some());

        manager.stop_instance(&instance).await.unwrap();

        assert_eq!(task.instance_count(), 0);
        assert!(task.get_instance(instance.id()).is_none());
        assert!(manager.get_instance(&instance.id().to_string()).is_none());
    }

    #[tokio::test]
    async fn test_cleanup_loop_removes_task_from_artifact() {
        let mut rm = RuntimeManager::new();
        rm.register_runtime(
            RuntimeType::Process,
            Box::new(DummyRuntime {
                ty: RuntimeType::Process,
            }),
        )
        .unwrap();
        let rm = Arc::new(rm);

        let cfg = TaskExecutionManagerConfig {
            cleanup_interval_ms: 10,
            task_idle_timeout_ms: 1,
            ..Default::default()
        };

        let manager = TaskExecutionManager::new(
            cfg,
            rm,
            Arc::new(crate::spearlet::config::SpearletConfig::default()),
        )
        .await
        .unwrap();

        let proto = crate::proto::spearlet::ArtifactSpec {
            artifact_id: "artifact-cleanup".to_string(),
            artifact_type: "process".to_string(),
            location: "".to_string(),
            version: "1.0.0".to_string(),
            checksum: "".to_string(),
            metadata: StdHashMap::new(),
        };

        let spec_local = crate::spearlet::execution::artifact::ArtifactSpec::from(proto);
        let artifact = manager
            .ensure_artifact_with_id("artifact-cleanup".to_string(), spec_local)
            .unwrap();
        use crate::spearlet::execution::task::{
            ExecutionKind, HealthCheckConfig, ScalingConfig, TaskSpec, TaskType, TimeoutConfig,
        };
        let task_spec = TaskSpec {
            name: "task-cleanup".to_string(),
            task_type: TaskType::HttpHandler,
            runtime_type: artifact.spec.runtime_type,
            entry_point: "main".to_string(),
            handler_config: StdHashMap::new(),
            environment: artifact.spec.environment.clone(),
            invocation_type: artifact.spec.invocation_type.clone(),
            min_instances: 1,
            max_instances: 10,
            target_concurrency: 100,
            scaling_config: ScalingConfig::default(),
            health_check: HealthCheckConfig::default(),
            timeout_config: TimeoutConfig::default(),
            execution_kind: ExecutionKind::ShortRunning,
        };
        let task = manager
            .ensure_task_with_id("task-cleanup".to_string(), &artifact, task_spec)
            .unwrap();
        let task_id = task.id().to_string();

        let handle = {
            let m = manager.clone();
            tokio::spawn(async move { m.run_cleanup_loop().await })
        };

        tokio::time::sleep(std::time::Duration::from_millis(50)).await;
        handle.abort();

        assert!(manager.get_task(&task_id).is_none());
        assert!(artifact.get_task(&task_id).is_none());
    }

    #[tokio::test]
    async fn test_get_or_create_task_uses_desired_task_id() {
        let mut rm = RuntimeManager::new();
        rm.register_runtime(
            RuntimeType::Process,
            Box::new(DummyRuntime {
                ty: RuntimeType::Process,
            }),
        )
        .unwrap();
        let rm = Arc::new(rm);

        let manager = TaskExecutionManager::new(
            TaskExecutionManagerConfig::default(),
            rm,
            Arc::new(crate::spearlet::config::SpearletConfig::default()),
        )
        .await
        .unwrap();

        let proto = crate::proto::spearlet::ArtifactSpec {
            artifact_id: "artifact-fixed".to_string(),
            artifact_type: "process".to_string(),
            location: "file:///bin/foo".to_string(),
            version: "1.0.0".to_string(),
            checksum: "".to_string(),
            metadata: StdHashMap::new(),
        };

        let spec_local = crate::spearlet::execution::artifact::ArtifactSpec::from(proto);
        let artifact = manager
            .ensure_artifact_with_id("artifact-fixed".to_string(), spec_local)
            .unwrap();
        let desired = "sms-task-123".to_string();
        use crate::spearlet::execution::task::{
            ExecutionKind, HealthCheckConfig, ScalingConfig, TaskSpec, TaskType, TimeoutConfig,
        };
        let task_spec = TaskSpec {
            name: desired.clone(),
            task_type: TaskType::HttpHandler,
            runtime_type: artifact.spec.runtime_type,
            entry_point: "main".to_string(),
            handler_config: StdHashMap::new(),
            environment: artifact.spec.environment.clone(),
            invocation_type: artifact.spec.invocation_type.clone(),
            min_instances: 1,
            max_instances: 10,
            target_concurrency: 100,
            scaling_config: ScalingConfig::default(),
            health_check: HealthCheckConfig::default(),
            timeout_config: TimeoutConfig::default(),
            execution_kind: ExecutionKind::ShortRunning,
        };
        let task = manager
            .ensure_task_with_id(desired.clone(), &artifact, task_spec)
            .unwrap();
        assert_eq!(task.id(), desired);
        assert!(manager.get_task(&desired).is_some());
    }

    #[tokio::test]
    async fn test_health_check_failure_triggers_cascade_removal() {
        use std::sync::atomic::{AtomicBool, Ordering};

        struct FailingRuntime {
            ty: RuntimeType,
            fail: Arc<AtomicBool>,
        }

        #[async_trait]
        impl Runtime for FailingRuntime {
            fn runtime_type(&self) -> RuntimeType {
                self.ty
            }
            async fn create_instance(
                &self,
                config: &instance::InstanceConfig,
            ) -> super::ExecutionResult<Arc<instance::TaskInstance>> {
                Ok(Arc::new(instance::TaskInstance::new(
                    config.task_id.clone(),
                    config.clone(),
                )))
            }
            async fn start_instance(
                &self,
                _instance: &Arc<instance::TaskInstance>,
            ) -> super::ExecutionResult<()> {
                Ok(())
            }
            async fn stop_instance(
                &self,
                _instance: &Arc<instance::TaskInstance>,
            ) -> super::ExecutionResult<()> {
                Ok(())
            }
            async fn execute(
                &self,
                _instance: &Arc<instance::TaskInstance>,
                _context: runtime::ExecutionContext,
            ) -> super::ExecutionResult<runtime::RuntimeExecutionResponse> {
                Ok(runtime::RuntimeExecutionResponse::default())
            }
            async fn health_check(
                &self,
                _instance: &Arc<instance::TaskInstance>,
            ) -> super::ExecutionResult<bool> {
                if self.fail.load(Ordering::SeqCst) {
                    Err(super::ExecutionError::HealthCheckFailed {
                        message: "fail".to_string(),
                    })
                } else {
                    Ok(true)
                }
            }
            async fn get_metrics(
                &self,
                _instance: &Arc<instance::TaskInstance>,
            ) -> super::ExecutionResult<StdHashMap<String, serde_json::Value>> {
                Ok(StdHashMap::new())
            }
            async fn scale_instance(
                &self,
                _instance: &Arc<instance::TaskInstance>,
                _new_limits: &instance::InstanceResourceLimits,
            ) -> super::ExecutionResult<()> {
                Ok(())
            }
            async fn cleanup_instance(
                &self,
                _instance: &Arc<instance::TaskInstance>,
            ) -> super::ExecutionResult<()> {
                Ok(())
            }
            fn validate_config(
                &self,
                _config: &instance::InstanceConfig,
            ) -> super::ExecutionResult<()> {
                Ok(())
            }
            fn get_capabilities(&self) -> RuntimeCapabilities {
                RuntimeCapabilities::default()
            }
        }

        let mut rm = RuntimeManager::new();
        let fail_flag = Arc::new(AtomicBool::new(false));
        rm.register_runtime(
            RuntimeType::Process,
            Box::new(FailingRuntime {
                ty: RuntimeType::Process,
                fail: fail_flag.clone(),
            }),
        )
        .unwrap();
        let rm = Arc::new(rm);

        let cfg = TaskExecutionManagerConfig {
            health_check_interval_ms: 10,
            ..Default::default()
        };

        let manager = TaskExecutionManager::new(
            cfg,
            rm,
            Arc::new(crate::spearlet::config::SpearletConfig::default()),
        )
        .await
        .unwrap();

        let proto = crate::proto::spearlet::ArtifactSpec {
            artifact_id: "artifact-hc".to_string(),
            artifact_type: "process".to_string(),
            location: "".to_string(),
            version: "1.0.0".to_string(),
            checksum: "".to_string(),
            metadata: StdHashMap::new(),
        };

        let spec_local = crate::spearlet::execution::artifact::ArtifactSpec::from(proto);
        let artifact = manager
            .ensure_artifact_with_id("artifact-hc".to_string(), spec_local)
            .unwrap();
        use crate::spearlet::execution::task::{
            ExecutionKind, HealthCheckConfig, ScalingConfig, TaskSpec, TaskType, TimeoutConfig,
        };
        let task_spec = TaskSpec {
            name: "task-hc".to_string(),
            task_type: TaskType::HttpHandler,
            runtime_type: artifact.spec.runtime_type,
            entry_point: "main".to_string(),
            handler_config: StdHashMap::new(),
            environment: artifact.spec.environment.clone(),
            invocation_type: artifact.spec.invocation_type.clone(),
            min_instances: 1,
            max_instances: 10,
            target_concurrency: 100,
            scaling_config: ScalingConfig::default(),
            health_check: HealthCheckConfig::default(),
            timeout_config: TimeoutConfig::default(),
            execution_kind: ExecutionKind::ShortRunning,
        };
        let task = manager
            .ensure_task_with_id("task-hc".to_string(), &artifact, task_spec)
            .unwrap();
        let instance = manager.get_or_create_instance(&task).await.unwrap();

        fail_flag.store(true, Ordering::SeqCst);
        manager.process_health_checks_once().await;
        manager.process_health_checks_once().await;
        manager.process_health_checks_once().await;

        assert!(manager.get_instance(&instance.id().to_string()).is_none());
        assert!(task.get_instance(instance.id()).is_none());
    }
}
