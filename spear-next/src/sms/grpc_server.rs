//! gRPC server implementation for SMS (SPEAR Metadata Server)
//! SMS（SPEAR元数据服务器）的gRPC服务器实现

use std::net::SocketAddr;
use anyhow::Result;
use tonic::transport::Server;
use tracing::{info, error};

use crate::proto::sms::{
    node_service_server::NodeServiceServer,
    task_service_server::TaskServiceServer,
};

use crate::sms::service::SmsServiceImpl;
/// SMS gRPC server / SMS gRPC服务器
pub struct GrpcServer {
    addr: SocketAddr,
    sms_service: SmsServiceImpl,
}

impl GrpcServer {
    /// Create a new gRPC server / 创建新的gRPC服务器
    pub fn new(addr: SocketAddr, sms_service: SmsServiceImpl) -> Self {
        Self {
            addr,
            sms_service,
        }
    }

    /// Start the gRPC server / 启动gRPC服务器
    pub async fn start(self) -> Result<()> {
        info!("Starting SMS gRPC server on {}", self.addr);

        // Build the server with all services / 构建包含所有服务的服务器
        // SmsServiceImpl implements NodeService and TaskService
        // SmsServiceImpl实现了NodeService和TaskService
        let server = Server::builder()
            .add_service(NodeServiceServer::new(self.sms_service.clone()))
            .add_service(TaskServiceServer::new(self.sms_service))
            .serve(self.addr);

        info!("SMS gRPC server listening on {}", self.addr);

        // Start the server / 启动服务器
        if let Err(e) = server.await {
            error!("SMS gRPC server error: {}", e);
            return Err(e.into());
        }

        Ok(())
    }
}