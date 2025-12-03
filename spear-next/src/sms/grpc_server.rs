//! gRPC server implementation for SMS (SPEAR Metadata Server)
//! SMS（SPEAR元数据服务器）的gRPC服务器实现

use anyhow::Result;
use std::net::SocketAddr;
use tonic::transport::Server;
use tracing::{error, info};

use crate::proto::sms::{
    node_service_server::NodeServiceServer, task_service_server::TaskServiceServer,
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
        Self { addr, sms_service }
    }

    /// Start the gRPC server / 启动gRPC服务器
    pub async fn start(self) -> Result<()> {
        let (addr, sms_service) = self.prepare();
        let server = Server::builder()
            .add_service(NodeServiceServer::new(sms_service.clone()))
            .add_service(TaskServiceServer::new(sms_service))
            .serve(addr);

        info!("SMS gRPC server listening on {}", addr);

        if let Err(e) = server.await {
            error!("SMS gRPC server error: {}", e);
            return Err(e.into());
        }

        Ok(())
    }

    /// Start the gRPC server with shutdown signal / 使用关闭信号启动gRPC服务器
    pub async fn start_with_shutdown<F>(self, shutdown: F) -> Result<()>
    where
        F: std::future::Future<Output = ()> + Send + 'static,
    {
        let (addr, sms_service) = self.prepare();
        let server = Server::builder()
            .add_service(NodeServiceServer::new(sms_service.clone()))
            .add_service(TaskServiceServer::new(sms_service))
            .serve_with_shutdown(addr, shutdown);

        info!("SMS gRPC server listening on {}", addr);

        if let Err(e) = server.await {
            error!("SMS gRPC server error: {}", e);
            return Err(e.into());
        }

        Ok(())
    }

    fn prepare(&self) -> (SocketAddr, SmsServiceImpl) {
        info!("Starting SMS gRPC server on {}", self.addr);
        (self.addr, self.sms_service.clone())
    }
}
