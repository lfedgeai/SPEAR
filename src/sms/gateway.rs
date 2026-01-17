//! HTTP gateway for SPEAR Metadata Server gRPC service
//! SPEAR元数据服务器gRPC服务的HTTP网关

use axum::Router;
use rust_embed::RustEmbed;
use tokio_util::sync::CancellationToken;
use tower_http::cors::CorsLayer;

use super::routes::create_routes;
use crate::proto::sms::{
    backend_registry_service_client::BackendRegistryServiceClient,
    mcp_registry_service_client::McpRegistryServiceClient, node_service_client::NodeServiceClient,
    placement_service_client::PlacementServiceClient, task_service_client::TaskServiceClient,
};

/// Embedded static files for Swagger UI / Swagger UI的嵌入式静态文件
#[derive(RustEmbed)]
#[folder = "static/"]
#[allow(dead_code)]
struct StaticFiles;

/// HTTP gateway state / HTTP网关状态
#[derive(Clone, Debug)]
pub struct GatewayState {
    pub node_client: NodeServiceClient<tonic::transport::Channel>,
    pub task_client: TaskServiceClient<tonic::transport::Channel>,
    pub placement_client: PlacementServiceClient<tonic::transport::Channel>,
    pub mcp_registry_client: McpRegistryServiceClient<tonic::transport::Channel>,
    pub backend_registry_client: BackendRegistryServiceClient<tonic::transport::Channel>,
    pub cancel_token: CancellationToken,
    pub max_upload_bytes: usize,
}

/// Create HTTP gateway router / 创建HTTP网关路由器
pub fn create_gateway_router(state: GatewayState) -> Router {
    // Use the centralized route creation function / 使用集中的路由创建函数
    create_routes(state).layer(CorsLayer::permissive()) // Add CORS support / 添加CORS支持
}
