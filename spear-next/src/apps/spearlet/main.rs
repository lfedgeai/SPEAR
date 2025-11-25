//! SPEARlet main entry point
//! SPEARlet 主入口点

use clap::Parser;
use spear_next::spearlet::config::{CliArgs, SpearletConfig, StorageConfig, LoggingConfig};
use spear_next::spearlet::registration::RegistrationService;
use spear_next::spearlet::grpc_server::GrpcServer;
use spear_next::spearlet::http_gateway::HttpGateway;
use spear_next::config::init_tracing;

use std::net::{IpAddr, Ipv4Addr, SocketAddr};
use std::sync::Arc;

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error + Send + Sync>> {
    // Parse command line arguments / 解析命令行参数
    let args = CliArgs::parse();
    
    // Store args values for logging before they are moved / 在参数被移动之前存储参数值用于日志记录
    let log_args = format!("{:?}", args);
    
    // Load configuration with home-first, then CLI override /
    // 先从主目录加载配置，其次使用命令行覆盖
    let app_cfg = spear_next::spearlet::config::AppConfig::load_with_cli(&args)?;
    let config = Arc::new(app_cfg.spearlet);

    // Initialize logging with configuration / 使用配置初始化日志
    init_tracing(&config.logging.to_common_config()).unwrap();
    
    tracing::info!("Starting SPEARlet with args: {}", log_args);
    
    tracing::info!("SPEARlet starting with:");
    tracing::info!(
        "  - gRPC server on: {}:{}",
        config.grpc.address,
        config.grpc.port
    );
    tracing::info!(
        "  - HTTP gateway on: {}:{}",
        config.http.address,
        config.http.port
    );
    tracing::info!("  - SMS service at: {}", config.sms_addr);
    tracing::info!("  - Node ID: {}", config.node_id);
    tracing::info!("  - Storage backend: {:?}", config.storage.backend);
    tracing::info!("  - Auto register: {}", config.auto_register);
    
    // Initialize gRPC server / 初始化gRPC服务器
    let grpc_server = GrpcServer::new(config.clone()).await?;
    
    // Create health service for HTTP gateway / 为HTTP网关创建健康服务
    let health_service = spear_next::spearlet::grpc_server::HealthService::new(
        grpc_server.get_object_service(),
        grpc_server.get_function_service(),
    );
    
    let grpc_handle = tokio::spawn(async move {
        if let Err(e) = grpc_server.start().await {
            tracing::error!("gRPC server error: {}", e);
        }
    });
    
    // Initialize HTTP gateway / 初始化HTTP网关
    let http_gateway = HttpGateway::new(config.clone(), Arc::new(health_service));
    let http_handle = tokio::spawn(async move {
        if let Err(e) = http_gateway.start().await {
            tracing::error!("HTTP gateway error: {}", e);
        }
    });
    
    // Register with SMS if enabled / 如果启用，向SMS注册
    if config.auto_register {
        let registration_service = RegistrationService::new(config.clone());
        if let Err(e) = registration_service.force_register().await {
            tracing::warn!("Failed to register with SMS: {}", e);
        } else {
            tracing::info!("Successfully registered with SMS");
        }
    }
    
    // Wait for servers to complete / 等待服务器完成
    let _ = tokio::join!(grpc_handle, http_handle);
    
    Ok(())
}
