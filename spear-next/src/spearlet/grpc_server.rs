//! gRPC server implementation for spearlet
//! spearlet的gRPC服务器实现

use std::net::SocketAddr;
use std::sync::Arc;
use tonic::transport::Server;

use tracing::{info, error};

use crate::spearlet::config::SpearletConfig;
use crate::spearlet::function_service::FunctionServiceImpl;
use crate::spearlet::object_service::ObjectServiceImpl;
use crate::proto::spearlet::object_service_server::ObjectServiceServer;
// use crate::proto::spearlet::function_service_server::FunctionService;
// TODO: Add FunctionServiceServer import when proto is regenerated
// TODO: 当proto重新生成时添加FunctionServiceServer导入
// use crate::proto::spearlet::function_service_server::FunctionServiceServer;

/// gRPC server for spearlet / spearlet的gRPC服务器
pub struct GrpcServer {
    /// Server configuration / 服务器配置
    config: Arc<SpearletConfig>,
    /// Object service implementation / 对象服务实现
    object_service: Arc<ObjectServiceImpl>,
    /// Function service implementation / 函数服务实现
    function_service: Arc<FunctionServiceImpl>,
}

impl GrpcServer {
    /// Create new gRPC server / 创建新的gRPC服务器
    pub async fn new(config: Arc<SpearletConfig>) -> Result<Self, Box<dyn std::error::Error + Send + Sync>> {
        let object_service = Arc::new(ObjectServiceImpl::new_with_memory(config.storage.max_object_size));
        let function_service = Arc::new(FunctionServiceImpl::new(config.clone()).await?);
        
        Ok(Self {
            config,
            object_service,
            function_service,
        })
    }

    /// Get object service reference / 获取对象服务引用
    pub fn get_object_service(&self) -> Arc<ObjectServiceImpl> {
        self.object_service.clone()
    }

    /// Get function service reference / 获取函数服务引用
    pub fn get_function_service(&self) -> Arc<FunctionServiceImpl> {
        self.function_service.clone()
    }

    /// Start gRPC server / 启动gRPC服务器
    pub async fn start(self) -> Result<(), Box<dyn std::error::Error + Send + Sync>> {
        let (addr, object_service) = self.prepare().await?;
        let server = Server::builder().add_service(object_service).serve(addr);

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

    /// Start gRPC server with shutdown signal / 使用关闭信号启动gRPC服务器
    pub async fn start_with_shutdown<F>(self, shutdown: F) -> Result<(), Box<dyn std::error::Error + Send + Sync>>
    where
        F: std::future::Future<Output = ()> + Send + 'static,
    {
        let (addr, object_service) = self.prepare().await?;
        let server = Server::builder().add_service(object_service).serve_with_shutdown(addr, shutdown);

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

    async fn prepare(self) -> Result<(
        SocketAddr,
        ObjectServiceServer<Arc<ObjectServiceImpl>>
    ), Box<dyn std::error::Error + Send + Sync>> {
        let addr: SocketAddr = self.config.grpc.addr;
        info!("Starting gRPC server on {}", addr);

        let object_service = ObjectServiceServer::new(self.object_service.clone())
            .max_decoding_message_size(self.config.storage.max_object_size as usize)
            .max_encoding_message_size(self.config.storage.max_object_size as usize);

        Ok((addr, object_service))
    }
}

/// Health service for monitoring / 用于监控的健康服务
pub struct HealthService {
    object_service: Arc<ObjectServiceImpl>,
    function_service: Arc<FunctionServiceImpl>,
}

impl HealthService {
    pub fn new(
        object_service: Arc<ObjectServiceImpl>,
        function_service: Arc<FunctionServiceImpl>,
    ) -> Self {
        Self { 
            object_service,
            function_service,
        }
    }

    /// Get current health status / 获取当前健康状态
    pub async fn get_health_status(&self) -> HealthStatus {
        let object_stats = self.object_service.get_stats().await;
        let function_stats = self.function_service.get_stats().await;
        HealthStatus {
            status: "healthy".to_string(),
            object_count: object_stats.object_count,
            total_object_size: object_stats.total_size,
            pinned_object_count: object_stats.pinned_count,
            task_count: function_stats.task_count,
            execution_count: function_stats.execution_count,
            running_executions: function_stats.running_executions,
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
    pub task_count: usize,
    pub execution_count: usize,
    pub running_executions: usize,
}
