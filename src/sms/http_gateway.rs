//! HTTP gateway implementation for SMS (SPEAR Metadata Server)
//! SMS（SPEAR元数据服务器）的HTTP网关实现

use anyhow::Result;
use std::net::SocketAddr;

use tracing::{error, info};

use super::gateway::{create_gateway_router, GatewayState};
use crate::proto::sms::{
    mcp_registry_service_client::McpRegistryServiceClient,
    node_service_client::NodeServiceClient, placement_service_client::PlacementServiceClient,
    task_service_client::TaskServiceClient,
};
use tokio_util::sync::CancellationToken;

/// SMS HTTP gateway / SMS HTTP网关
pub struct HttpGateway {
    addr: SocketAddr,
    grpc_addr: SocketAddr,
    enable_swagger: bool,
    max_upload_bytes: usize,
}

impl HttpGateway {
    /// Create a new HTTP gateway / 创建新的HTTP网关
    pub fn new(
        addr: SocketAddr,
        grpc_addr: SocketAddr,
        enable_swagger: bool,
        max_upload_bytes: usize,
    ) -> Self {
        Self {
            addr,
            grpc_addr,
            enable_swagger,
            max_upload_bytes,
        }
    }

    /// Get the HTTP address / 获取HTTP地址
    pub fn addr(&self) -> SocketAddr {
        self.addr
    }

    /// Get the gRPC address / 获取gRPC地址
    pub fn grpc_addr(&self) -> SocketAddr {
        self.grpc_addr
    }

    /// Check if Swagger is enabled / 检查是否启用Swagger
    pub fn enable_swagger(&self) -> bool {
        self.enable_swagger
    }

    /// Start the HTTP gateway / 启动HTTP网关
    pub async fn start(self) -> Result<()> {
        let (listener, app) = self.prepare().await?;
        if let Err(e) = axum::serve(listener, app).await {
            error!("SMS HTTP gateway error: {}", e);
            return Err(e.into());
        }
        Ok(())
    }

    /// Start HTTP gateway with shutdown signal / 使用关闭信号启动HTTP网关
    pub async fn start_with_shutdown<F>(self, shutdown: F) -> Result<()>
    where
        F: std::future::Future<Output = ()> + Send + 'static,
    {
        let (listener, app) = self.prepare().await?;
        if let Err(e) = axum::serve(listener, app)
            .with_graceful_shutdown(shutdown)
            .await
        {
            error!("SMS HTTP gateway error: {}", e);
            return Err(e.into());
        }
        Ok(())
    }

    async fn prepare(self) -> Result<(tokio::net::TcpListener, axum::Router)> {
        info!("Starting SMS HTTP gateway on {}", self.addr);
        info!("Connecting to gRPC server at {}", self.grpc_addr);

        let grpc_url = format!("http://{}", self.grpc_addr);
        let channel = tonic::transport::Channel::from_shared(grpc_url)
            .expect("Invalid gRPC URL")
            .connect_lazy();
        let node_client = NodeServiceClient::new(channel.clone());
        let task_client = TaskServiceClient::new(channel.clone());
        let placement_client = PlacementServiceClient::new(channel.clone());
        let mcp_registry_client = McpRegistryServiceClient::new(channel);

        let state = GatewayState {
            node_client,
            task_client,
            placement_client,
            mcp_registry_client,
            cancel_token: CancellationToken::new(),
            max_upload_bytes: self.max_upload_bytes,
        };
        let app = create_gateway_router(state);

        info!("SMS HTTP gateway listening on {}", self.addr);
        info!("Swagger UI enabled: {}", self.enable_swagger);

        let listener = tokio::net::TcpListener::bind(self.addr).await?;
        Ok((listener, app))
    }
}
