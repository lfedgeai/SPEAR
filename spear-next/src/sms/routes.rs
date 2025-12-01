//! HTTP routes for SPEAR Metadata Server
//! SPEAR元数据服务器的HTTP路由
//!
//! This module defines all HTTP routes and their mappings to handlers
//! 此模块定义所有HTTP路由及其到处理器的映射

use axum::{
    routing::{get, post, put, delete},
    Router,
};

use super::gateway::GatewayState;
use super::handlers::{
    register_node, list_nodes, get_node, update_node, delete_node, heartbeat,
    update_node_resource, get_node_resource, list_node_resources, get_node_with_resource,
    register_task, list_tasks, get_task, unregister_task,
    openapi_spec, swagger_ui, swagger_ui_assets,
    health_check,
    upload_file, download_file, delete_file, get_file_meta, presign_upload, list_files,
};

/// Create HTTP routes / 创建HTTP路由
pub(crate) fn create_routes(state: GatewayState) -> Router {
    Router::new()
        // Node management endpoints / 节点管理端点
        .route("/api/v1/nodes", post(register_node))
        .route("/api/v1/nodes", get(list_nodes))
        .route("/api/v1/nodes/{uuid}", get(get_node))
        .route("/api/v1/nodes/{uuid}", put(update_node))
        .route("/api/v1/nodes/{uuid}", delete(delete_node))
        .route("/api/v1/nodes/{uuid}/heartbeat", post(heartbeat))
        
        // Node resource management endpoints / 节点资源管理端点
        .route("/api/v1/nodes/{uuid}/resource", put(update_node_resource))
        .route("/api/v1/nodes/{uuid}/resource", get(get_node_resource))
        .route("/api/v1/resources", get(list_node_resources))
        .route("/api/v1/nodes/{uuid}/with-resource", get(get_node_with_resource))
        
        // Task management endpoints / 任务管理端点
        .route("/api/v1/tasks", post(register_task))
        .route("/api/v1/tasks", get(list_tasks))
        .route("/api/v1/tasks/{task_id}", get(get_task))
        .route("/api/v1/tasks/{task_id}", delete(unregister_task))
        
        // API documentation endpoints / API文档端点
        .route("/api/openapi.json", get(openapi_spec))
        .route("/swagger-ui/", get(swagger_ui))
        .route("/swagger-ui/{*file}", get(swagger_ui_assets))
        
        // Health check endpoint / 健康检查端点
        .route("/health", get(health_check))
        .route("/api/v1/files/presign-upload", post(presign_upload))
        .route("/api/v1/files", get(list_files))
        .route("/api/v1/files", post(upload_file))
        .route("/api/v1/files/{id}", get(download_file))
        .route("/api/v1/files/{id}", delete(delete_file))
        .route("/api/v1/files/{id}/meta", get(get_file_meta))
        
        .with_state(state)
}
