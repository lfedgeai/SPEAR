//! Task HTTP handlers for SPEAR Metadata Server
//! SPEAR元数据服务器的任务HTTP处理器
//!
//! This module provides HTTP handlers that act as a gateway to the gRPC TaskService
//! 此模块提供作为gRPC TaskService网关的HTTP处理器

use axum::{
    extract::{Path, Query, State},
    http::StatusCode,
    response::Json,
};
use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use tonic::Request;
use tracing::{debug, error, info};

use crate::sms::FilterState;
use crate::proto::sms::{
    RegisterTaskRequest, ListTasksRequest, GetTaskRequest,
    UnregisterTaskRequest, TaskStatus, TaskPriority, TaskExecutable, ExecutableType
};
use crate::sms::gateway::GatewayState;
use super::common::ErrorResponse;

// HTTP request/response types / HTTP请求/响应类型

#[derive(Debug, Deserialize, Serialize)]
pub struct RegisterTaskParams {
    pub name: String,
    pub description: Option<String>,
    pub priority: Option<String>, // "low", "normal", "high"
    pub node_uuid: Option<String>,
    pub endpoint: String,
    pub version: String,
    pub capabilities: Option<Vec<String>>,
    pub metadata: Option<HashMap<String, String>>,
    pub config: Option<HashMap<String, String>>,
    pub executable: Option<TaskExecutableParams>,
}

#[derive(Debug, Deserialize, Serialize)]
pub struct TaskExecutableParams {
    pub r#type: String,
    pub uri: String,
    pub name: Option<String>,
    pub checksum_sha256: Option<String>,
    pub args: Option<Vec<String>>,
    pub env: Option<HashMap<String, String>>,
}

#[derive(Debug, Deserialize)]
pub struct ListTasksParams {
    pub node_uuid: Option<String>,
    pub status: Option<String>, // "unknown", "registered", "active", "inactive", "unregistered"
    pub priority: Option<String>, // "low", "normal", "high"
    pub limit: Option<i32>,
    pub offset: Option<i32>,
}

#[derive(Debug, Deserialize, Serialize)]
pub struct UnregisterTaskParams {
    pub reason: Option<String>,
}

#[derive(Debug, Serialize)]
pub struct TaskResponse {
    pub task_id: String,
    pub name: String,
    pub description: String,
    pub status: String,
    pub priority: String,
    pub node_uuid: String,
    pub endpoint: String,
    pub version: String,
    pub capabilities: Vec<String>,
    pub registered_at: i64,
    pub last_heartbeat: i64,
    pub metadata: HashMap<String, String>,
    pub config: HashMap<String, String>,
}

#[derive(Debug, Serialize)]
pub struct RegisterTaskResponse {
    pub success: bool,
    pub task_id: Option<String>,
    pub message: String,
}

#[derive(Debug, Serialize)]
pub struct ListTasksResponse {
    pub tasks: Vec<TaskResponse>,
    pub total_count: i32,
}

#[derive(Debug, Serialize)]
pub struct TaskActionResponse {
    pub success: bool,
    pub message: String,
}



// Helper function to convert TaskStatus enum / 转换TaskStatus枚举的辅助函数
fn parse_task_status(status: &str) -> Option<TaskStatus> {
    match status.to_lowercase().as_str() {
        "unknown" => Some(TaskStatus::Unknown),
        "registered" => Some(TaskStatus::Registered),
        "active" => Some(TaskStatus::Active),
        "inactive" => Some(TaskStatus::Inactive),
        "unregistered" => Some(TaskStatus::Unregistered),
        _ => None,
    }
}

// Helper function to convert TaskPriority enum / 转换TaskPriority枚举的辅助函数
fn parse_task_priority(priority: &str) -> TaskPriority {
    match priority.to_lowercase().as_str() {
        "unknown" => TaskPriority::Unknown,
        "low" => TaskPriority::Low,
        "normal" => TaskPriority::Normal,
        "high" => TaskPriority::High,
        "urgent" => TaskPriority::Urgent,
        _ => TaskPriority::Normal,
    }
}



// Helper function to convert proto Task to TaskResponse / 转换proto Task为TaskResponse的辅助函数
fn task_to_response(task: crate::proto::sms::Task) -> TaskResponse {
    TaskResponse {
        task_id: task.task_id,
        name: task.name,
        description: task.description,
        status: match TaskStatus::try_from(task.status).unwrap_or(TaskStatus::Unknown) {
            TaskStatus::Unknown => "unknown".to_string(),
            TaskStatus::Registered => "registered".to_string(),
            TaskStatus::Active => "active".to_string(),
            TaskStatus::Inactive => "inactive".to_string(),
            TaskStatus::Unregistered => "unregistered".to_string(),
        },
        priority: match TaskPriority::try_from(task.priority).unwrap_or(TaskPriority::Normal) {
            TaskPriority::Unknown => "unknown".to_string(),
            TaskPriority::Low => "low".to_string(),
            TaskPriority::Normal => "normal".to_string(),
            TaskPriority::High => "high".to_string(),
            TaskPriority::Urgent => "urgent".to_string(),
        },
        node_uuid: task.node_uuid,
        endpoint: task.endpoint,
        version: task.version,
        capabilities: task.capabilities,
        registered_at: task.registered_at,
        last_heartbeat: task.last_heartbeat,
        metadata: task.metadata,
        config: task.config,
    }
}

// HTTP Handlers / HTTP处理器

/// Register a new task / 注册新任务
pub async fn register_task(
    State(gateway_state): State<GatewayState>,
    Json(params): Json<RegisterTaskParams>,
) -> Result<Json<RegisterTaskResponse>, (StatusCode, Json<ErrorResponse>)> {
    info!("HTTP: Registering task: {}", params.name);

    let priority = params.priority
        .as_ref()
        .map(|p| parse_task_priority(p))
        .unwrap_or(TaskPriority::Normal);

    let request = Request::new(RegisterTaskRequest {
        name: params.name.clone(),
        description: params.description.unwrap_or_else(|| format!("Task: {}", params.name)),
        priority: priority as i32,
        node_uuid: params.node_uuid.unwrap_or_default(),
        endpoint: params.endpoint,
        version: params.version,
        capabilities: params.capabilities.unwrap_or_default(),
        metadata: params.metadata.unwrap_or_default(),
        config: params.config.unwrap_or_default(),
        executable: params.executable.as_ref().map(|e| TaskExecutable {
            r#type: match e.r#type.to_lowercase().as_str() {
                "binary" => ExecutableType::Binary as i32,
                "script" => ExecutableType::Script as i32,
                "container" => ExecutableType::Container as i32,
                "wasm" => ExecutableType::Wasm as i32,
                "process" => ExecutableType::Process as i32,
                _ => ExecutableType::Unknown as i32,
            },
            uri: e.uri.clone(),
            name: e.name.clone().unwrap_or_default(),
            checksum_sha256: e.checksum_sha256.clone().unwrap_or_default(),
            args: e.args.clone().unwrap_or_default(),
            env: e.env.clone().unwrap_or_default(),
        }),
    });

    match gateway_state.task_client.clone().register_task(request).await {
        Ok(response) => {
            let resp = response.into_inner();
            Ok(Json(RegisterTaskResponse {
                success: resp.success,
                task_id: Some(resp.task_id),
                message: resp.message,
            }))
        }
        Err(e) => {
            error!("Failed to register task: {}", e);
            Err((
                StatusCode::INTERNAL_SERVER_ERROR,
                Json(ErrorResponse {
                    error: "REGISTER_TASK_FAILED".to_string(),
                    message: format!("Failed to register task: {}", e),
                }),
            ))
        }
    }
}

/// List tasks with optional filters / 列出任务（可选过滤器）
pub async fn list_tasks(
    State(gateway_state): State<GatewayState>,
    Query(params): Query<ListTasksParams>,
) -> Result<Json<ListTasksResponse>, (StatusCode, Json<ErrorResponse>)> {
    debug!("HTTP: Listing tasks with filters: {:?}", params);

    // Convert optional filters to FilterState and then to i32 for protobuf compatibility
    // 将可选过滤器转换为FilterState，然后转换为i32以兼容protobuf
    let status_filter = params.status
        .as_ref()
        .and_then(|s| parse_task_status(s))
        .map(|s| FilterState::Value(s as i32))
        .unwrap_or(FilterState::None)
        .to_i32();
    
    let priority_filter = params.priority
        .as_ref()
        .map(|p| FilterState::Value(parse_task_priority(p) as i32))
        .unwrap_or(FilterState::None)
        .to_i32();

    let request = Request::new(ListTasksRequest {
        node_uuid: params.node_uuid.unwrap_or_default(),
        status_filter,
        priority_filter,
        limit: params.limit.unwrap_or(100),
        offset: params.offset.unwrap_or(0),
    });

    match gateway_state.task_client.clone().list_tasks(request).await {
        Ok(response) => {
            let resp = response.into_inner();
            let tasks = resp.tasks.into_iter().map(task_to_response).collect();
            
            Ok(Json(ListTasksResponse {
                tasks,
                total_count: resp.total_count,
            }))
        }
        Err(e) => {
            error!("Failed to list tasks: {}", e);
            Err((
                StatusCode::INTERNAL_SERVER_ERROR,
                Json(ErrorResponse {
                    error: "LIST_TASKS_FAILED".to_string(),
                    message: format!("Failed to list tasks: {}", e),
                }),
            ))
        }
    }
}

/// Get a specific task by ID / 根据ID获取特定任务
pub async fn get_task(
    State(gateway_state): State<GatewayState>,
    Path(task_id): Path<String>,
) -> Result<Json<TaskResponse>, (StatusCode, Json<ErrorResponse>)> {
    debug!("HTTP: Getting task: {}", task_id);

    let request = Request::new(GetTaskRequest { task_id });

    match gateway_state.task_client.clone().get_task(request).await {
        Ok(response) => {
            let resp = response.into_inner();
            if let Some(task) = resp.task {
                Ok(Json(task_to_response(task)))
            } else {
                Err((
                    StatusCode::NOT_FOUND,
                    Json(ErrorResponse {
                        error: "TASK_NOT_FOUND".to_string(),
                        message: "Task not found".to_string(),
                    }),
                ))
            }
        }
        Err(e) => {
            error!("Failed to get task: {}", e);
            Err((
                StatusCode::INTERNAL_SERVER_ERROR,
                Json(ErrorResponse {
                    error: "GET_TASK_FAILED".to_string(),
                    message: format!("Failed to get task: {}", e),
                }),
            ))
        }
    }
}

/// Unregister a task / 注销任务
pub async fn unregister_task(
    State(gateway_state): State<GatewayState>,
    Path(task_id): Path<String>,
    Json(params): Json<UnregisterTaskParams>,
) -> Result<Json<TaskActionResponse>, (StatusCode, Json<ErrorResponse>)> {
    info!("HTTP: Unregistering task: {}", task_id);

    let request = Request::new(UnregisterTaskRequest {
        task_id,
        reason: params.reason.unwrap_or_default(),
    });

    match gateway_state.task_client.clone().unregister_task(request).await {
        Ok(response) => {
            let resp = response.into_inner();
            Ok(Json(TaskActionResponse {
                success: resp.success,
                message: resp.message,
            }))
        }
        Err(e) => {
            error!("Failed to unregister task: {}", e);
            Err((
                StatusCode::INTERNAL_SERVER_ERROR,
                Json(ErrorResponse {
                    error: "UNREGISTER_TASK_FAILED".to_string(),
                    message: format!("Failed to unregister task: {}", e),
                }),
            ))
        }
    }
}
