//! SPEARlet main entry point
//! SPEARlet 主入口点

use clap::Parser;
use spear_next::config::init_tracing;
use spear_next::spearlet::backend_reporter::BackendReporterService;
use spear_next::spearlet::config::CliArgs;
use spear_next::spearlet::grpc_server::GrpcServer;
use spear_next::spearlet::http_gateway::HttpGateway;
use spear_next::spearlet::mcp::registry_sync::global_mcp_registry_sync;
use spear_next::spearlet::ollama_discovery::maybe_import_ollama_serving_models;
use spear_next::spearlet::registration::RegistrationService;

use std::sync::Arc;

fn main() -> Result<(), Box<dyn std::error::Error + Send + Sync>> {
    let args = CliArgs::parse();
    let log_args = format!("{:?}", args);

    let app_cfg = spear_next::spearlet::config::AppConfig::load_with_cli(&args)?;
    let spearlet_cfg = app_cfg.spearlet;

    init_tracing(&spearlet_cfg.logging.to_logging_config()).unwrap();

    let max_blocking_threads = spearlet_cfg.max_blocking_threads.max(1);
    let runtime = tokio::runtime::Builder::new_multi_thread()
        .enable_all()
        .max_blocking_threads(max_blocking_threads)
        .build()?;

    runtime.block_on(run(args, log_args, spearlet_cfg))
}

async fn run(
    args: CliArgs,
    log_args: String,
    mut spearlet_cfg: spear_next::spearlet::config::SpearletConfig,
) -> Result<(), Box<dyn std::error::Error + Send + Sync>> {
    if spearlet_cfg.llm.discovery.ollama.enabled {
        match maybe_import_ollama_serving_models(&mut spearlet_cfg).await {
            Ok(n) => {
                if n > 0 {
                    tracing::info!(imported = n, "Imported Ollama serving models");
                }
            }
            Err(e) => {
                tracing::warn!(error = %e, "Ollama model import failed");
            }
        }
    }

    let config = Arc::new(spearlet_cfg);

    tracing::info!("Starting SPEARlet with args: {}", log_args);

    tracing::info!("SPEARlet starting with:");
    tracing::info!("  - gRPC server on: {}", config.grpc.addr);
    tracing::info!("  - HTTP gateway on: {}", config.http.server.addr);
    tracing::info!("  - SMS gRPC target at: {}", config.sms_grpc_addr);
    tracing::info!("  - Node Name: {}", config.node_name);
    tracing::info!("  - Storage backend: {:?}", config.storage.backend);
    tracing::info!("  - Auto register: {}", config.auto_register);

    global_mcp_registry_sync(config.clone());

    let grpc_server = GrpcServer::new(config.clone()).await?;
    let (shutdown_tx_grpc, shutdown_rx_grpc) = tokio::sync::oneshot::channel::<()>();

    let object_service = grpc_server.get_object_service();
    let function_service = grpc_server.get_function_service();
    let health_service = spear_next::spearlet::grpc_server::HealthService::new(
        object_service,
        function_service.clone(),
    );

    let grpc_handle = tokio::spawn(async move {
        if let Err(e) = grpc_server
            .start_with_shutdown(async move {
                let _ = shutdown_rx_grpc.await;
            })
            .await
        {
            tracing::error!("gRPC server error: {}", e);
        }
    });

    let http_gateway = HttpGateway::new(config.clone(), Arc::new(health_service));
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

    let connect_requested = config.auto_register
        || args.sms_grpc_addr.is_some()
        || std::env::var("SPEARLET_SMS_GRPC_ADDR")
            .ok()
            .map(|v| !v.is_empty())
            .unwrap_or(false);
    if connect_requested {
        let registration_service = RegistrationService::new(config.clone());
        if let Err(e) = registration_service.start().await {
            tracing::error!("Registration service start failed: {}", e);
            return Err(e);
        }
        tracing::info!(
            "Registration service started (heartbeat every {}s)",
            config.heartbeat_interval
        );
        let execution_manager = function_service.get_execution_manager();
        let subscriber = spear_next::spearlet::task_events::TaskEventSubscriber::new(
            config.clone(),
            execution_manager,
        );
        subscriber.start().await;

        let backend_reporter = BackendReporterService::new(config.clone());
        backend_reporter.start();
    }

    tokio::signal::ctrl_c().await?;
    tracing::info!("SPEARlet shutting down");
    let _ = shutdown_tx_grpc.send(());
    let _ = shutdown_tx_http.send(());
    let _ = grpc_handle.await;
    let _ = http_handle.await;

    tracing::info!("SPEARlet shutdown complete");
    Ok(())
}
