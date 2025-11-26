//! SPEARlet main entry point
//! SPEARlet 主入口点

use clap::Parser;
use spear_next::spearlet::config::{CliArgs, SpearletConfig};
use spear_next::spearlet::registration::RegistrationService;
use spear_next::spearlet::grpc_server::GrpcServer;
use spear_next::spearlet::http_gateway::HttpGateway;
use spear_next::config::init_tracing;

use std::net::SocketAddr;
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
    init_tracing(&config.logging.to_logging_config()).unwrap();
    
    tracing::info!("Starting SPEARlet with args: {}", log_args);
    
    tracing::info!("SPEARlet starting with:");
    tracing::info!("  - gRPC server on: {}", config.grpc.addr);
    tracing::info!("  - HTTP gateway on: {}", config.http.server.addr);
    tracing::info!("  - SMS service at: {}", config.sms_addr);
    tracing::info!("  - Node Name: {}", config.node_name);
    tracing::info!("  - Storage backend: {:?}", config.storage.backend);
    tracing::info!("  - Auto register: {}", config.auto_register);
    
    // Initialize gRPC server / 初始化gRPC服务器
    let grpc_server = GrpcServer::new(config.clone()).await?;
    // Shutdown channels / 关闭通道
    let (shutdown_tx_grpc, shutdown_rx_grpc) = tokio::sync::oneshot::channel::<()>();
    
    // Create health service for HTTP gateway / 为HTTP网关创建健康服务
    let health_service = spear_next::spearlet::grpc_server::HealthService::new(
        grpc_server.get_object_service(),
        grpc_server.get_function_service(),
    );
    
    let grpc_handle = tokio::spawn(async move {
        if let Err(e) = grpc_server.start_with_shutdown(async move { let _ = shutdown_rx_grpc.await; }).await {
            tracing::error!("gRPC server error: {}", e);
        }
    });
    
    // Initialize HTTP gateway / 初始化HTTP网关
    let http_gateway = HttpGateway::new(config.clone(), Arc::new(health_service));
    let (shutdown_tx_http, shutdown_rx_http) = tokio::sync::oneshot::channel::<()>();
    let http_handle = tokio::spawn(async move {
        if let Err(e) = http_gateway.start_with_shutdown(async move { let _ = shutdown_rx_http.await; }).await {
            tracing::error!("HTTP gateway error: {}", e);
        }
    });
    
    // CLI/env-gated SMS registration & heartbeat / 通过CLI或环境变量控制注册与心跳
    let connect_requested = args.sms_addr.is_some()
        || std::env::var("SPEARLET_SMS_ADDR").ok().map(|v| !v.is_empty()).unwrap_or(false);
    if connect_requested {
        let registration_service = RegistrationService::new(config.clone());
        if let Err(e) = registration_service.start().await {
            tracing::error!("Registration service start failed: {}", e);
            return Err(Box::<dyn std::error::Error + Send + Sync>::from(e));
        }
        tracing::info!("Registration service started (heartbeat every {}s)", config.heartbeat_interval);
    }
    
    // Wait for shutdown signal / 等待关闭信号
    tokio::signal::ctrl_c().await?;
    tracing::info!("SPEARlet shutting down");
    // Graceful shutdown / 优雅关闭
    let _ = shutdown_tx_grpc.send(());
    let _ = shutdown_tx_http.send(());
    // Wait tasks to finish / 等待任务结束
    let _ = grpc_handle.await;
    let _ = http_handle.await;
    
    Ok(())
}
