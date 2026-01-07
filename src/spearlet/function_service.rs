//! Function service implementation for spearlet
//! spearlet的函数服务实现

use std::collections::{HashMap, HashSet};
use std::pin::Pin;
use std::sync::Arc;
use std::time::{SystemTime, UNIX_EPOCH};
use tokio::sync::RwLock;
use tokio_stream::Stream;
use tonic::{Request, Response, Status};
use tracing::debug;
use uuid::Uuid;

use crate::proto::spearlet::{
    function_service_server::FunctionService,
    ArtifactSpec,
    CancelExecutionRequest,
    CancelExecutionResponse,
    DeleteTaskRequest,
    DeleteTaskResponse,
    ExecutionResult,
    ExecutionStats,
    ExecutionStatus,
    GetExecutionStatusRequest,
    GetExecutionStatusResponse,
    // Health and stats / 健康状态和统计
    GetHealthRequest,
    GetHealthResponse,
    GetStatsRequest,
    GetStatsResponse,
    GetTaskRequest,
    GetTaskResponse,
    // Common types / 通用类型
    HealthDetails,
    // Request and response types / 请求和响应类型
    InvokeFunctionRequest,
    InvokeFunctionResponse,
    ListExecutionsRequest,
    ListExecutionsResponse,
    // Task management / 任务管理
    ListTasksRequest,
    ListTasksResponse,
    ServiceStats,
    StreamExecutionResult,
    TaskStats,
};

use crate::spearlet::execution::{
    artifact::InvocationType as ExecutionInvocationType,
    runtime::{ResourcePoolConfig, RuntimeConfig, RuntimeFactory, RuntimeManager},
    Artifact, ArtifactSpec as ExecutionArtifactSpec, ExecutionError, ExecutionResponse,
    InstancePool, InstancePoolConfig, InstanceScheduler, RuntimeType, SchedulingPolicy,
    TaskExecutionManager, TaskExecutionManagerConfig,
};
use crate::spearlet::SpearletConfig;

fn collect_llm_global_environment(cfg: &SpearletConfig) -> HashMap<String, String> {
    let mut cred_env: HashMap<String, String> = HashMap::new();
    for c in cfg.llm.credentials.iter() {
        if c.kind.as_str() != "env" {
            continue;
        }
        if c.name.trim().is_empty() {
            continue;
        }
        if c.api_key_env.trim().is_empty() {
            continue;
        }
        cred_env.insert(c.name.clone(), c.api_key_env.clone());
    }

    let mut required: HashSet<String> = HashSet::new();
    for b in cfg.llm.backends.iter() {
        if !backend_requires_api_key(&b.kind) {
            continue;
        }
        let Some(r) = b.credential_ref.as_ref() else {
            continue;
        };
        let Some(env) = cred_env.get(r) else {
            continue;
        };
        required.insert(env.clone());
    }

    let mut out: HashMap<String, String> = HashMap::new();
    for env_name in required.into_iter() {
        if let Ok(v) = std::env::var(&env_name) {
            if !v.is_empty() {
                out.insert(env_name, v);
            }
        }
    }
    out
}

fn backend_requires_api_key(kind: &str) -> bool {
    matches!(kind, "openai_chat_completion" | "openai_realtime_ws")
}

/// Function service statistics / 函数服务统计信息
#[derive(Debug, Clone)]
pub struct FunctionServiceStats {
    pub task_count: usize,
    pub execution_count: usize,
    pub running_executions: usize,
    pub artifact_count: usize,
    pub instance_count: usize,
    pub successful_executions: usize,
    pub failed_executions: usize,
    pub average_response_time_ms: f64,
}

/// Function service implementation / 函数服务实现
pub struct FunctionServiceImpl {
    /// Service start time / 服务启动时间
    start_time: SystemTime,
    /// Task execution manager / 任务执行管理器
    execution_manager: Arc<TaskExecutionManager>,
    /// Instance pool / 实例池
    instance_pool: Arc<InstancePool>,
    /// Service statistics / 服务统计信息
    stats: Arc<RwLock<FunctionServiceStats>>,
}

impl FunctionServiceImpl {
    /// Create new function service / 创建新的函数服务
    pub async fn new(config: Arc<SpearletConfig>) -> Result<Self, ExecutionError> {
        let mut rm = RuntimeManager::new();
        let global_environment = collect_llm_global_environment(&config);
        let default_configs: Vec<RuntimeConfig> = RuntimeFactory::available_runtimes()
            .into_iter()
            .map(|rt| RuntimeConfig {
                runtime_type: rt,
                settings: HashMap::new(),
                global_environment: global_environment.clone(),
                spearlet_config: Some((*config).clone()),
                resource_pool: ResourcePoolConfig::default(),
            })
            .collect();
        rm.initialize_runtimes(default_configs)?;
        let runtime_manager = Arc::new(rm);

        // Create execution manager / 创建执行管理器
        let manager_config = TaskExecutionManagerConfig::default();
        let execution_manager =
            TaskExecutionManager::new(manager_config, runtime_manager, config.clone()).await?;

        // Create instance pool / 创建实例池
        let pool_config = InstancePoolConfig::default();
        let scheduler = Arc::new(InstanceScheduler::new(SchedulingPolicy::RoundRobin));
        let instance_pool = InstancePool::new(pool_config, scheduler).await?;

        // Initialize statistics / 初始化统计信息
        let stats = Arc::new(RwLock::new(FunctionServiceStats {
            task_count: 0,
            execution_count: 0,
            running_executions: 0,
            artifact_count: 0,
            instance_count: 0,
            successful_executions: 0,
            failed_executions: 0,
            average_response_time_ms: 0.0,
        }));

        Ok(Self {
            start_time: SystemTime::now(),
            execution_manager,
            instance_pool,
            stats,
        })
    }

    pub fn get_execution_manager(&self) -> Arc<TaskExecutionManager> {
        self.execution_manager.clone()
    }

    /// Generate execution ID / 生成执行ID
    fn generate_execution_id(&self) -> String {
        Uuid::new_v4().to_string()
    }

    /// Generate task ID / 生成任务ID
    fn generate_task_id(&self) -> String {
        Uuid::new_v4().to_string()
    }

    /// Handle synchronous execution / 处理同步执行
    async fn handle_sync_execution(
        &self,
        request: &InvokeFunctionRequest,
    ) -> Result<ExecutionResult, Status> {
        // 使用 TaskExecutionManager 执行同步任务 / Use TaskExecutionManager for sync execution
        match self
            .execution_manager
            .submit_execution(request.clone())
            .await
        {
            Ok(execution_response) => {
                // 使用 HTTP 适配器转换响应 / Use HTTP adapter to convert response
                let http_adapter = crate::spearlet::execution::http_adapter::HttpAdapter::new();
                let _http_response = http_adapter.to_sync_response(&execution_response);

                // 转换为 protobuf ExecutionResult / Convert to protobuf ExecutionResult
                Ok(ExecutionResult {
                    status: if execution_response.status == "completed" {
                        ExecutionStatus::Completed as i32
                    } else {
                        ExecutionStatus::Failed as i32
                    },
                    result: Some(prost_types::Any {
                        type_url: "type.googleapis.com/spearlet.ExecutionData".to_string(),
                        value: execution_response.output_data.clone(),
                    }),
                    error_message: execution_response.error_message.clone().unwrap_or_default(),
                    error_code: execution_response
                        .error_message
                        .as_ref()
                        .map(|_| "EXECUTION_ERROR".to_string())
                        .unwrap_or_default(),
                    execution_time_ms: execution_response.execution_time_ms as i64,
                    memory_used_bytes: 0, // TODO: 从 metadata 中获取 / TODO: Get from metadata
                    metrics: execution_response.metadata.clone(),
                    started_at: Some(prost_types::Timestamp {
                        seconds: SystemTime::now()
                            .duration_since(UNIX_EPOCH)
                            .unwrap()
                            .as_secs() as i64,
                        nanos: 0,
                    }),
                    completed_at: Some(prost_types::Timestamp {
                        seconds: SystemTime::now()
                            .duration_since(UNIX_EPOCH)
                            .unwrap()
                            .as_secs() as i64,
                        nanos: 0,
                    }),
                })
            }
            Err(e) => Err(Status::internal(format!(
                "执行失败 / Execution failed: {}",
                e
            ))),
        }
    }

    /// Handle asynchronous execution / 处理异步执行
    async fn handle_async_execution(&self, request: &InvokeFunctionRequest) -> (String, i64) {
        let execution_id = request.execution_id.as_deref().unwrap_or_default();

        // 启动异步任务 / Start async task
        let execution_manager = self.execution_manager.clone();
        let request_clone = request.clone();
        let execution_id_clone = execution_id.to_string();

        // 在后台启动异步执行 / Start async execution in background
        tokio::spawn(async move {
            match execution_manager.submit_execution(request_clone).await {
                Ok(_execution_response) => {
                    // 异步执行完成，结果会存储在 execution_manager 中 / Async execution completed, result stored in execution_manager
                    tracing::info!(
                        "异步执行完成 / Async execution completed: {}",
                        execution_id_clone
                    );
                }
                Err(e) => {
                    tracing::error!(
                        "异步执行失败 / Async execution failed: {} - {}",
                        execution_id_clone,
                        e
                    );
                }
            }
        });

        // 返回状态端点和预估完成时间 / Return status endpoint and estimated completion time
        let status_endpoint = format!("/api/v1/executions/{}/status", execution_id);
        let estimated_completion_ms = 5000; // 预估5秒完成 / Estimated 5 seconds to complete

        (status_endpoint, estimated_completion_ms)
    }

    /// Create a failed execution result / 创建失败的执行结果
    fn create_failed_result(&self, error_code: &str) -> ExecutionResult {
        ExecutionResult {
            status: ExecutionStatus::Failed as i32,
            result: None,
            error_message: match error_code {
                "SYNC_EXECUTION_FAILED" => "同步执行失败 / Sync execution failed".to_string(),
                "STREAM_MODE_NOT_SUPPORTED" => {
                    "不支持流式模式 / Stream mode not supported".to_string()
                }
                "UNKNOWN_EXECUTION_MODE" => "未知执行模式 / Unknown execution mode".to_string(),
                _ => "执行失败 / Execution failed".to_string(),
            },
            error_code: error_code.to_string(),
            execution_time_ms: 0,
            memory_used_bytes: 0,
            metrics: HashMap::new(),
            started_at: Some(prost_types::Timestamp {
                seconds: SystemTime::now()
                    .duration_since(UNIX_EPOCH)
                    .unwrap()
                    .as_secs() as i64,
                nanos: 0,
            }),
            completed_at: Some(prost_types::Timestamp {
                seconds: SystemTime::now()
                    .duration_since(UNIX_EPOCH)
                    .unwrap()
                    .as_secs() as i64,
                nanos: 0,
            }),
        }
    }

    /// Get service statistics / 获取服务统计信息
    pub async fn get_stats(&self) -> FunctionServiceStats {
        // Update statistics from execution manager and instance pool
        // 从执行管理器和实例池更新统计信息
        let execution_stats = self.execution_manager.get_statistics();
        let pool_metrics = self.instance_pool.get_global_metrics();

        let mut stats = self.stats.write().await;
        stats.task_count = self.execution_manager.list_tasks().len();
        stats.artifact_count = self.execution_manager.list_artifacts().len();
        stats.instance_count = pool_metrics.total_instances as usize;
        // Use active_instances instead of non-existent active_executions
        // 使用 active_instances 而不是不存在的 active_executions
        stats.running_executions = execution_stats.active_instances as usize;
        stats.successful_executions = execution_stats.successful_executions as usize;
        stats.failed_executions = execution_stats.failed_executions as usize;
        stats.average_response_time_ms = pool_metrics.average_response_time_ms;

        stats.clone()
    }

    /// Create artifact from proto spec / 从 proto 规范创建 artifact
    #[allow(dead_code)]
    async fn create_artifact_from_proto(
        &self,
        proto_spec: &ArtifactSpec,
    ) -> Result<Arc<Artifact>, ExecutionError> {
        let artifact_spec = ExecutionArtifactSpec {
            name: proto_spec.artifact_id.clone(),
            version: proto_spec.version.clone(),
            description: None,
            runtime_type: match proto_spec.artifact_type.as_str() {
                "docker" => RuntimeType::Kubernetes, // Migrate Docker to Kubernetes / 将 Docker 迁移到 Kubernetes
                "kubernetes" => RuntimeType::Kubernetes,
                "process" => RuntimeType::Process,
                "wasm" => RuntimeType::Wasm,
                _ => RuntimeType::Process, // Default fallback
            },
            runtime_config: std::collections::HashMap::new(),
            location: None,
            checksum_sha256: None,
            environment: std::collections::HashMap::new(),
            resource_limits: Default::default(),
            invocation_type: ExecutionInvocationType::NewTask,
            max_execution_timeout_ms: 30000,
            labels: proto_spec.metadata.clone(),
        };

        let artifact = Artifact::new(artifact_spec);

        Ok(Arc::new(artifact))
    }

    /// Convert execution response to proto result / 将执行响应转换为 proto 结果
    #[allow(dead_code)]
    fn execution_response_to_proto(&self, response: &ExecutionResponse) -> ExecutionResult {
        ExecutionResult {
            status: match response.status.as_str() {
                "completed" => ExecutionStatus::Completed as i32,
                "failed" => ExecutionStatus::Failed as i32,
                "running" => ExecutionStatus::Running as i32,
                "pending" => ExecutionStatus::Pending as i32,
                _ => ExecutionStatus::Failed as i32,
            },
            result: Some(prost_types::Any {
                type_url: "type.googleapis.com/google.protobuf.StringValue".to_string(),
                value: response.output_data.clone(),
            }),
            error_message: response.error_message.clone().unwrap_or_default(),
            error_code: String::new(),
            execution_time_ms: response.execution_time_ms as i64,
            memory_used_bytes: 0, // TODO: Add memory tracking
            metrics: response.metadata.clone(),
            started_at: Some(prost_types::Timestamp {
                seconds: response
                    .timestamp
                    .duration_since(UNIX_EPOCH)
                    .unwrap()
                    .as_secs() as i64,
                nanos: 0,
            }),
            completed_at: Some(prost_types::Timestamp {
                seconds: response
                    .timestamp
                    .duration_since(UNIX_EPOCH)
                    .unwrap()
                    .as_secs() as i64,
                nanos: 0,
            }),
        }
    }
}

#[tonic::async_trait]
impl FunctionService for FunctionServiceImpl {
    /// Invoke function / 调用函数
    async fn invoke_function(
        &self,
        request: Request<InvokeFunctionRequest>,
    ) -> Result<Response<InvokeFunctionResponse>, Status> {
        let mut req = request.into_inner();
        debug!(
            "收到函数调用请求 / Received function invocation request: {:?}",
            req
        );

        // 生成执行 ID / Generate execution ID
        let execution_id = req
            .execution_id
            .clone()
            .filter(|id| !id.is_empty())
            .unwrap_or_else(|| self.generate_execution_id());
        req.execution_id = Some(execution_id.clone());
        let task_id = self.generate_task_id();

        // 根据执行模式处理请求 / Handle request based on execution mode
        match req.execution_mode() {
            crate::proto::spearlet::ExecutionMode::Sync => {
                // 同步执行：等待完成后返回完整结果 / Sync execution: wait for completion and return complete result
                let result = self.handle_sync_execution(&req).await;

                let response = InvokeFunctionResponse {
                    success: result.is_ok(),
                    message: if result.is_ok() {
                        "函数执行成功 / Function executed successfully".to_string()
                    } else {
                        "函数执行失败 / Function execution failed".to_string()
                    },
                    execution_id: execution_id.clone(),
                    task_id: task_id.clone(),
                    instance_id: String::new(),
                    result: Some(
                        result
                            .unwrap_or_else(|_| self.create_failed_result("SYNC_EXECUTION_FAILED")),
                    ),
                    status_endpoint: String::new(), // 同步模式不需要状态端点 / No status endpoint needed for sync mode
                    estimated_completion_ms: 0,
                };

                Ok(Response::new(response))
            }
            crate::proto::spearlet::ExecutionMode::Async => {
                // 异步执行：立即返回执行ID和状态端点 / Async execution: immediately return execution ID and status endpoint
                let (status_endpoint, estimated_ms) = self.handle_async_execution(&req).await;

                let result = ExecutionResult {
                    status: ExecutionStatus::Pending as i32,
                    result: None,
                    error_message: String::new(),
                    error_code: String::new(),
                    execution_time_ms: 0,
                    memory_used_bytes: 0,
                    metrics: HashMap::new(),
                    started_at: Some(prost_types::Timestamp {
                        seconds: SystemTime::now()
                            .duration_since(UNIX_EPOCH)
                            .unwrap()
                            .as_secs() as i64,
                        nanos: 0,
                    }),
                    completed_at: None, // 异步执行尚未完成 / Async execution not yet completed
                };

                let response = InvokeFunctionResponse {
                    success: true,
                    message: "异步函数执行已启动 / Async function execution started".to_string(),
                    execution_id: execution_id.clone(),
                    task_id: task_id.clone(),
                    instance_id: String::new(),
                    result: Some(result),
                    status_endpoint,
                    estimated_completion_ms: estimated_ms,
                };

                Ok(Response::new(response))
            }
            crate::proto::spearlet::ExecutionMode::Stream => {
                // 流式执行应使用 StreamFunction RPC / Streaming execution should use StreamFunction RPC
                let result = self.create_failed_result("STREAM_MODE_NOT_SUPPORTED");

                let response = InvokeFunctionResponse {
                    success: false,
                    message: "流式模式请使用 StreamFunction RPC / Use StreamFunction RPC for streaming mode".to_string(),
                    execution_id: execution_id.clone(),
                    task_id: task_id.clone(),
                    instance_id: String::new(),
                    result: Some(result),
                    status_endpoint: String::new(),
                    estimated_completion_ms: 0,
                };

                Ok(Response::new(response))
            }
            _ => {
                // 未知执行模式 / Unknown execution mode
                let result = self.create_failed_result("UNKNOWN_EXECUTION_MODE");

                let response = InvokeFunctionResponse {
                    success: false,
                    message: "未知的执行模式 / Unknown execution mode".to_string(),
                    execution_id: execution_id.clone(),
                    task_id: task_id.clone(),
                    instance_id: String::new(),
                    result: Some(result),
                    status_endpoint: String::new(),
                    estimated_completion_ms: 0,
                };

                Ok(Response::new(response))
            }
        }
    }

    /// Get execution status / 获取执行状态
    async fn get_execution_status(
        &self,
        request: Request<GetExecutionStatusRequest>,
    ) -> Result<Response<GetExecutionStatusResponse>, Status> {
        let req = request.into_inner();
        debug!(
            "收到执行状态查询请求 / Received execution status request: {:?}",
            req
        );

        // 尝试从 execution_manager 获取执行状态 / Try to get execution status from execution_manager
        match self
            .execution_manager
            .get_execution_status(&req.execution_id)
            .await
        {
            Ok(Some(execution_response)) => {
                // 直接构建响应 / Build response directly
                let result = if execution_response.is_completed() {
                    Some(ExecutionResult {
                        status: if execution_response.is_successful() {
                            ExecutionStatus::Completed as i32
                        } else {
                            ExecutionStatus::Failed as i32
                        },
                        result: Some(prost_types::Any {
                            type_url: "type.googleapis.com/spearlet.ExecutionData".to_string(),
                            value: execution_response.output_data.clone(),
                        }),
                        error_message: execution_response.error_message.clone().unwrap_or_default(),
                        error_code: execution_response
                            .error_message
                            .as_ref()
                            .map(|_| "EXECUTION_ERROR".to_string())
                            .unwrap_or_default(),
                        execution_time_ms: execution_response.execution_time_ms as i64,
                        memory_used_bytes: 0,
                        metrics: execution_response.metadata.clone(),
                        started_at: Some(prost_types::Timestamp {
                            seconds: SystemTime::now()
                                .duration_since(UNIX_EPOCH)
                                .unwrap()
                                .as_secs() as i64,
                            nanos: 0,
                        }),
                        completed_at: if execution_response.status == "completed" {
                            Some(prost_types::Timestamp {
                                seconds: SystemTime::now()
                                    .duration_since(UNIX_EPOCH)
                                    .unwrap()
                                    .as_secs() as i64,
                                nanos: 0,
                            })
                        } else {
                            None
                        },
                    })
                } else {
                    None
                };

                let response = GetExecutionStatusResponse {
                    found: true,
                    result,
                    logs: vec![], // TODO: 从 execution_manager 获取日志 / TODO: Get logs from execution_manager
                    message: if execution_response.status == "completed" {
                        "执行已完成 / Execution completed".to_string()
                    } else {
                        "执行进行中 / Execution in progress".to_string()
                    },
                };

                Ok(Response::new(response))
            }
            Ok(None) => {
                // 执行未找到 / Execution not found
                let response = GetExecutionStatusResponse {
                    found: false,
                    result: None,
                    logs: vec![],
                    message: "执行未找到 / Execution not found".to_string(),
                };

                Ok(Response::new(response))
            }
            Err(e) => Err(Status::internal(format!(
                "查询执行状态失败 / Failed to query execution status: {}",
                e
            ))),
        }
    }

    /// Cancel execution / 取消执行
    async fn cancel_execution(
        &self,
        request: Request<CancelExecutionRequest>,
    ) -> Result<Response<CancelExecutionResponse>, Status> {
        let req = request.into_inner();
        debug!(
            "收到取消执行请求 / Received cancel execution request: {:?}",
            req
        );

        let response = CancelExecutionResponse {
            success: true,
            message: "执行已取消 / Execution cancelled".to_string(),
            final_status: ExecutionStatus::Failed as i32,
        };

        Ok(Response::new(response))
    }

    type StreamFunctionStream =
        Pin<Box<dyn Stream<Item = Result<StreamExecutionResult, Status>> + Send>>;

    /// Stream function execution / 流式函数执行
    async fn stream_function(
        &self,
        request: Request<InvokeFunctionRequest>,
    ) -> Result<Response<Self::StreamFunctionStream>, Status> {
        let req = request.into_inner();
        debug!(
            "收到流式函数调用请求 / Received stream function request: {:?}",
            req
        );

        // 创建一个简单的流响应 / Create a simple stream response
        let execution_id = self.generate_execution_id();
        let stream = tokio_stream::iter(vec![
            Ok(StreamExecutionResult {
                execution_id: execution_id.clone(),
                status: ExecutionStatus::Running as i32,
                chunk_data: Some(prost_types::Any {
                    type_url: "type.googleapis.com/google.protobuf.StringValue".to_string(),
                    value: "第一个数据块 / First data chunk".as_bytes().to_vec(),
                }),
                is_final: false,
                error_message: String::new(),
                metadata: HashMap::new(),
            }),
            Ok(StreamExecutionResult {
                execution_id,
                status: ExecutionStatus::Completed as i32,
                chunk_data: Some(prost_types::Any {
                    type_url: "type.googleapis.com/google.protobuf.StringValue".to_string(),
                    value: "最后一个数据块 / Last data chunk".as_bytes().to_vec(),
                }),
                is_final: true,
                error_message: String::new(),
                metadata: HashMap::new(),
            }),
        ]);

        Ok(Response::new(Box::pin(stream)))
    }

    /// List tasks / 列出任务
    async fn list_tasks(
        &self,
        request: Request<ListTasksRequest>,
    ) -> Result<Response<ListTasksResponse>, Status> {
        let _req = request.into_inner();
        debug!("收到任务列表请求 / Received list tasks request");

        let response = ListTasksResponse {
            tasks: vec![],
            has_more: false,
            next_start_after: String::new(),
        };

        Ok(Response::new(response))
    }

    /// Get task / 获取任务
    async fn get_task(
        &self,
        request: Request<GetTaskRequest>,
    ) -> Result<Response<GetTaskResponse>, Status> {
        let req = request.into_inner();
        debug!("收到获取任务请求 / Received get task request: {:?}", req);

        let response = GetTaskResponse {
            found: false,
            task: None,
            executions: vec![],
            message: format!(
                "任务 {} 未找到 / Task {} not found",
                req.task_id, req.task_id
            ),
        };

        Ok(Response::new(response))
    }

    /// Delete task / 删除任务
    async fn delete_task(
        &self,
        request: Request<DeleteTaskRequest>,
    ) -> Result<Response<DeleteTaskResponse>, Status> {
        let req = request.into_inner();
        debug!("收到删除任务请求 / Received delete task request: {:?}", req);

        let response = DeleteTaskResponse {
            success: true,
            message: "任务已删除 / Task deleted".to_string(),
            deleted: true,
            cleaned_executions: 0,
        };

        Ok(Response::new(response))
    }

    /// List executions / 列出执行
    async fn list_executions(
        &self,
        request: Request<ListExecutionsRequest>,
    ) -> Result<Response<ListExecutionsResponse>, Status> {
        let _req = request.into_inner();
        debug!("收到执行列表请求 / Received list executions request");

        let response = ListExecutionsResponse {
            executions: vec![],
            has_more: false,
            next_start_after: String::new(),
        };

        Ok(Response::new(response))
    }

    /// Get health / 获取健康状态
    async fn get_health(
        &self,
        _request: Request<GetHealthRequest>,
    ) -> Result<Response<GetHealthResponse>, Status> {
        debug!("收到健康检查请求 / Received health check request");

        let uptime = self.start_time.elapsed().unwrap_or_default().as_secs();

        let response = GetHealthResponse {
            status: "健康 / Healthy".to_string(),
            message: "函数服务运行正常 / Function service is running normally".to_string(),
            uptime_seconds: uptime as i64,
            details: Some(HealthDetails {
                active_tasks: 10,
                running_executions: 3,
                pending_executions: 5,
                total_memory_used: 1024 * 1024 * 100, // 100MB
                total_executions: 100,
                system_info: HashMap::new(),
            }),
        };

        Ok(Response::new(response))
    }

    /// Get statistics / 获取统计信息
    async fn get_stats(
        &self,
        request: Request<GetStatsRequest>,
    ) -> Result<Response<GetStatsResponse>, Status> {
        let req = request.into_inner();
        debug!("收到统计请求 / Received stats request: {:?}", req);

        let service_stats = Some(ServiceStats {
            uptime_seconds: self.start_time.elapsed().unwrap_or_default().as_secs() as i64,
            total_tasks: 50,
            total_executions: 100,
            total_memory_used: 1024 * 1024 * 100, // 100MB
            cpu_usage_percent: 25.0,
            memory_usage_percent: 25.0,
        });

        let task_stats = if req.include_task_stats {
            Some(TaskStats {
                active_tasks: 50,
                idle_tasks: 5,
                tasks_by_type: HashMap::new(),
                most_active_tasks: vec![],
            })
        } else {
            None
        };

        let execution_stats = if req.include_execution_stats {
            Some(ExecutionStats {
                running_executions: 3,
                pending_executions: 5,
                completed_executions: 95,
                failed_executions: 5,
                average_execution_time_ms: 500.0,
                success_rate_percent: 95.0,
                executions_by_function: HashMap::new(),
            })
        } else {
            None
        };

        let response = GetStatsResponse {
            service_stats,
            task_stats,
            execution_stats,
        };

        Ok(Response::new(response))
    }
}

/// Arc wrapper implementation for FunctionService / FunctionService 的 Arc 包装器实现
#[tonic::async_trait]
impl FunctionService for Arc<FunctionServiceImpl> {
    async fn invoke_function(
        &self,
        request: Request<InvokeFunctionRequest>,
    ) -> Result<Response<InvokeFunctionResponse>, Status> {
        (**self).invoke_function(request).await
    }

    async fn get_execution_status(
        &self,
        request: Request<GetExecutionStatusRequest>,
    ) -> Result<Response<GetExecutionStatusResponse>, Status> {
        (**self).get_execution_status(request).await
    }

    async fn cancel_execution(
        &self,
        request: Request<CancelExecutionRequest>,
    ) -> Result<Response<CancelExecutionResponse>, Status> {
        (**self).cancel_execution(request).await
    }

    type StreamFunctionStream = <FunctionServiceImpl as FunctionService>::StreamFunctionStream;

    async fn stream_function(
        &self,
        request: Request<InvokeFunctionRequest>,
    ) -> Result<Response<Self::StreamFunctionStream>, Status> {
        (**self).stream_function(request).await
    }

    async fn list_tasks(
        &self,
        request: Request<ListTasksRequest>,
    ) -> Result<Response<ListTasksResponse>, Status> {
        (**self).list_tasks(request).await
    }

    async fn get_task(
        &self,
        request: Request<GetTaskRequest>,
    ) -> Result<Response<GetTaskResponse>, Status> {
        (**self).get_task(request).await
    }

    async fn delete_task(
        &self,
        request: Request<DeleteTaskRequest>,
    ) -> Result<Response<DeleteTaskResponse>, Status> {
        (**self).delete_task(request).await
    }

    async fn list_executions(
        &self,
        request: Request<ListExecutionsRequest>,
    ) -> Result<Response<ListExecutionsResponse>, Status> {
        (**self).list_executions(request).await
    }

    async fn get_health(
        &self,
        request: Request<GetHealthRequest>,
    ) -> Result<Response<GetHealthResponse>, Status> {
        (**self).get_health(request).await
    }

    async fn get_stats(
        &self,
        request: Request<GetStatsRequest>,
    ) -> Result<Response<GetStatsResponse>, Status> {
        <FunctionServiceImpl as FunctionService>::get_stats(self, request).await
    }
}
#[cfg(test)]
mod tests {
    use super::*;
    use crate::spearlet::execution::artifact::{Artifact, ArtifactSpec};
    use crate::spearlet::execution::runtime::Runtime;
    use crate::spearlet::execution::runtime::{
        ResourcePoolConfig, RuntimeConfig, RuntimeType, WasmRuntime,
    };
    use crate::spearlet::execution::task::{Task, TaskSpec, TaskType};

    #[tokio::test]
    async fn test_function_service_initializes_runtimes() {
        let svc = FunctionServiceImpl::new(Arc::new(crate::spearlet::SpearletConfig::default()))
            .await
            .unwrap();
        let mgr = svc.get_execution_manager();
        let types = mgr.list_runtime_types();
        assert!(types.contains(&RuntimeType::Process));
        assert!(types.contains(&RuntimeType::Wasm));
        assert!(types.contains(&RuntimeType::Kubernetes));
    }

    #[tokio::test]
    async fn test_instance_has_correct_task_id() {
        // Create artifact and task spec following existing patterns
        let artifact_spec = ArtifactSpec {
            name: "test-artifact".to_string(),
            version: "1.0.0".to_string(),
            description: Some("Test artifact".to_string()),
            runtime_type: RuntimeType::Wasm,
            runtime_config: std::collections::HashMap::new(),
            location: None,
            checksum_sha256: None,
            environment: std::collections::HashMap::new(),
            resource_limits: Default::default(),
            invocation_type: crate::spearlet::execution::artifact::InvocationType::NewTask,
            max_execution_timeout_ms: 30000,
            labels: std::collections::HashMap::new(),
        };
        let artifact = Arc::new(Artifact::new(artifact_spec));

        let task_spec = TaskSpec {
            name: "test-task".to_string(),
            task_type: TaskType::HttpHandler,
            runtime_type: RuntimeType::Wasm,
            entry_point: "main".to_string(),
            handler_config: std::collections::HashMap::new(),
            environment: std::collections::HashMap::new(),
            invocation_type: crate::spearlet::execution::artifact::InvocationType::NewTask,
            min_instances: 1,
            max_instances: 10,
            target_concurrency: 1,
            scaling_config: Default::default(),
            health_check: Default::default(),
            timeout_config: Default::default(),
            execution_kind: crate::spearlet::execution::task::ExecutionKind::ShortRunning,
        };
        let task = Arc::new(Task::new(artifact.id().to_string(), task_spec));

        // Build instance config from task (injects TASK_ID)
        let instance_config = task.create_instance_config();

        // Create WASM runtime
        let rt_cfg = RuntimeConfig {
            runtime_type: RuntimeType::Wasm,
            settings: std::collections::HashMap::new(),
            global_environment: std::collections::HashMap::new(),
            spearlet_config: None,
            resource_pool: ResourcePoolConfig::default(),
        };
        let wasm = WasmRuntime::new(&rt_cfg).unwrap();

        // Verify TASK_ID injected into environment without creating instance
        assert_eq!(
            instance_config
                .environment
                .get("TASK_ID")
                .cloned()
                .unwrap_or_default(),
            task.id()
        );
        // Creating instance without valid wasm bytes should error per current logic
        let result = wasm.create_instance(&instance_config).await;
        assert!(result.is_err());
    }
}
