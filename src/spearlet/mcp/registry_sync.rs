use std::sync::Arc;
use std::time::Duration;

use tokio::sync::RwLock;
use tokio::time::{interval, timeout};
use tokio_util::sync::CancellationToken;
use tonic::transport::Channel;
use tracing::{debug, warn};

use crate::proto::sms::mcp_registry_service_client::McpRegistryServiceClient;
use crate::proto::sms::{ListMcpServersRequest, McpServerRecord, WatchMcpServersRequest};
use crate::spearlet::config::SpearletConfig;

#[derive(Debug, Clone, Default)]
pub struct McpRegistrySnapshot {
    pub revision: u64,
    pub servers: Vec<McpServerRecord>,
}

#[derive(Debug, Default)]
pub struct McpRegistryCache {
    inner: RwLock<McpRegistrySnapshot>,
}

impl McpRegistryCache {
    pub async fn snapshot(&self) -> McpRegistrySnapshot {
        self.inner.read().await.clone()
    }

    async fn replace(&self, snapshot: McpRegistrySnapshot) {
        *self.inner.write().await = snapshot;
    }
}

#[derive(Debug)]
pub struct McpRegistrySyncService {
    config: Arc<SpearletConfig>,
    cache: Arc<McpRegistryCache>,
    cancel: CancellationToken,
}

impl McpRegistrySyncService {
    pub fn new(config: Arc<SpearletConfig>) -> Self {
        Self {
            config,
            cache: Arc::new(McpRegistryCache::default()),
            cancel: CancellationToken::new(),
        }
    }

    pub fn cache(&self) -> Arc<McpRegistryCache> {
        self.cache.clone()
    }

    pub fn shutdown(&self) {
        self.cancel.cancel();
    }

    pub fn start(&self) {
        let config = self.config.clone();
        let cache = self.cache.clone();
        let cancel = self.cancel.clone();
        if let Ok(handle) = tokio::runtime::Handle::try_current() {
            handle.spawn(async move {
                sync_loop(config, cache, cancel).await;
            });
            return;
        }

        std::thread::spawn(move || {
            let rt = tokio::runtime::Builder::new_current_thread()
                .enable_all()
                .build();
            if let Ok(rt) = rt {
                rt.block_on(async move {
                    sync_loop(config, cache, cancel).await;
                });
            }
        });
    }
}

async fn connect_sms(config: &SpearletConfig) -> Result<McpRegistryServiceClient<Channel>, tonic::Status> {
    let sms_url = format!("http://{}", config.sms_grpc_addr);
    let connect_fut = McpRegistryServiceClient::connect(sms_url);
    let client = timeout(Duration::from_millis(config.sms_connect_timeout_ms), connect_fut)
        .await
        .map_err(|_| tonic::Status::deadline_exceeded("connect sms timeout"))?
        .map_err(|e| tonic::Status::unavailable(format!("connect sms failed: {}", e)))?;
    Ok(client)
}

async fn refresh_once(
    client: &mut McpRegistryServiceClient<Channel>,
    cache: &McpRegistryCache,
) -> Result<u64, tonic::Status> {
    let resp = client
        .list_mcp_servers(ListMcpServersRequest { since_revision: 0 })
        .await?
        .into_inner();
    let snapshot = McpRegistrySnapshot {
        revision: resp.revision,
        servers: resp.servers,
    };
    cache.replace(snapshot).await;
    Ok(resp.revision)
}

async fn sync_loop(config: Arc<SpearletConfig>, cache: Arc<McpRegistryCache>, cancel: CancellationToken) {
    let mut backoff_ms = config.sms_connect_retry_ms.max(200);
    let mut revision: Option<u64> = None;
    let mut poll = interval(Duration::from_secs(60));

    loop {
        if cancel.is_cancelled() {
            return;
        }

        let mut client = match connect_sms(&config).await {
            Ok(c) => c,
            Err(e) => {
                warn!(error = %e, "MCP registry connect failed");
                tokio::select! {
                    _ = cancel.cancelled() => return,
                    _ = tokio::time::sleep(Duration::from_millis(backoff_ms)) => {}
                }
                backoff_ms = (backoff_ms * 2).min(10_000);
                continue;
            }
        };
        backoff_ms = config.sms_connect_retry_ms.max(200);

        match refresh_once(&mut client, &cache).await {
            Ok(r) => {
                revision = Some(r);
                debug!(revision, "MCP registry snapshot refreshed");
            }
            Err(e) => {
                warn!(error = %e, "MCP registry list failed");
                tokio::select! {
                    _ = cancel.cancelled() => return,
                    _ = tokio::time::sleep(Duration::from_millis(backoff_ms)) => {}
                }
                continue;
            }
        }

        let mut watch_stream = match client
            .watch_mcp_servers(WatchMcpServersRequest {
                since_revision: revision.unwrap_or(0),
            })
            .await
        {
            Ok(r) => r.into_inner(),
            Err(e) => {
                warn!(error = %e, "MCP registry watch start failed");
                continue;
            }
        };

        loop {
            tokio::select! {
                _ = cancel.cancelled() => return,
                _ = poll.tick() => {
                    if let Ok(r) = refresh_once(&mut client, &cache).await {
                        revision = Some(r);
                    }
                }
                msg = watch_stream.message() => {
                    match msg {
                        Ok(Some(resp)) => {
                            if let Some(event) = resp.event {
                                if event.revision > revision.unwrap_or(0) {
                                    if let Ok(r) = refresh_once(&mut client, &cache).await {
                                        revision = Some(r);
                                    }
                                }
                            }
                        }
                        Ok(None) => break,
                        Err(e) => {
                            warn!(error = %e, "MCP registry watch ended");
                            break;
                        }
                    }
                }
            }
        }
    }
}
