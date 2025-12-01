//! HTTP gateway implementation for spearlet
//! spearlet的HTTP gateway实现

use std::collections::HashMap;
use std::net::SocketAddr;
use std::sync::Arc;
use axum::{
    extract::{Path, Query, State},
    http::StatusCode,
    response::{Json, Html, IntoResponse},
    routing::{get, post, put, delete},
    Router,
};
use serde::Deserialize;
use tonic::transport::Channel;
use tracing::{info, error, debug};
use base64::{Engine as _, engine::general_purpose};

use crate::spearlet::config::SpearletConfig;
use crate::spearlet::grpc_server::HealthService;
use crate::proto::spearlet::{
    object_service_client::ObjectServiceClient,
    function_service_client::FunctionServiceClient,
    PutObjectRequest, GetObjectRequest, ListObjectsRequest,
    AddObjectRefRequest, RemoveObjectRefRequest,
    PinObjectRequest, UnpinObjectRequest, DeleteObjectRequest,
};

/// HTTP gateway server / HTTP网关服务器
pub struct HttpGateway {
    /// Server configuration / 服务器配置
    config: Arc<SpearletConfig>,
    /// Health service / 健康检查服务
    health_service: Arc<HealthService>,
}

/// Application state / 应用状态
#[derive(Clone)]
struct AppState {
    object_client: ObjectServiceClient<Channel>,
    function_client: FunctionServiceClient<Channel>,
    health_service: Arc<HealthService>,
    config: Arc<SpearletConfig>,
}

impl HttpGateway {
    /// Create new HTTP gateway / 创建新的HTTP网关
    pub fn new(config: Arc<SpearletConfig>, health_service: Arc<HealthService>) -> Self {
        Self {
            config,
            health_service,
        }
    }

    /// Start HTTP gateway server / 启动HTTP网关服务器
    pub async fn start(self) -> Result<(), Box<dyn std::error::Error + Send + Sync>> {
        let (listener, app) = self.prepare().await?;
        axum::serve(listener, app).await?;
        Ok(())
    }

    /// Start HTTP gateway with shutdown signal / 使用关闭信号启动HTTP网关
    pub async fn start_with_shutdown<F>(self, shutdown: F) -> Result<(), Box<dyn std::error::Error + Send + Sync>>
    where
        F: std::future::Future<Output = ()> + Send + 'static,
    {
        let (listener, app) = self.prepare().await?;
        axum::serve(listener, app)
            .with_graceful_shutdown(shutdown)
            .await?;
        Ok(())
    }

    async fn prepare(self) -> Result<(tokio::net::TcpListener, Router), Box<dyn std::error::Error + Send + Sync>> {
        let addr: SocketAddr = self.config.http.server.addr;
        info!("Starting HTTP gateway on {}", addr);

        let grpc_endpoint = format!("http://{}", self.config.grpc.addr);
        info!("Connecting to gRPC server at {}", grpc_endpoint);

        let mut grpc_client = None;
        let max_retries = 5;
        let mut retry_count = 0;
        while retry_count < max_retries {
            match ObjectServiceClient::connect(grpc_endpoint.clone()).await {
                Ok(client) => { grpc_client = Some(client); break; }
                Err(e) => {
                    retry_count += 1;
                    if retry_count >= max_retries { return Err(e.into()); }
                    tokio::time::sleep(std::time::Duration::from_secs(1)).await;
                }
            }
        }
        let object_client = grpc_client.unwrap();
        let function_client = FunctionServiceClient::connect(grpc_endpoint.clone())
            .await
            .map_err(|e| format!("Failed to connect to FunctionService: {}", e))?;

        let state = AppState {
            object_client,
            function_client,
            health_service: self.health_service,
            config: self.config.clone(),
        };

        let mut app = Router::new()
            .route("/health", get(health_check))
            .route("/status", get(status_check))
            .route("/objects/{key}", put(put_object))
            .route("/objects/{key}", get(get_object))
            .route("/objects", get(list_objects))
            .route("/objects/{key}/refs", post(add_object_ref))
            .route("/objects/{key}/refs", delete(remove_object_ref))
            .route("/objects/{key}/pin", post(pin_object))
            .route("/objects/{key}/pin", delete(unpin_object))
            .route("/objects/{key}", delete(delete_object))
            .route("/functions/execute", post(execute_function))
            .route("/functions/executions/{execution_id}", get(get_execution_status))
            .route("/functions/executions/{execution_id}/cancel", post(cancel_execution))
            .route("/tasks", get(list_tasks))
            .route("/tasks/{task_id}", get(get_task))
            .route("/tasks/{task_id}/executions", get(get_task_executions))
            .route("/monitoring/stats", get(get_stats))
            .route("/monitoring/health", get(get_health_status))
            .with_state(state);

        if self.config.http.swagger_enabled {
            app = app
                .route("/api-docs", get(api_docs))
                .route("/api/openapi.json", get(api_docs))
                .route("/swagger-ui", get(swagger_ui))
                .route("/docs", get(swagger_ui));
        }

        let listener = tokio::net::TcpListener::bind(addr).await?;
        info!("HTTP gateway listening on {}", addr);
        if self.config.http.swagger_enabled {
            info!("Swagger UI available at:");
            info!("  - http://{}/swagger-ui", addr);
            info!("  - http://{}/docs", addr);
            info!("  - OpenAPI JSON: http://{}/api/openapi.json", addr);
        }

        Ok((listener, app))
    }
}

/// Health check endpoint / 健康检查端点
/// GET /health
async fn health_check(State(state): State<AppState>) -> Result<Json<serde_json::Value>, StatusCode> {
    let health_status = state.health_service.get_health_status().await;
    
    Ok(Json(serde_json::json!({
        "status": health_status.status,
        "timestamp": chrono::Utc::now().to_rfc3339(),
        "service": "spearlet"
    })))
}

/// Status check endpoint / 状态检查端点
/// GET /status
async fn status_check(State(state): State<AppState>) -> Result<Json<serde_json::Value>, StatusCode> {
    let health_status = state.health_service.get_health_status().await;
    
    Ok(Json(serde_json::json!({
        "status": health_status.status,
        "object_count": health_status.object_count,
        "total_object_size": health_status.total_object_size,
        "pinned_object_count": health_status.pinned_object_count,
        "node_name": state.config.node_name
    })))
}

#[derive(Deserialize)]
struct PutObjectBody {
    value: String, // Base64 encoded / Base64编码
    metadata: Option<HashMap<String, String>>,
    overwrite: Option<bool>,
}

/// Put object endpoint / 存储对象端点
/// PUT /objects/:key
async fn put_object(
    Path(key): Path<String>,
    State(state): State<AppState>,
    Json(body): Json<PutObjectBody>,
) -> Result<Json<serde_json::Value>, StatusCode> {
    debug!("PUT /objects/{}", key);

    // Decode base64 value / 解码base64值
    let value = match general_purpose::STANDARD.decode(&body.value) {
        Ok(v) => v,
        Err(_) => return Err(StatusCode::BAD_REQUEST),
    };

    let request = PutObjectRequest {
        key: key.clone(),
        value,
        metadata: body.metadata.unwrap_or_default(),
        overwrite: body.overwrite.unwrap_or(false),
    };

    let mut client = state.object_client.clone();
    match client.put_object(request).await {
        Ok(response) => {
            let resp = response.into_inner();
            Ok(Json(serde_json::json!({
                "success": resp.success,
                "message": resp.message,
                "key": key
            })))
        }
        Err(e) => {
            error!("Failed to put object {}: {}", key, e);
            Err(StatusCode::INTERNAL_SERVER_ERROR)
        }
    }
}

/// Get object endpoint / 获取对象端点
/// GET /objects/:key
async fn get_object(
    Path(key): Path<String>,
    State(state): State<AppState>,
) -> Result<Json<serde_json::Value>, StatusCode> {
    debug!("GET /objects/{}", key);

    let request = GetObjectRequest {
        key: key.clone(),
        include_value: true,
    };

    let mut client = state.object_client.clone();
    match client.get_object(request).await {
        Ok(response) => {
            let resp = response.into_inner();
            if resp.found {
                if let Some(object) = resp.object {
                    // Encode value as base64 / 将值编码为base64
                    let encoded_value = general_purpose::STANDARD.encode(&object.value);
                    Ok(Json(serde_json::json!({
                        "found": true,
                        "key": object.key,
                        "value": encoded_value,
                        "metadata": object.metadata,
                        "size": object.value.len(),
                        "created_at": object.created_at,
                        "updated_at": object.updated_at,
                        "ref_count": object.ref_count,
                        "pinned": object.pinned
                    })))
                } else {
                    Err(StatusCode::NOT_FOUND)
                }
            } else {
                Err(StatusCode::NOT_FOUND)
            }
        }
        Err(e) => {
            error!("Failed to get object {}: {}", key, e);
            Err(StatusCode::INTERNAL_SERVER_ERROR)
        }
    }
}

#[derive(Deserialize)]
struct ListObjectsQuery {
    prefix: Option<String>,
    limit: Option<i32>,
    continuation_token: Option<String>,
}

/// List objects endpoint / 列出对象端点
/// GET /objects
async fn list_objects(
    Query(params): Query<ListObjectsQuery>,
    State(state): State<AppState>,
) -> Result<Json<serde_json::Value>, StatusCode> {
    debug!("GET /objects with prefix: {:?}", params.prefix);

    let request = ListObjectsRequest {
        prefix: params.prefix.unwrap_or_default(),
        limit: params.limit.unwrap_or(100),
        start_after: params.continuation_token.unwrap_or_default(),
        include_values: true,
    };

    let mut client = state.object_client.clone();
    match client.list_objects(request).await {
        Ok(response) => {
            let resp = response.into_inner();
            // Manually build JSON response since proto types don't have serde support
            // 手动构建JSON响应，因为proto类型不支持serde
            let objects: Vec<serde_json::Value> = resp.objects.into_iter().map(|obj| {
                serde_json::json!({
                    "key": obj.key,
                    "size": obj.size,
                    "created_at": obj.created_at,
                    "updated_at": obj.updated_at,
                    "metadata": obj.metadata,
                    "ref_count": obj.ref_count,
                    "is_pinned": obj.pinned
                })
            }).collect();
            
            Ok(Json(serde_json::json!({
                "objects": objects,
                "continuation_token": resp.next_start_after,
                "has_more": resp.has_more
            })))
        }
        Err(e) => {
            error!("Failed to list objects: {}", e);
            Err(StatusCode::INTERNAL_SERVER_ERROR)
        }
    }
}

#[derive(Deserialize)]
struct RefCountBody {
    count: Option<i32>,
}

/// Add object reference endpoint / 添加对象引用端点
/// POST /objects/:key/refs
async fn add_object_ref(
    Path(key): Path<String>,
    State(state): State<AppState>,
    Json(body): Json<RefCountBody>,
) -> Result<Json<serde_json::Value>, StatusCode> {
    debug!("POST /objects/{}/refs", key);

    let request = AddObjectRefRequest {
        key: key.clone(),
        count: body.count.unwrap_or(1),
    };

    let mut client = state.object_client.clone();
    match client.add_object_ref(request).await {
        Ok(response) => {
            let resp = response.into_inner();
            Ok(Json(serde_json::json!({
                "success": resp.success,
                "message": resp.message,
                "new_ref_count": resp.new_ref_count
            })))
        }
        Err(e) => {
            error!("Failed to add object ref {}: {}", key, e);
            Err(StatusCode::INTERNAL_SERVER_ERROR)
        }
    }
}

/// Remove object reference endpoint / 移除对象引用端点
/// DELETE /objects/:key/refs
async fn remove_object_ref(
    Path(key): Path<String>,
    State(state): State<AppState>,
    Json(body): Json<RefCountBody>,
) -> Result<Json<serde_json::Value>, StatusCode> {
    debug!("DELETE /objects/{}/refs", key);

    let request = RemoveObjectRefRequest {
        key: key.clone(),
        count: body.count.unwrap_or(1),
    };

    let mut client = state.object_client.clone();
    match client.remove_object_ref(request).await {
        Ok(response) => {
            let resp = response.into_inner();
            Ok(Json(serde_json::json!({
                "success": resp.success,
                "message": resp.message,
                "new_ref_count": resp.new_ref_count
            })))
        }
        Err(e) => {
            error!("Failed to remove object ref {}: {}", key, e);
            Err(StatusCode::INTERNAL_SERVER_ERROR)
        }
    }
}

/// Pin object endpoint / 固定对象端点
/// POST /objects/:key/pin
async fn pin_object(
    Path(key): Path<String>,
    State(state): State<AppState>,
) -> Result<Json<serde_json::Value>, StatusCode> {
    debug!("POST /objects/{}/pin", key);

    let request = PinObjectRequest {
        key: key.clone(),
    };

    let mut client = state.object_client.clone();
    match client.pin_object(request).await {
        Ok(response) => {
            let resp = response.into_inner();
            Ok(Json(serde_json::json!({
                "success": resp.success,
                "message": resp.message
            })))
        }
        Err(e) => {
            error!("Failed to pin object {}: {}", key, e);
            Err(StatusCode::INTERNAL_SERVER_ERROR)
        }
    }
}

/// Unpin object endpoint / 取消固定对象端点
/// DELETE /objects/:key/pin
async fn unpin_object(
    Path(key): Path<String>,
    State(state): State<AppState>,
) -> Result<Json<serde_json::Value>, StatusCode> {
    debug!("DELETE /objects/{}/pin", key);

    let request = UnpinObjectRequest {
        key: key.clone(),
    };

    let mut client = state.object_client.clone();
    match client.unpin_object(request).await {
        Ok(response) => {
            let resp = response.into_inner();
            Ok(Json(serde_json::json!({
                "success": resp.success,
                "message": resp.message
            })))
        }
        Err(e) => {
            error!("Failed to unpin object {}: {}", key, e);
            Err(StatusCode::INTERNAL_SERVER_ERROR)
        }
    }
}

#[derive(Deserialize)]
struct DeleteObjectQuery {
    force: Option<bool>,
}

/// Delete object endpoint / 删除对象端点
/// DELETE /objects/:key
async fn delete_object(
    Path(key): Path<String>,
    Query(params): Query<DeleteObjectQuery>,
    State(state): State<AppState>,
) -> Result<Json<serde_json::Value>, StatusCode> {
    debug!("DELETE /objects/{}", key);

    let request = DeleteObjectRequest {
        key: key.clone(),
        force: params.force.unwrap_or(false),
    };

    let mut client = state.object_client.clone();
    match client.delete_object(request).await {
        Ok(response) => {
            let resp = response.into_inner();
            Ok(Json(serde_json::json!({
                "success": resp.success,
                "message": resp.message
            })))
        }
        Err(e) => {
            error!("Failed to delete object {}: {}", key, e);
            Err(StatusCode::INTERNAL_SERVER_ERROR)
        }
    }
}

// Function execution endpoints / 函数执行端点

/// Execute function endpoint / 执行函数端点
/// POST /functions/execute
async fn execute_function(
    State(_state): State<AppState>,
    Json(_body): Json<serde_json::Value>,
) -> Result<Json<serde_json::Value>, StatusCode> {
    debug!("POST /functions/execute");
    
    // TODO: Implement function execution logic
    // 待实现：函数执行逻辑
    
    Ok(Json(serde_json::json!({
        "success": true,
        "message": "Function execution endpoint - Not implemented yet",
        "execution_id": "todo-generate-execution-id"
    })))
}

/// Get execution status endpoint / 获取执行状态端点
/// GET /functions/executions/:execution_id
async fn get_execution_status(
    State(_state): State<AppState>,
    Path(execution_id): Path<String>,
) -> Result<Json<serde_json::Value>, StatusCode> {
    debug!("GET /functions/executions/{}", execution_id);
    
    // TODO: Implement execution status retrieval
    // 待实现：执行状态获取逻辑
    
    Ok(Json(serde_json::json!({
        "execution_id": execution_id,
        "status": "pending",
        "message": "Execution status endpoint - Not implemented yet"
    })))
}

/// Cancel execution endpoint / 取消执行端点
/// POST /functions/executions/:execution_id/cancel
async fn cancel_execution(
    State(_state): State<AppState>,
    Path(execution_id): Path<String>,
) -> Result<Json<serde_json::Value>, StatusCode> {
    debug!("POST /functions/executions/{}/cancel", execution_id);
    
    // TODO: Implement execution cancellation
    // 待实现：执行取消逻辑
    
    Ok(Json(serde_json::json!({
        "success": true,
        "message": "Execution cancellation endpoint - Not implemented yet",
        "execution_id": execution_id
    })))
}

// Task management endpoints / 任务管理端点

/// List tasks endpoint / 列出任务端点
/// GET /tasks
async fn list_tasks(
    State(_state): State<AppState>,
    Query(_params): Query<HashMap<String, String>>,
) -> Result<Json<serde_json::Value>, StatusCode> {
    debug!("GET /tasks");
    
    // TODO: Implement task listing logic
    // 待实现：任务列表获取逻辑
    
    Ok(Json(serde_json::json!({
        "tasks": [],
        "has_more": false,
        "message": "Task listing endpoint - Not implemented yet"
    })))
}

/// Get task details endpoint / 获取任务详情端点
/// GET /tasks/:task_id
async fn get_task(
    State(_state): State<AppState>,
    Path(task_id): Path<String>,
) -> Result<Json<serde_json::Value>, StatusCode> {
    debug!("GET /tasks/{}", task_id);
    
    // TODO: Implement task details retrieval
    // 待实现：任务详情获取逻辑
    
    Ok(Json(serde_json::json!({
        "task_id": task_id,
        "name": "example-task",
        "status": "active",
        "message": "Task details endpoint - Not implemented yet"
    })))
}

/// Get task executions endpoint / 获取任务执行记录端点
/// GET /tasks/:task_id/executions
async fn get_task_executions(
    State(_state): State<AppState>,
    Path(task_id): Path<String>,
    Query(_params): Query<HashMap<String, String>>,
) -> Result<Json<serde_json::Value>, StatusCode> {
    debug!("GET /tasks/{}/executions", task_id);
    
    // TODO: Implement task executions retrieval
    // 待实现：任务执行记录获取逻辑
    
    Ok(Json(serde_json::json!({
        "task_id": task_id,
        "executions": [],
        "message": "Task executions endpoint - Not implemented yet"
    })))
}

// Monitoring endpoints / 监控端点

/// Get statistics endpoint / 获取统计信息端点
/// GET /monitoring/stats
async fn get_stats(
    State(_state): State<AppState>,
) -> Result<Json<serde_json::Value>, StatusCode> {
    debug!("GET /monitoring/stats");
    
    // TODO: Implement statistics retrieval
    // 待实现：统计信息获取逻辑
    
    Ok(Json(serde_json::json!({
        "total_executions": 0,
        "successful_executions": 0,
        "failed_executions": 0,
        "active_executions": 0,
        "message": "Statistics endpoint - Not implemented yet"
    })))
}

/// Get health status endpoint / 获取健康状态端点
/// GET /monitoring/health
async fn get_health_status(
    State(_state): State<AppState>,
) -> Result<Json<serde_json::Value>, StatusCode> {
    debug!("GET /monitoring/health");
    
    // TODO: Implement health status retrieval
    // 待实现：健康状态获取逻辑
    
    Ok(Json(serde_json::json!({
        "status": "healthy",
        "timestamp": chrono::Utc::now().to_rfc3339(),
        "message": "Health status endpoint - Not implemented yet"
    })))
}

/// API documentation endpoint / API文档端点
/// GET /api-docs
async fn api_docs() -> Json<serde_json::Value> {
    Json(serde_json::json!({
        "openapi": "3.0.0",
        "info": {
            "title": "SPEARlet API",
            "description": "SPEARlet HTTP Gateway API - SPEAR core agent component / SPEARlet HTTP网关API - SPEAR核心代理组件",
            "version": "0.1.0",
            "contact": {
                "name": "SPEAR Team",
                "url": "https://github.com/spear-ai/spear"
            }
        },
        "servers": [
            {
                "url": "/",
                "description": "Local server / 本地服务器"
            }
        ],
        "tags": [
            {
                "name": "System",
                "description": "System health and status endpoints / 系统健康和状态端点"
            },
            {
                "name": "Objects",
                "description": "Object storage, reference and pinning operations / 对象存储、引用和固定操作"
            },
            {
                "name": "Functions",
                "description": "Function execution and task management / 函数执行和任务管理"
            },
            {
                "name": "Monitoring",
                "description": "Service monitoring and statistics / 服务监控和统计"
            }
        ],
        "paths": {
            "/health": {
                "get": {
                    "tags": ["System"],
                    "summary": "Health check / 健康检查",
                    "description": "Check if the SPEARlet service is healthy / 检查SPEARlet服务是否健康",
                    "responses": {
                        "200": {
                            "description": "Service is healthy / 服务健康",
                            "content": {
                                "application/json": {
                                    "schema": {
                                        "type": "object",
                                        "properties": {
                                            "service": {
                                                "type": "string",
                                                "example": "spearlet"
                                            },
                                            "status": {
                                                "type": "string",
                                                "example": "healthy"
                                            },
                                            "timestamp": {
                                                "type": "string",
                                                "format": "date-time",
                                                "example": "2024-01-01T00:00:00Z"
                                            }
                                        }
                                    }
                                }
                            }
                        }
                    }
                }
            },
            "/status": {
                "get": {
                    "tags": ["System"],
                    "summary": "Get node status / 获取节点状态",
                    "description": "Get detailed status information about the SPEARlet node / 获取SPEARlet节点的详细状态信息",
                    "responses": {
                        "200": {
                            "description": "Node status information / 节点状态信息",
                            "content": {
                                "application/json": {
                                    "schema": {
                                        "type": "object",
                                        "properties": {
                                            "node_name": {"type": "string"},
                                            "status": {"type": "string"},
                                            "object_count": {"type": "integer"},
                                            "total_object_size": {"type": "integer"},
                                            "pinned_object_count": {"type": "integer"}
                                        }
                                    }
                                }
                            }
                        }
                    }
                }
            },
            "/objects": {
                "get": {
                    "tags": ["Objects"],
                    "summary": "List objects / 列出对象",
                    "description": "List all stored objects / 列出所有存储的对象",
                    "parameters": [
                        {
                            "name": "prefix",
                            "in": "query",
                            "description": "Filter objects by prefix / 按前缀过滤对象",
                            "schema": {"type": "string"}
                        },
                        {
                            "name": "limit",
                            "in": "query",
                            "description": "Maximum number of objects to return / 返回对象的最大数量",
                            "schema": {
                                "type": "integer",
                                "minimum": 1,
                                "maximum": 1000,
                                "default": 100
                            }
                        },
                        {
                            "name": "continuation_token",
                            "in": "query",
                            "description": "Token for pagination / 分页令牌",
                            "schema": {"type": "string"}
                        }
                    ],
                    "responses": {
                        "200": {
                            "description": "List of objects / 对象列表",
                            "content": {
                                "application/json": {
                                    "schema": {
                                        "type": "object",
                                        "properties": {
                                            "objects": {
                                                "type": "array",
                                                "items": {
                                                    "type": "object",
                                                    "properties": {
                                                        "key": {"type": "string"},
                                                        "size": {"type": "integer"},
                                                        "created_at": {"type": "string", "format": "date-time"},
                                                        "updated_at": {"type": "string", "format": "date-time"},
                                                        "ref_count": {"type": "integer"},
                                                        "is_pinned": {"type": "boolean"}
                                                    }
                                                }
                                            },
                                            "continuation_token": {"type": "string"},
                                            "has_more": {"type": "boolean"}
                                        }
                                    }
                                }
                            }
                        }
                    }
                }
            },
            "/objects/{key}": {
                "put": {
                    "tags": ["Objects"],
                    "summary": "Store object / 存储对象",
                    "description": "Store an object with the specified key / 使用指定键存储对象",
                    "parameters": [
                        {
                            "name": "key",
                            "in": "path",
                            "required": true,
                            "description": "Object key / 对象键",
                            "schema": {"type": "string"}
                        }
                    ],
                    "requestBody": {
                        "description": "Object data / 对象数据",
                        "required": true,
                        "content": {
                            "application/json": {
                                "schema": {
                                    "type": "object",
                                    "properties": {
                                        "value": {
                                            "type": "string",
                                            "description": "Base64 encoded object value / Base64编码的对象值"
                                        },
                                        "metadata": {
                                            "type": "object",
                                            "additionalProperties": {"type": "string"},
                                            "description": "Object metadata / 对象元数据"
                                        },
                                        "overwrite": {
                                            "type": "boolean",
                                            "default": false,
                                            "description": "Whether to overwrite existing object / 是否覆盖现有对象"
                                        }
                                    },
                                    "required": ["value"]
                                }
                            }
                        }
                    },
                    "responses": {
                        "200": {
                            "description": "Object stored successfully / 对象存储成功",
                            "content": {
                                "application/json": {
                                    "schema": {
                                        "type": "object",
                                        "properties": {
                                            "success": {"type": "boolean"},
                                            "message": {"type": "string"},
                                            "key": {"type": "string"}
                                        }
                                    }
                                }
                            }
                        },
                        "400": {"description": "Bad request / 请求错误"}
                    }
                },
                "get": {
                    "tags": ["Objects"],
                    "summary": "Get object / 获取对象",
                    "description": "Retrieve an object by its key / 通过键检索对象",
                    "parameters": [
                        {
                            "name": "key",
                            "in": "path",
                            "required": true,
                            "description": "Object key / 对象键",
                            "schema": {"type": "string"}
                        }
                    ],
                    "responses": {
                        "200": {
                            "description": "Object data / 对象数据",
                            "content": {
                                "application/json": {
                                    "schema": {
                                        "type": "object",
                                        "properties": {
                                            "found": {"type": "boolean"},
                                            "key": {"type": "string"},
                                            "value": {"type": "string", "description": "Base64 encoded value"},
                                            "metadata": {"type": "object"},
                                            "size": {"type": "integer"},
                                            "created_at": {"type": "string", "format": "date-time"},
                                            "updated_at": {"type": "string", "format": "date-time"},
                                            "ref_count": {"type": "integer"},
                                            "pinned": {"type": "boolean"}
                                        }
                                    }
                                }
                            }
                        },
                        "404": {"description": "Object not found / 对象未找到"}
                    }
                },
                "delete": {
                    "tags": ["Objects"],
                    "summary": "Delete object / 删除对象",
                    "description": "Delete an object by its key / 通过键删除对象",
                    "parameters": [
                        {
                            "name": "key",
                            "in": "path",
                            "required": true,
                            "description": "Object key / 对象键",
                            "schema": {"type": "string"}
                        },
                        {
                            "name": "force",
                            "in": "query",
                            "description": "Force delete even if object has references / 即使对象有引用也强制删除",
                            "schema": {"type": "boolean", "default": false}
                        }
                    ],
                    "responses": {
                        "200": {
                            "description": "Object deleted successfully / 对象删除成功",
                            "content": {
                                "application/json": {
                                    "schema": {
                                        "type": "object",
                                        "properties": {
                                            "success": {"type": "boolean"},
                                            "message": {"type": "string"}
                                        }
                                    }
                                }
                            }
                        },
                        "404": {"description": "Object not found / 对象未找到"}
                    }
                }
            },
            "/objects/{key}/refs": {
                "post": {
                    "tags": ["Objects"],
                    "summary": "Add object reference / 添加对象引用",
                    "description": "Add a reference to an object / 为对象添加引用",
                    "parameters": [
                        {
                            "name": "key",
                            "in": "path",
                            "required": true,
                            "description": "Object key / 对象键",
                            "schema": {"type": "string"}
                        }
                    ],
                    "requestBody": {
                        "description": "Reference count / 引用计数",
                        "content": {
                            "application/json": {
                                "schema": {
                                    "type": "object",
                                    "properties": {
                                        "count": {"type": "integer", "default": 1}
                                    }
                                }
                            }
                        }
                    },
                    "responses": {
                        "200": {"description": "Reference added / 引用已添加"},
                        "404": {"description": "Object not found / 对象未找到"}
                    }
                },
                "delete": {
                    "tags": ["Objects"],
                    "summary": "Remove object reference / 移除对象引用",
                    "description": "Remove a reference from an object / 从对象移除引用",
                    "parameters": [
                        {
                            "name": "key",
                            "in": "path",
                            "required": true,
                            "description": "Object key / 对象键",
                            "schema": {"type": "string"}
                        }
                    ],
                    "requestBody": {
                        "description": "Reference count to remove / 要移除的引用计数",
                        "content": {
                            "application/json": {
                                "schema": {
                                    "type": "object",
                                    "properties": {
                                        "count": {"type": "integer", "default": 1}
                                    }
                                }
                            }
                        }
                    },
                    "responses": {
                        "200": {"description": "Reference removed / 引用已移除"},
                        "404": {"description": "Object not found / 对象未找到"}
                    }
                }
            },
            "/objects/{key}/pin": {
                "post": {
                    "tags": ["Objects"],
                    "summary": "Pin object / 固定对象",
                    "description": "Pin an object to prevent it from being garbage collected / 固定对象以防止被垃圾回收",
                    "parameters": [
                        {
                            "name": "key",
                            "in": "path",
                            "required": true,
                            "description": "Object key / 对象键",
                            "schema": {"type": "string"}
                        }
                    ],
                    "responses": {
                        "200": {"description": "Object pinned / 对象已固定"},
                        "404": {"description": "Object not found / 对象未找到"}
                    }
                },
                "delete": {
                    "tags": ["Objects"],
                    "summary": "Unpin object / 取消固定对象",
                    "description": "Unpin an object to allow it to be garbage collected / 取消固定对象以允许被垃圾回收",
                    "parameters": [
                        {
                            "name": "key",
                            "in": "path",
                            "required": true,
                            "description": "Object key / 对象键",
                            "schema": {"type": "string"}
                        }
                    ],
                    "responses": {
                        "200": {"description": "Object unpinned / 对象已取消固定"},
                        "404": {"description": "Object not found / 对象未找到"}
                    }
                }
            },
            "/functions/invoke": {
                "post": {
                    "tags": ["Functions"],
                    "summary": "Invoke function / 调用函数",
                    "description": "Execute a function with specified parameters / 使用指定参数执行函数",
                    "requestBody": {
                        "description": "Function invocation request / 函数调用请求",
                        "content": {
                            "application/json": {
                                "schema": {
                                    "type": "object",
                                    "required": ["function_name"],
                                    "properties": {
                                        "function_name": {
                                            "type": "string",
                                            "description": "Name of the function to invoke / 要调用的函数名称"
                                        },
                                        "parameters": {
                                            "type": "object",
                                            "description": "Function parameters / 函数参数",
                                            "additionalProperties": true
                                        },
                                        "invocation_type": {
                                            "type": "string",
                                            "enum": ["SYNC", "ASYNC"],
                                            "default": "SYNC",
                                            "description": "Invocation type / 调用类型"
                                        },
                                        "execution_mode": {
                                            "type": "string",
                                            "enum": ["NORMAL", "DEBUG"],
                                            "default": "NORMAL",
                                            "description": "Execution mode / 执行模式"
                                        },
                                        "timeout_seconds": {
                                            "type": "integer",
                                            "description": "Execution timeout in seconds / 执行超时时间（秒）"
                                        }
                                    }
                                }
                            }
                        }
                    },
                    "responses": {
                        "200": {
                            "description": "Function execution result / 函数执行结果",
                            "content": {
                                "application/json": {
                                    "schema": {
                                        "type": "object",
                                        "properties": {
                                            "execution_id": {"type": "string"},
                                            "status": {"type": "string"},
                                            "result": {"type": "object"},
                                            "error": {"type": "string"},
                                            "execution_time_ms": {"type": "integer"}
                                        }
                                    }
                                }
                            }
                        },
                        "400": {"description": "Invalid request / 无效请求"},
                        "500": {"description": "Execution error / 执行错误"}
                    }
                }
            },
            "/functions/executions/{execution_id}/status": {
                "get": {
                    "tags": ["Functions"],
                    "summary": "Get execution status / 获取执行状态",
                    "description": "Get the status of a function execution / 获取函数执行的状态",
                    "parameters": [
                        {
                            "name": "execution_id",
                            "in": "path",
                            "required": true,
                            "description": "Execution ID / 执行ID",
                            "schema": {"type": "string"}
                        }
                    ],
                    "responses": {
                        "200": {
                            "description": "Execution status / 执行状态",
                            "content": {
                                "application/json": {
                                    "schema": {
                                        "type": "object",
                                        "properties": {
                                            "execution_id": {"type": "string"},
                                            "status": {
                                                "type": "string",
                                                "enum": ["PENDING", "RUNNING", "COMPLETED", "FAILED", "CANCELLED"]
                                            },
                                            "result": {"type": "object"},
                                            "error": {"type": "string"},
                                            "start_time": {"type": "string", "format": "date-time"},
                                            "end_time": {"type": "string", "format": "date-time"},
                                            "execution_time_ms": {"type": "integer"}
                                        }
                                    }
                                }
                            }
                        },
                        "404": {"description": "Execution not found / 执行未找到"}
                    }
                }
            },
            "/functions/executions/{execution_id}/cancel": {
                "post": {
                    "tags": ["Functions"],
                    "summary": "Cancel execution / 取消执行",
                    "description": "Cancel a running function execution / 取消正在运行的函数执行",
                    "parameters": [
                        {
                            "name": "execution_id",
                            "in": "path",
                            "required": true,
                            "description": "Execution ID / 执行ID",
                            "schema": {"type": "string"}
                        }
                    ],
                    "responses": {
                        "200": {"description": "Execution cancelled / 执行已取消"},
                        "404": {"description": "Execution not found / 执行未找到"},
                        "409": {"description": "Cannot cancel execution / 无法取消执行"}
                    }
                }
            },
            "/functions/stream": {
                "post": {
                    "tags": ["Functions"],
                    "summary": "Stream function execution / 流式函数执行",
                    "description": "Execute a function with streaming results / 执行函数并流式返回结果",
                    "requestBody": {
                        "description": "Function streaming request / 函数流式请求",
                        "content": {
                            "application/json": {
                                "schema": {
                                    "type": "object",
                                    "required": ["function_name"],
                                    "properties": {
                                        "function_name": {"type": "string"},
                                        "parameters": {"type": "object", "additionalProperties": true},
                                        "execution_mode": {
                                            "type": "string",
                                            "enum": ["NORMAL", "DEBUG"],
                                            "default": "NORMAL"
                                        }
                                    }
                                }
                            }
                        }
                    },
                    "responses": {
                        "200": {
                            "description": "Streaming execution results / 流式执行结果",
                            "content": {
                                "text/event-stream": {
                                    "schema": {
                                        "type": "string",
                                        "description": "Server-sent events stream / 服务器发送事件流"
                                    }
                                }
                            }
                        }
                    }
                }
            },
            "/tasks": {
                "get": {
                    "tags": ["Functions"],
                    "summary": "List tasks / 列出任务",
                    "description": "Get a list of all tasks / 获取所有任务的列表",
                    "parameters": [
                        {
                            "name": "limit",
                            "in": "query",
                            "description": "Maximum number of tasks to return / 返回的最大任务数",
                            "schema": {"type": "integer", "default": 100}
                        },
                        {
                            "name": "offset",
                            "in": "query", 
                            "description": "Number of tasks to skip / 跳过的任务数",
                            "schema": {"type": "integer", "default": 0}
                        },
                        {
                            "name": "status",
                            "in": "query",
                            "description": "Filter by task status / 按任务状态过滤",
                            "schema": {
                                "type": "string",
                                "enum": ["PENDING", "RUNNING", "COMPLETED", "FAILED", "CANCELLED"]
                            }
                        }
                    ],
                    "responses": {
                        "200": {
                            "description": "List of tasks / 任务列表",
                            "content": {
                                "application/json": {
                                    "schema": {
                                        "type": "object",
                                        "properties": {
                                            "tasks": {
                                                "type": "array",
                                                "items": {
                                                    "type": "object",
                                                    "properties": {
                                                        "task_id": {"type": "string"},
                                                        "function_name": {"type": "string"},
                                                        "status": {"type": "string"},
                                                        "created_at": {"type": "string", "format": "date-time"},
                                                        "updated_at": {"type": "string", "format": "date-time"},
                                                        "execution_count": {"type": "integer"}
                                                    }
                                                }
                                            },
                                            "total": {"type": "integer"},
                                            "limit": {"type": "integer"},
                                            "offset": {"type": "integer"}
                                        }
                                    }
                                }
                            }
                        }
                    }
                }
            },
            "/tasks/{task_id}": {
                "get": {
                    "tags": ["Functions"],
                    "summary": "Get task details / 获取任务详情",
                    "description": "Get detailed information about a specific task / 获取特定任务的详细信息",
                    "parameters": [
                        {
                            "name": "task_id",
                            "in": "path",
                            "required": true,
                            "description": "Task ID / 任务ID",
                            "schema": {"type": "string"}
                        }
                    ],
                    "responses": {
                        "200": {
                            "description": "Task details / 任务详情",
                            "content": {
                                "application/json": {
                                    "schema": {
                                        "type": "object",
                                        "properties": {
                                            "task_id": {"type": "string"},
                                            "function_name": {"type": "string"},
                                            "status": {"type": "string"},
                                            "parameters": {"type": "object"},
                                            "created_at": {"type": "string", "format": "date-time"},
                                            "updated_at": {"type": "string", "format": "date-time"},
                                            "execution_count": {"type": "integer"},
                                            "last_execution": {
                                                "type": "object",
                                                "properties": {
                                                    "execution_id": {"type": "string"},
                                                    "status": {"type": "string"},
                                                    "result": {"type": "object"},
                                                    "error": {"type": "string"}
                                                }
                                            }
                                        }
                                    }
                                }
                            }
                        },
                        "404": {"description": "Task not found / 任务未找到"}
                    }
                },
                "delete": {
                    "tags": ["Functions"],
                    "summary": "Delete task / 删除任务",
                    "description": "Delete a specific task / 删除特定任务",
                    "parameters": [
                        {
                            "name": "task_id",
                            "in": "path",
                            "required": true,
                            "description": "Task ID / 任务ID",
                            "schema": {"type": "string"}
                        }
                    ],
                    "responses": {
                        "200": {"description": "Task deleted / 任务已删除"},
                        "404": {"description": "Task not found / 任务未找到"},
                        "409": {"description": "Cannot delete running task / 无法删除正在运行的任务"}
                    }
                }
            },
            "/tasks/{task_id}/executions": {
                "get": {
                    "tags": ["Functions"],
                    "summary": "List task executions / 列出任务执行记录",
                    "description": "Get execution history for a specific task / 获取特定任务的执行历史",
                    "parameters": [
                        {
                            "name": "task_id",
                            "in": "path",
                            "required": true,
                            "description": "Task ID / 任务ID",
                            "schema": {"type": "string"}
                        },
                        {
                            "name": "limit",
                            "in": "query",
                            "description": "Maximum number of executions to return / 返回的最大执行记录数",
                            "schema": {"type": "integer", "default": 50}
                        },
                        {
                            "name": "offset",
                            "in": "query",
                            "description": "Number of executions to skip / 跳过的执行记录数",
                            "schema": {"type": "integer", "default": 0}
                        }
                    ],
                    "responses": {
                        "200": {
                            "description": "List of task executions / 任务执行记录列表",
                            "content": {
                                "application/json": {
                                    "schema": {
                                        "type": "object",
                                        "properties": {
                                            "executions": {
                                                "type": "array",
                                                "items": {
                                                    "type": "object",
                                                    "properties": {
                                                        "execution_id": {"type": "string"},
                                                        "status": {"type": "string"},
                                                        "start_time": {"type": "string", "format": "date-time"},
                                                        "end_time": {"type": "string", "format": "date-time"},
                                                        "execution_time_ms": {"type": "integer"},
                                                        "result": {"type": "object"},
                                                        "error": {"type": "string"}
                                                    }
                                                }
                                            },
                                            "total": {"type": "integer"},
                                            "limit": {"type": "integer"},
                                            "offset": {"type": "integer"}
                                        }
                                    }
                                }
                            }
                        },
                        "404": {"description": "Task not found / 任务未找到"}
                    }
                }
            },
            "/functions/health": {
                "get": {
                    "tags": ["Monitoring"],
                    "summary": "Get function service health / 获取函数服务健康状态",
                    "description": "Get detailed health information about the function service / 获取函数服务的详细健康信息",
                    "responses": {
                        "200": {
                            "description": "Function service health status / 函数服务健康状态",
                            "content": {
                                "application/json": {
                                    "schema": {
                                        "type": "object",
                                        "properties": {
                                            "service": {"type": "string", "example": "function_service"},
                                            "status": {
                                                "type": "string",
                                                "enum": ["HEALTHY", "UNHEALTHY", "DEGRADED"],
                                                "example": "HEALTHY"
                                            },
                                            "timestamp": {"type": "string", "format": "date-time"},
                                            "details": {
                                                "type": "object",
                                                "properties": {
                                                    "active_executions": {"type": "integer"},
                                                    "pending_tasks": {"type": "integer"},
                                                    "total_memory_usage": {"type": "integer"},
                                                    "cpu_usage_percent": {"type": "number"},
                                                    "uptime_seconds": {"type": "integer"}
                                                }
                                            }
                                        }
                                    }
                                }
                            }
                        }
                    }
                }
            },
            "/functions/stats": {
                "get": {
                    "tags": ["Monitoring"],
                    "summary": "Get function service statistics / 获取函数服务统计信息",
                    "description": "Get comprehensive statistics about function executions and performance / 获取函数执行和性能的综合统计信息",
                    "responses": {
                        "200": {
                            "description": "Function service statistics / 函数服务统计信息",
                            "content": {
                                "application/json": {
                                    "schema": {
                                        "type": "object",
                                        "properties": {
                                            "service_stats": {
                                                "type": "object",
                                                "properties": {
                                                    "total_executions": {"type": "integer"},
                                                    "successful_executions": {"type": "integer"},
                                                    "failed_executions": {"type": "integer"},
                                                    "average_execution_time_ms": {"type": "number"},
                                                    "total_execution_time_ms": {"type": "integer"},
                                                    "active_executions": {"type": "integer"},
                                                    "peak_concurrent_executions": {"type": "integer"}
                                                }
                                            },
                                            "task_stats": {
                                                "type": "object",
                                                "properties": {
                                                    "total_tasks": {"type": "integer"},
                                                    "active_tasks": {"type": "integer"},
                                                    "completed_tasks": {"type": "integer"},
                                                    "failed_tasks": {"type": "integer"},
                                                    "average_task_duration_ms": {"type": "number"}
                                                }
                                            },
                                            "execution_stats": {
                                                "type": "object",
                                                "properties": {
                                                    "executions_per_minute": {"type": "number"},
                                                    "success_rate_percent": {"type": "number"},
                                                    "average_queue_time_ms": {"type": "number"},
                                                    "memory_usage_mb": {"type": "number"},
                                                    "cpu_usage_percent": {"type": "number"}
                                                }
                                            },
                                            "timestamp": {"type": "string", "format": "date-time"}
                                        }
                                    }
                                }
                            }
                        }
                    }
                }
            }
        }
    }))
}

/// Swagger UI HTML page / Swagger UI HTML页面
async fn swagger_ui() -> impl IntoResponse {
    let html = r#"
<!DOCTYPE html>
<html lang="en">
<head>
    <meta charset="UTF-8">
    <meta name="viewport" content="width=device-width, initial-scale=1.0">
    <title>SPEARlet API Documentation</title>
    <link rel="stylesheet" type="text/css" href="https://unpkg.com/swagger-ui-dist@5.9.0/swagger-ui.css" />
    <style>
        html {
            box-sizing: border-box;
            overflow: -moz-scrollbars-vertical;
            overflow-y: scroll;
        }
        *, *:before, *:after {
            box-sizing: inherit;
        }
        body {
            margin:0;
            background: #fafafa;
        }
        .swagger-ui .topbar {
            background-color: #1f2937;
        }
        .swagger-ui .topbar .download-url-wrapper {
            display: none;
        }
    </style>
</head>
<body>
    <div id="swagger-ui"></div>
    <script src="https://unpkg.com/swagger-ui-dist@5.9.0/swagger-ui-bundle.js"></script>
    <script src="https://unpkg.com/swagger-ui-dist@5.9.0/swagger-ui-standalone-preset.js"></script>
    <script>
        window.onload = function() {
            const ui = SwaggerUIBundle({
                url: '/api/openapi.json',
                dom_id: '#swagger-ui',
                deepLinking: true,
                presets: [
                    SwaggerUIBundle.presets.apis,
                    SwaggerUIStandalonePreset
                ],
                plugins: [
                    SwaggerUIBundle.plugins.DownloadUrl
                ],
                layout: "StandaloneLayout",
                validatorUrl: null,
                docExpansion: "list",
                defaultModelsExpandDepth: 1,
                defaultModelExpandDepth: 1,
                displayRequestDuration: true,
                tryItOutEnabled: true,
                filter: true,
                showExtensions: true,
                showCommonExtensions: true
            });
        };
    </script>
</body>
</html>
    "#;
    
    Html(html)
}
