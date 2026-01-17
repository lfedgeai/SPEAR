use std::collections::HashMap;
use std::sync::Arc;
use std::time::Duration;

use tokio::time::{interval, timeout};
use tokio_util::sync::CancellationToken;
use tonic::transport::Channel;
use tracing::{debug, warn};

use crate::proto::sms::backend_registry_service_client::BackendRegistryServiceClient;
use crate::proto::sms::{
    BackendInfo, BackendStatus, NodeBackendSnapshot, ReportNodeBackendsRequest,
};
use crate::spearlet::config::{LlmBackendConfig, SpearletConfig};

#[derive(Debug)]
pub struct BackendReporterService {
    config: Arc<SpearletConfig>,
    cancel: CancellationToken,
}

impl BackendReporterService {
    pub fn new(config: Arc<SpearletConfig>) -> Self {
        Self {
            config,
            cancel: CancellationToken::new(),
        }
    }

    pub fn shutdown(&self) {
        self.cancel.cancel();
    }

    pub fn start(&self) {
        let config = self.config.clone();
        let cancel = self.cancel.clone();
        if let Ok(handle) = tokio::runtime::Handle::try_current() {
            handle.spawn(async move {
                report_loop(config, cancel).await;
            });
            return;
        }

        std::thread::spawn(move || {
            let rt = tokio::runtime::Builder::new_current_thread()
                .enable_all()
                .build();
            if let Ok(rt) = rt {
                rt.block_on(async move {
                    report_loop(config, cancel).await;
                });
            }
        });
    }
}

async fn connect_sms(
    config: &SpearletConfig,
) -> Result<BackendRegistryServiceClient<Channel>, tonic::Status> {
    let sms_url = format!("http://{}", config.sms_grpc_addr);
    let connect_fut = BackendRegistryServiceClient::connect(sms_url);
    let client = timeout(
        Duration::from_millis(config.sms_connect_timeout_ms),
        connect_fut,
    )
    .await
    .map_err(|_| tonic::Status::deadline_exceeded("connect sms timeout"))?
    .map_err(|e| tonic::Status::unavailable(format!("connect sms failed: {}", e)))?;
    Ok(client)
}

fn backend_requires_api_key(kind: &str) -> bool {
    matches!(kind, "openai_chat_completion" | "openai_realtime_ws")
}

fn credential_env_map(cfg: &SpearletConfig) -> HashMap<String, String> {
    cfg.llm
        .credentials
        .iter()
        .filter(|c| c.kind.as_str() == "env")
        .filter(|c| !c.name.trim().is_empty())
        .filter(|c| !c.api_key_env.trim().is_empty())
        .map(|c| (c.name.clone(), c.api_key_env.clone()))
        .collect()
}

fn resolve_backend_env(
    backend: &LlmBackendConfig,
    creds: &HashMap<String, String>,
) -> Result<String, String> {
    let r = backend
        .credential_ref
        .as_ref()
        .ok_or_else(|| "missing credential_ref".to_string())?;
    if r.trim().is_empty() {
        return Err("credential_ref is empty".to_string());
    }
    creds
        .get(r)
        .cloned()
        .ok_or_else(|| format!("credential_ref not found: {r}"))
}

fn build_backend_info_list(cfg: &SpearletConfig) -> Vec<BackendInfo> {
    let creds = credential_env_map(cfg);
    let mut out = Vec::new();

    out.push(BackendInfo {
        name: "stub".to_string(),
        kind: "stub".to_string(),
        operations: vec!["chat_completions".to_string()],
        features: Vec::new(),
        transports: vec!["inprocess".to_string()],
        weight: 0,
        priority: 0,
        base_url: String::new(),
        status: BackendStatus::Available as i32,
        status_reason: String::new(),
    });

    for b in cfg.llm.backends.iter() {
        let mut status = BackendStatus::Available as i32;
        let mut reason = String::new();

        if backend_requires_api_key(&b.kind) {
            let env = resolve_backend_env(b, &creds);
            match env {
                Ok(env_name) => match std::env::var(&env_name) {
                    Ok(v) if !v.trim().is_empty() => {}
                    _ => {
                        status = BackendStatus::Unavailable as i32;
                        reason = format!("missing env {env_name}");
                    }
                },
                Err(e) => {
                    status = BackendStatus::Unavailable as i32;
                    reason = e;
                }
            }
        }

        out.push(BackendInfo {
            name: b.name.clone(),
            kind: b.kind.clone(),
            operations: b.ops.clone(),
            features: b.features.clone(),
            transports: b.transports.clone(),
            weight: b.weight as u32,
            priority: b.priority,
            base_url: b.base_url.clone(),
            status,
            status_reason: reason,
        });
    }

    out
}

async fn report_loop(config: Arc<SpearletConfig>, cancel: CancellationToken) {
    let mut backoff_ms = config.sms_connect_retry_ms.max(200);
    let mut ticker = interval(Duration::from_secs(30));
    let node_uuid = config.compute_node_uuid();
    let mut revision: u64 = 0;

    loop {
        if cancel.is_cancelled() {
            return;
        }
        ticker.tick().await;

        let mut client = match connect_sms(&config).await {
            Ok(c) => c,
            Err(e) => {
                warn!(error = %e, "backend reporter connect failed");
                tokio::select! {
                    _ = cancel.cancelled() => return,
                    _ = tokio::time::sleep(Duration::from_millis(backoff_ms)) => {}
                }
                backoff_ms = (backoff_ms * 2).min(10_000);
                continue;
            }
        };
        backoff_ms = config.sms_connect_retry_ms.max(200);

        revision = revision.saturating_add(1);
        let backends = build_backend_info_list(&config);
        let snapshot = NodeBackendSnapshot {
            node_uuid: node_uuid.clone(),
            revision,
            reported_at_ms: 0,
            backends,
        };
        let req = ReportNodeBackendsRequest {
            snapshot: Some(snapshot),
        };
        match client.report_node_backends(req).await {
            Ok(resp) => {
                let inner = resp.into_inner();
                debug!(
                    node_uuid = %node_uuid,
                    accepted_revision = inner.accepted_revision,
                    "backend snapshot reported"
                );
            }
            Err(e) => {
                warn!(error = %e, "backend snapshot report failed");
            }
        }
    }
}
