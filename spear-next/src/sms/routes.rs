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
    // Node handlers / 节点处理器
    register_node, list_nodes, get_node, update_node, delete_node, heartbeat,
    // Resource handlers / 资源处理器
    update_node_resource, get_node_resource, list_node_resources, get_node_with_resource,
    // Task handlers / 任务处理器
    register_task, list_tasks, get_task, unregister_task,
    // Documentation handlers / 文档处理器
    openapi_spec, swagger_ui, swagger_ui_assets,
    // Health handler / 健康检查处理器
    health_check,
};

/// Create HTTP routes / 创建HTTP路由
pub fn create_routes(state: GatewayState) -> Router {
    Router::new()
        // Node management endpoints / 节点管理端点
        .route("/api/v1/nodes", post(register_node))
        .route("/api/v1/nodes", get(list_nodes))
        .route("/api/v1/nodes/:uuid", get(get_node))
        .route("/api/v1/nodes/:uuid", put(update_node))
        .route("/api/v1/nodes/:uuid", delete(delete_node))
        .route("/api/v1/nodes/:uuid/heartbeat", post(heartbeat))
        
        // Node resource management endpoints / 节点资源管理端点
        .route("/api/v1/nodes/:uuid/resource", put(update_node_resource))
        .route("/api/v1/nodes/:uuid/resource", get(get_node_resource))
        .route("/api/v1/resources", get(list_node_resources))
        .route("/api/v1/nodes/:uuid/with-resource", get(get_node_with_resource))
        
        // Task management endpoints / 任务管理端点
        .route("/api/v1/tasks", post(register_task))
        .route("/api/v1/tasks", get(list_tasks))
        .route("/api/v1/tasks/:task_id", get(get_task))
        .route("/api/v1/tasks/:task_id", delete(unregister_task))
        
        // API documentation endpoints / API文档端点
        .route("/api/openapi.json", get(openapi_spec))
        .route("/swagger-ui/", get(swagger_ui))
        .route("/swagger-ui/*file", get(swagger_ui_assets))
        
        // Health check endpoint / 健康检查端点
        .route("/health", get(health_check))
        
        .with_state(state)
}