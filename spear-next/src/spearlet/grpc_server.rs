//! gRPC server implementation for spearlet
//! spearlet的gRPC服务器实现

use std::net::SocketAddr;
use std::sync::Arc;
use tonic::transport::Server;

use tracing::{info, error};

use crate::spearlet::config::SpearletConfig;
use crate::spearlet::object_service::ObjectServiceImpl;
use crate::proto::spearlet::object_service_server::ObjectServiceServer;

/// gRPC server for spearlet / spearlet的gRPC服务器
pub struct GrpcServer {
    /// Server configuration / 服务器配置
    config: Arc<SpearletConfig>,
    /// Object service implementation / 对象服务实现
    object_service: Arc<ObjectServiceImpl>,
}

impl GrpcServer {
    /// Create new gRPC server / 创建新的gRPC服务器
    pub fn new(config: Arc<SpearletConfig>) -> Self {
        let object_service = Arc::new(ObjectServiceImpl::new_with_memory(config.storage.max_object_size));
        
        Self {
            config,
            object_service,
        }
    }

    /// Get object service reference / 获取对象服务引用
    pub fn get_object_service(&self) -> Arc<ObjectServiceImpl> {
        self.object_service.clone()
    }

    /// Start gRPC server / 启动gRPC服务器
    pub async fn start(self) -> Result<(), Box<dyn std::error::Error + Send + Sync>> {
        let addr: SocketAddr = format!("{}:{}", self.config.grpc.address, self.config.grpc.port)
            .parse()
            .map_err(|e| format!("Invalid gRPC address: {}", e))?;
        
        info!("Starting gRPC server on {}", addr);
        
        // Create gRPC service / 创建gRPC服务
        let object_service = ObjectServiceServer::new(self.object_service.clone())
            .max_decoding_message_size(self.config.storage.max_object_size as usize)
            .max_encoding_message_size(self.config.storage.max_object_size as usize);

        // Build and start server / 构建并启动服务器
        let server = Server::builder()
            .add_service(object_service)
            .serve(addr);

        match server.await {
            Ok(_) => {
                info!("gRPC server stopped gracefully");
                Ok(())
            }
            Err(e) => {
                error!("gRPC server error: {}", e);
                Err(Box::new(e))
            }
        }
    }
}

/// Health service for monitoring / 用于监控的健康服务
pub struct HealthService {
    object_service: Arc<ObjectServiceImpl>,
}

impl HealthService {
    pub fn new(object_service: Arc<ObjectServiceImpl>) -> Self {
        Self { object_service }
    }

    /// Get current health status / 获取当前健康状态
    pub async fn get_health_status(&self) -> HealthStatus {
        let stats = self.object_service.get_stats().await;
        HealthStatus {
            status: "healthy".to_string(),
            object_count: stats.object_count,
            total_object_size: stats.total_size,
            pinned_object_count: stats.pinned_count,
        }
    }
}

/// Health status information / 健康状态信息
#[derive(Debug, Clone)]
pub struct HealthStatus {
    pub status: String,
    pub object_count: usize,
    pub total_object_size: u64,
    pub pinned_object_count: usize,
}