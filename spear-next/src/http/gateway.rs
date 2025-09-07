//! HTTP gateway for SPEAR Metadata Server gRPC service
//! SPEAR元数据服务器gRPC服务的HTTP网关

use axum::Router;
use rust_embed::RustEmbed;


use crate::proto::sms::{
    node_service_client::NodeServiceClient,
    task_service_client::TaskServiceClient,
};
use crate::http::routes::create_routes;

/// Embedded static files for Swagger UI / Swagger UI的嵌入式静态文件
#[derive(RustEmbed)]
#[folder = "static/"]
struct StaticFiles;

/// HTTP gateway state / HTTP网关状态
#[derive(Clone)]
pub struct GatewayState {
    pub node_client: NodeServiceClient<tonic::transport::Channel>,
    pub task_client: TaskServiceClient<tonic::transport::Channel>,
}

/// Create HTTP gateway router / 创建HTTP网关路由器
pub fn create_gateway_router(state: GatewayState) -> Router {
    // Use the centralized route creation function / 使用集中的路由创建函数
    create_routes(state)
}