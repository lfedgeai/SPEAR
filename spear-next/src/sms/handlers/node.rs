//! Node HTTP handlers for SPEAR Metadata Server
//! SPEAR元数据服务器的节点HTTP处理器
//!
//! This module contains HTTP handlers for node management operations
//! 此模块包含节点管理操作的HTTP处理器

use axum::{
    extract::{Path, Query, State},
    http::StatusCode,
    response::Json,
};
use serde::{Deserialize, Serialize};
use serde_json::json;
use std::collections::HashMap;
use tracing::{info, warn};
use uuid::Uuid;

use crate::proto::sms::{
    Node, RegisterNodeRequest, ListNodesRequest, GetNodeRequest, 
    UpdateNodeRequest, DeleteNodeRequest, HeartbeatRequest,
};
use crate::sms::gateway::GatewayState;

/// Node registration request for HTTP API / HTTP API的节点注册请求
#[derive(Debug, Serialize, Deserialize)]
pub struct HttpRegisterNodeRequest {
    pub ip_address: String,
    pub port: i32,
    pub metadata: Option<HashMap<String, String>>,
}

/// Node update request for HTTP API / HTTP API的节点更新请求
#[derive(Debug, Serialize, Deserialize)]
pub struct HttpUpdateNodeRequest {
    pub ip_address: Option<String>,
    pub port: Option<i32>,
    pub status: Option<String>,
    pub metadata: Option<HashMap<String, String>>,
}

/// Heartbeat request for HTTP API / HTTP API的心跳请求
#[derive(Debug, Serialize, Deserialize)]
pub struct HttpHeartbeatRequest {
    pub health_info: Option<HashMap<String, String>>,
}

/// Query parameters for listing nodes / 列出节点的查询参数
#[derive(Debug, Deserialize)]
pub struct ListNodesQuery {
    pub status: Option<String>,
}

/// Register a new node / 注册新节点
pub async fn register_node(
    State(state): State<GatewayState>,
    Json(req): Json<HttpRegisterNodeRequest>,
) -> Result<(StatusCode, Json<serde_json::Value>), StatusCode> {
    let mut client = state.node_client.clone();
    
    let node = Node {
        uuid: Uuid::new_v4().to_string(),
        ip_address: req.ip_address,
        port: req.port,
        status: "active".to_string(),
        last_heartbeat: chrono::Utc::now().timestamp(),
        registered_at: chrono::Utc::now().timestamp(),
        metadata: req.metadata.unwrap_or_default(),
    };
    
    let grpc_req = RegisterNodeRequest {
        node: Some(node),
    };
    
    match client.register_node(grpc_req).await {
        Ok(response) => {
            let resp = response.into_inner();
            if resp.success {
                info!("Node registered successfully via HTTP: {}", resp.node_uuid);
                Ok((StatusCode::CREATED, Json(json!({
                    "success": true,
                    "message": resp.message,
                    "node_uuid": resp.node_uuid
                }))))
            } else {
                warn!("Failed to register node via HTTP: {}", resp.message);
                Ok((StatusCode::BAD_REQUEST, Json(json!({
                    "success": false,
                    "message": resp.message
                }))))
            }
        }
        Err(e) => {
            warn!("gRPC error during node registration: {}", e);
            Err(StatusCode::INTERNAL_SERVER_ERROR)
        }
    }
}

/// List all nodes / 列出所有节点
pub async fn list_nodes(
    State(state): State<GatewayState>,
    Query(query): Query<ListNodesQuery>,
) -> Result<Json<serde_json::Value>, StatusCode> {
    let mut client = state.node_client.clone();
    
    let status_filter = query.status.unwrap_or_default();
    info!("HTTP list_nodes called with status filter: '{}'", status_filter);
    
    let grpc_req = ListNodesRequest {
        status_filter,
    };
    
    match client.list_nodes(grpc_req).await {
        Ok(response) => {
            let resp = response.into_inner();
            info!("Listed {} nodes via HTTP", resp.nodes.len());
            // 手动构建节点数组以避免 serde 版本冲突 / Manually build node array to avoid serde version conflicts
            let nodes_json: Vec<serde_json::Value> = resp.nodes.into_iter().map(|node| {
                json!({
                    "uuid": node.uuid,
                    "ip_address": node.ip_address,
                    "port": node.port,
                    "status": node.status,
                    "last_heartbeat": node.last_heartbeat,
                    "registered_at": node.registered_at,
                    "metadata": node.metadata
                })
            }).collect();
            Ok(Json(json!({
                "success": true,
                "nodes": nodes_json
            })))
        }
        Err(e) => {
            warn!("gRPC error during node listing: {}", e);
            Err(StatusCode::INTERNAL_SERVER_ERROR)
        }
    }
}

/// Get a specific node / 获取特定节点
pub async fn get_node(
    State(state): State<GatewayState>,
    Path(uuid): Path<String>,
) -> Result<(StatusCode, Json<serde_json::Value>), StatusCode> {
    let mut client = state.node_client.clone();
    
    let grpc_req = GetNodeRequest { uuid };
    
    match client.get_node(grpc_req).await {
        Ok(response) => {
            let resp = response.into_inner();
            if resp.found {
                // 手动构建节点对象以避免 serde 版本冲突 / Manually build node object to avoid serde version conflicts
                let node_json = if let Some(node) = resp.node {
                    json!({
                        "uuid": node.uuid,
                        "ip_address": node.ip_address,
                        "port": node.port,
                        "status": node.status,
                        "last_heartbeat": node.last_heartbeat,
                        "registered_at": node.registered_at,
                        "metadata": node.metadata
                    })
                } else {
                    serde_json::Value::Null
                };
                Ok((StatusCode::OK, Json(json!({
                    "success": true,
                    "node": node_json
                }))))
            } else {
                Ok((StatusCode::NOT_FOUND, Json(json!({
                    "success": false,
                    "error": "Node not found"
                }))))
            }
        }
        Err(e) => {
            if e.code() == tonic::Code::NotFound {
                Ok((StatusCode::NOT_FOUND, Json(json!({
                    "success": false,
                    "error": "Node not found"
                }))))
            } else {
                warn!("gRPC error during node retrieval: {}", e);
                Err(StatusCode::INTERNAL_SERVER_ERROR)
            }
        }
    }
}

/// Update an existing node / 更新现有节点
pub async fn update_node(
    State(state): State<GatewayState>,
    Path(uuid): Path<String>,
    Json(req): Json<HttpUpdateNodeRequest>,
) -> Result<(StatusCode, Json<serde_json::Value>), StatusCode> {
    let mut client = state.node_client.clone();
    
    // First get the existing node / 首先获取现有节点
    let get_req = GetNodeRequest { uuid: uuid.clone() };
    
    let existing_node = match client.get_node(get_req).await {
        Ok(response) => {
            let resp = response.into_inner();
            if resp.found {
                resp.node.unwrap()
            } else {
                return Ok((StatusCode::NOT_FOUND, Json(json!({
                    "success": false,
                    "error": "Node not found"
                }))));
            }
        }
        Err(e) => {
            if e.code() == tonic::Code::NotFound {
                return Ok((StatusCode::NOT_FOUND, Json(json!({
                    "success": false,
                    "error": "Node not found"
                }))));
            } else {
                warn!("gRPC error during node retrieval for update: {}", e);
                return Err(StatusCode::INTERNAL_SERVER_ERROR);
            }
        }
    };
    
    // Update the node with new values / 使用新值更新节点
    let updated_node = Node {
        uuid: existing_node.uuid,
        ip_address: req.ip_address.unwrap_or(existing_node.ip_address),
        port: req.port.unwrap_or(existing_node.port),
        status: req.status.unwrap_or(existing_node.status),
        last_heartbeat: chrono::Utc::now().timestamp(),
        registered_at: existing_node.registered_at,
        metadata: req.metadata.unwrap_or(existing_node.metadata),
    };
    
    let grpc_req = UpdateNodeRequest {
        uuid,
        node: Some(updated_node),
    };
    
    match client.update_node(grpc_req).await {
        Ok(response) => {
            let resp = response.into_inner();
            Ok((StatusCode::OK, Json(json!({
                "success": resp.success,
                "message": resp.message
            }))))
        }
        Err(e) => {
            warn!("gRPC error during node update: {}", e);
            Err(StatusCode::INTERNAL_SERVER_ERROR)
        }
    }
}

/// Delete a node / 删除节点
pub async fn delete_node(
    State(state): State<GatewayState>,
    Path(uuid): Path<String>,
) -> Result<(StatusCode, Json<serde_json::Value>), StatusCode> {
    let mut client = state.node_client.clone();
    
    let grpc_req = DeleteNodeRequest { uuid };
    
    match client.delete_node(grpc_req).await {
        Ok(response) => {
            let resp = response.into_inner();
            Ok((StatusCode::OK, Json(json!({
                "success": resp.success,
                "message": resp.message
            }))))
        }
        Err(e) => {
            warn!("gRPC error during node deletion: {}", e);
            // Handle specific gRPC error codes / 处理特定的gRPC错误码
            if e.code() == tonic::Code::NotFound {
                Ok((StatusCode::NOT_FOUND, Json(json!({
                    "success": false,
                    "error": "Node not found"
                }))))
            } else {
                Err(StatusCode::INTERNAL_SERVER_ERROR)
            }
        }
    }
}

/// Send heartbeat for a node / 为节点发送心跳
pub async fn heartbeat(
    State(state): State<GatewayState>,
    Path(uuid): Path<String>,
    Json(req): Json<HttpHeartbeatRequest>,
) -> Result<(StatusCode, Json<serde_json::Value>), StatusCode> {
    let mut client = state.node_client.clone();
    
    let grpc_req = HeartbeatRequest {
        uuid,
        timestamp: chrono::Utc::now().timestamp(),
        health_info: req.health_info.unwrap_or_default(),
    };
    
    match client.heartbeat(grpc_req).await {
        Ok(response) => {
            let resp = response.into_inner();
            Ok((StatusCode::OK, Json(json!({
                "success": resp.success,
                "message": resp.message
            }))))
        }
        Err(e) => {
            warn!("gRPC error during heartbeat: {}", e);
            // Handle specific gRPC error codes / 处理特定的gRPC错误码
            if e.code() == tonic::Code::NotFound {
                Ok((StatusCode::NOT_FOUND, Json(json!({
                    "success": false,
                    "error": "Node not found"
                }))))
            } else {
                Err(StatusCode::INTERNAL_SERVER_ERROR)
            }
        }
    }
}