//! SMS (SPEAR Metadata Server) main entry point
//! SMS (SPEAR元数据服务器) 主入口点

use clap::Parser;
use spear_next::sms::config::{CliArgs, SmsConfig, DatabaseConfig};
use spear_next::config::base::{ServerConfig, LogConfig};
use spear_next::sms::grpc_server::GrpcServer;
use spear_next::sms::http_gateway::HttpGateway;
use spear_next::sms::services::{NodeService, ResourceService, TaskService};
use spear_next::sms::service::SmsServiceImpl;
use spear_next::config::init_tracing;
use tokio::sync::RwLock;
use std::net::{IpAddr, Ipv4Addr, SocketAddr};
use std::sync::Arc;

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    // Parse command line arguments / 解析命令行参数
    let args = CliArgs::parse();
    
    // Store args values for logging before they are moved / 在参数被移动之前存储参数值用于日志记录
    let log_args = format!("{:?}", args);
    
    // Load configuration with home-first, then CLI override /
    // 先从主目录加载配置，其次使用命令行覆盖
    let cfg = SmsConfig::load_with_cli(&args)?;
    let config = Arc::new(cfg);

    // Initialize logging with configuration / 使用配置初始化日志
    init_tracing(&config.log.to_logging_config()).unwrap();
    
    tracing::info!("Starting SMS server with args: {}", log_args);
    
    tracing::info!("SMS server starting with:");
    tracing::info!("  - gRPC server on: {}", config.grpc.addr);
    tracing::info!("  - HTTP gateway on: {}", config.http.addr);
    tracing::info!("  - Database type: {:?}", config.database.db_type);
    tracing::info!("  - Database path: {:?}", config.database.path);
    tracing::info!("  - Enable Swagger: {}", config.enable_swagger);
    
    // Initialize services / 初始化服务
    let node_service = NodeService::new();
    let resource_service = ResourceService::new();
    let _task_service = TaskService::new();
    
    // Create SMS service collection / 创建SMS服务集合
    let sms_service = SmsServiceImpl::new(
        Arc::new(RwLock::new(node_service)),
        Arc::new(resource_service),
        config.clone(),
    ).await;
    
    // Initialize gRPC server / 初始化gRPC服务器
    let grpc_server = GrpcServer::new(config.grpc.addr, sms_service);
    let grpc_handle = tokio::spawn({
        async move {
            if let Err(e) = grpc_server.start().await {
                tracing::error!("gRPC server error: {}", e);
            }
        }
    });
    
    // Initialize HTTP gateway / 初始化HTTP网关
    let http_gateway = HttpGateway::new(config.http.addr, config.grpc.addr, config.enable_swagger);
    let http_handle = tokio::spawn(async move {
        if let Err(e) = http_gateway.start().await {
            tracing::error!("HTTP gateway error: {}", e);
        }
    });
    
    tracing::info!("SMS server started successfully");
    tracing::info!("gRPC server: http://{}", config.grpc.addr);
    tracing::info!("HTTP gateway: http://{}", config.http.addr);
    if config.enable_swagger {
        tracing::info!("Swagger UI: http://{}/swagger-ui", config.http.addr);
    }
    
    // Wait for shutdown signal / 等待关闭信号
    tokio::signal::ctrl_c().await?;
    tracing::info!("SMS server shutting down");
    
    // Graceful shutdown / 优雅关闭
    grpc_handle.abort();
    http_handle.abort();
    
    Ok(())
}
