//! SMS (SPEAR Metadata Server) main entry point
//! SMS (SPEAR元数据服务器) 主入口点

use clap::Parser;
use spear_next::config::init_tracing;
use spear_next::sms::config::{CliArgs, SmsConfig};
use spear_next::sms::grpc_server::GrpcServer;
use spear_next::sms::http_gateway::HttpGateway;
use spear_next::sms::service::SmsServiceImpl;
use spear_next::sms::services::{NodeService, ResourceService, TaskService};
use spear_next::sms::web_admin::WebAdminServer;
use std::sync::Arc;
use tokio::sync::RwLock;

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
    )
    .await;

    if !config.mcp.dir.is_empty() {
        let loaded = sms_service.bootstrap_mcp_from_dir(&config.mcp.dir).await?;
        tracing::info!(loaded, dir = %config.mcp.dir, "MCP servers loaded from directory");
    }
    let sms_service_for_cleanup = sms_service.clone();

    // Initialize gRPC server / 初始化gRPC服务器
    let grpc_server = GrpcServer::new(config.grpc.addr, sms_service);
    let (shutdown_tx_grpc, shutdown_rx_grpc) = tokio::sync::oneshot::channel::<()>();
    let grpc_handle = tokio::spawn({
        async move {
            if let Err(e) = grpc_server
                .start_with_shutdown(async move {
                    let _ = shutdown_rx_grpc.await;
                })
                .await
            {
                tracing::error!("gRPC server error: {}", e);
            }
        }
    });

    // Initialize HTTP gateway / 初始化HTTP网关
    let http_gateway = HttpGateway::new(
        config.http.addr,
        config.grpc.addr,
        config.enable_swagger,
        config.max_upload_bytes as usize,
    );
    let (shutdown_tx_http, shutdown_rx_http) = tokio::sync::oneshot::channel::<()>();
    let http_handle = tokio::spawn(async move {
        if let Err(e) = http_gateway
            .start_with_shutdown(async move {
                let _ = shutdown_rx_http.await;
            })
            .await
        {
            tracing::error!("HTTP gateway error: {}", e);
        }
    });

    // Initialize Web Admin (optional) / 初始化Web管理页面（可选）
    let web_admin_enabled = config.enable_web_admin;
    let web_admin_addr = config.web_admin.addr;
    let web_admin_server = WebAdminServer::new(web_admin_addr, config.grpc.addr, web_admin_enabled);
    let (shutdown_tx_admin, shutdown_rx_admin) = tokio::sync::oneshot::channel::<()>();
    let admin_handle = tokio::spawn(async move {
        if let Err(e) = web_admin_server
            .start_with_shutdown(async move {
                let _ = shutdown_rx_admin.await;
            })
            .await
        {
            tracing::error!("Web Admin server error: {}", e);
        }
    });

    // Liveness cleanup task / 存活清理任务
    let cleanup_cfg = config.clone();
    tokio::spawn(async move {
        let interval_secs = cleanup_cfg.cleanup_interval;
        loop {
            tokio::time::sleep(std::time::Duration::from_secs(interval_secs)).await;
            let now = chrono::Utc::now().timestamp();
            let timeout = cleanup_cfg.heartbeat_timeout as i64;
            let node_service = sms_service_for_cleanup.node_service();
            let mut ns = node_service.write().await;
            let mut changed = 0u64;
            let nodes = ns.list_nodes().await.unwrap_or_default();
            for mut n in nodes {
                let stale = now - n.last_heartbeat > timeout;
                let new_status = if stale { "offline" } else { "online" };
                if n.status != new_status {
                    n.status = new_status.to_string();
                    let _ = ns.update_node(n).await;
                    changed += 1;
                }
            }
            if changed > 0 {
                tracing::debug!(
                    "Cleanup updated node statuses, changed={} (timeout={}s)",
                    changed,
                    timeout
                );
            }
        }
    });

    tracing::info!("SMS server started successfully");
    tracing::info!("gRPC server: http://{}", config.grpc.addr);
    tracing::info!("HTTP gateway: http://{}", config.http.addr);
    if config.enable_swagger {
        tracing::info!("Swagger UI: http://{}/swagger-ui", config.http.addr);
    }
    if web_admin_enabled {
        tracing::info!("Web Admin: http://{}", web_admin_addr);
    }

    tokio::signal::ctrl_c().await?;
    tracing::info!("SMS server shutting down");
    let _ = shutdown_tx_grpc.send(());
    let _ = shutdown_tx_http.send(());
    let _ = shutdown_tx_admin.send(());

    let timeout = std::time::Duration::from_secs(5);
    let deadline = tokio::time::Instant::now() + timeout;
    loop {
        if grpc_handle.is_finished() && http_handle.is_finished() && admin_handle.is_finished() {
            break;
        }
        if tokio::time::Instant::now() >= deadline {
            tracing::warn!("Shutdown timeout reached, aborting servers");
            grpc_handle.abort();
            http_handle.abort();
            admin_handle.abort();
            break;
        }
        tokio::time::sleep(std::time::Duration::from_millis(100)).await;
    }

    Ok(())
}
