//! HTTP gateway implementation for spearlet
//! spearlet的HTTP gateway实现

use std::collections::HashMap;
use std::net::SocketAddr;
use std::sync::Arc;
use axum::{
    extract::{Path, Query, State},
    http::StatusCode,
    response::Json,
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
    grpc_client: ObjectServiceClient<Channel>,
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
        let addr: SocketAddr = format!("{}:{}", self.config.http.address, self.config.http.port)
            .parse()
            .map_err(|e| format!("Invalid HTTP address: {}", e))?;
        
        info!("Starting HTTP gateway on {}", addr);

        // Connect to local gRPC server / 连接到本地gRPC服务器
        let grpc_endpoint = format!("http://{}:{}", self.config.grpc.address, self.config.grpc.port);
        let grpc_client = ObjectServiceClient::connect(grpc_endpoint).await?;

        let state = AppState {
            grpc_client,
            health_service: self.health_service,
            config: self.config.clone(),
        };

        // Build router / 构建路由器
        let mut app = Router::new()
            .route("/health", get(health_check))
            .route("/status", get(status_check))
            .route("/objects/:key", put(put_object))
            .route("/objects/:key", get(get_object))
            .route("/objects", get(list_objects))
            .route("/objects/:key/refs", post(add_object_ref))
            .route("/objects/:key/refs", delete(remove_object_ref))
            .route("/objects/:key/pin", post(pin_object))
            .route("/objects/:key/pin", delete(unpin_object))
            .route("/objects/:key", delete(delete_object))
            .with_state(state);

        // Add Swagger UI if enabled / 如果启用则添加Swagger UI
        if self.config.http.swagger_enabled {
            app = app.route("/api-docs", get(api_docs));
        }

        // Start server / 启动服务器
        let listener = tokio::net::TcpListener::bind(addr).await?;
        info!("HTTP gateway listening on {}", addr);
        
        axum::serve(listener, app).await?;
        
        Ok(())
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
        "node_id": state.config.node_id
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

    let mut client = state.grpc_client.clone();
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

    let mut client = state.grpc_client.clone();
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

    let mut client = state.grpc_client.clone();
    match client.list_objects(request).await {
        Ok(response) => {
            let resp = response.into_inner();
            Ok(Json(serde_json::json!({
                "objects": resp.objects,
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

    let mut client = state.grpc_client.clone();
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

    let mut client = state.grpc_client.clone();
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

    let mut client = state.grpc_client.clone();
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

    let mut client = state.grpc_client.clone();
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

    let mut client = state.grpc_client.clone();
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

/// API documentation endpoint / API文档端点
/// GET /api-docs
async fn api_docs() -> Json<serde_json::Value> {
    Json(serde_json::json!({
        "openapi": "3.0.0",
        "info": {
            "title": "SPEARlet Core Agent API",
"version": "1.0.0",
"description": "RESTful API for SPEARlet core agent component"
        },
        "paths": {
            "/health": {
                "get": {
                    "summary": "Health check",
                    "responses": {
                        "200": {
                            "description": "Service is healthy"
                        }
                    }
                }
            },
            "/status": {
                "get": {
                    "summary": "Service status",
                    "responses": {
                        "200": {
                            "description": "Service status information"
                        }
                    }
                }
            },
            "/objects/{key}": {
                "put": {
                    "summary": "Store an object",
                    "parameters": [
                        {
                            "name": "key",
                            "in": "path",
                            "required": true,
                            "schema": {
                                "type": "string"
                            }
                        }
                    ]
                },
                "get": {
                    "summary": "Retrieve an object",
                    "parameters": [
                        {
                            "name": "key",
                            "in": "path",
                            "required": true,
                            "schema": {
                                "type": "string"
                            }
                        }
                    ]
                },
                "delete": {
                    "summary": "Delete an object",
                    "parameters": [
                        {
                            "name": "key",
                            "in": "path",
                            "required": true,
                            "schema": {
                                "type": "string"
                            }
                        }
                    ]
                }
            }
        }
    }))
}