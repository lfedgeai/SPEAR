//! HTTP gateway implementation for SMS (SPEAR Metadata Server)
//! SMS（SPEAR元数据服务器）的HTTP网关实现

use std::net::SocketAddr;
use anyhow::Result;

use tracing::{info, error};

use crate::proto::sms::{
    node_service_client::NodeServiceClient,
    task_service_client::TaskServiceClient,
};
use super::{gateway::{create_gateway_router, GatewayState}};



/// SMS HTTP gateway / SMS HTTP网关
pub struct HttpGateway {
    addr: SocketAddr,
    grpc_addr: SocketAddr,
    enable_swagger: bool,
}

impl HttpGateway {
    /// Create a new HTTP gateway / 创建新的HTTP网关
    pub fn new(addr: SocketAddr, grpc_addr: SocketAddr, enable_swagger: bool) -> Self {
        Self {
            addr,
            grpc_addr,
            enable_swagger,
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
        info!("Starting SMS HTTP gateway on {}", self.addr);
        info!("Connecting to gRPC server at {}", self.grpc_addr);

        // Connect to gRPC server / 连接到gRPC服务器
        let grpc_url = format!("http://{}", self.grpc_addr);
        
        let node_client = NodeServiceClient::connect(grpc_url.clone()).await
            .map_err(|e| {
                error!("Failed to connect to gRPC server for node service: {}", e);
                e
            })?;

        let task_client = TaskServiceClient::connect(grpc_url).await
            .map_err(|e| {
                error!("Failed to connect to gRPC server for task service: {}", e);
                e
            })?;

        // Create gateway state / 创建网关状态
        let state = GatewayState {
            node_client,
            task_client,
        };

        // Create router / 创建路由器
        let app = create_gateway_router(state);

        info!("SMS HTTP gateway listening on {}", self.addr);
        info!("Swagger UI enabled: {}", self.enable_swagger);

        // Start the server / 启动服务器
        let listener = tokio::net::TcpListener::bind(self.addr).await?;
        if let Err(e) = axum::serve(listener, app).await {
            error!("SMS HTTP gateway error: {}", e);
            return Err(e.into());
        }

        Ok(())
    }
}