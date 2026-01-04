use crate::spearlet::execution::ai::router::Router;
use crate::spearlet::execution::ai::AiEngine;
use crate::spearlet::execution::host_api::iface::{HttpCallResult, SpearHostApi};
use crate::spearlet::execution::hostcall::fd_table::FdTable;
use crate::spearlet::execution::ExecutionError;
use std::collections::HashMap;
use std::sync::Arc;
use std::time::{SystemTime, UNIX_EPOCH};
use tracing::{debug, info, warn};

#[derive(Clone, Debug)]
pub struct DefaultHostApi {
    pub(super) runtime_config: super::super::runtime::RuntimeConfig,
    pub(super) fd_table: Arc<FdTable>,
    pub(super) ai_engine: Arc<AiEngine>,
}

impl DefaultHostApi {
    pub fn new(runtime_config: super::super::runtime::RuntimeConfig) -> Self {
        let (registry, policy) = super::registry::build_registry_from_runtime_config(&runtime_config);
        let router = Router::new(registry, policy);
        let ai_engine = Arc::new(AiEngine::new(router));
        Self {
            runtime_config,
            fd_table: Arc::new(FdTable::new(1000)),
            ai_engine,
        }
    }
}

impl SpearHostApi for DefaultHostApi {
    fn log(&self, level: &str, message: &str) {
        match level {
            "trace" => debug!("{message}"),
            "debug" => debug!("{message}"),
            "info" => info!("{message}"),
            "warn" => warn!("{message}"),
            "error" => tracing::error!("{message}"),
            _ => info!("{message}"),
        }
    }

    fn time_now_ms(&self) -> u64 {
        SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .unwrap_or_default()
            .as_millis() as u64
    }

    fn random_bytes(&self, len: usize) -> Vec<u8> {
        use rand::RngCore;
        let mut out = vec![0u8; len];
        rand::thread_rng().fill_bytes(&mut out);
        out
    }

    fn get_env(&self, key: &str) -> Option<String> {
        self.runtime_config.global_environment.get(key).cloned()
    }

    fn http_call(
        &self,
        _method: &str,
        _url: &str,
        _headers: HashMap<String, String>,
        _body: Vec<u8>,
    ) -> Result<HttpCallResult, ExecutionError> {
        Err(ExecutionError::NotSupported {
            operation: "http_call".to_string(),
        })
    }

    fn put_result(
        &self,
        task_id: &str,
        data: Vec<u8>,
        metadata: HashMap<String, String>,
    ) -> Result<String, ExecutionError> {
        let _ = (task_id, data, metadata);
        Err(ExecutionError::NotSupported {
            operation: "put_result".to_string(),
        })
    }

    fn get_object(&self, id: &str) -> Result<Vec<u8>, ExecutionError> {
        let _ = id;
        Err(ExecutionError::NotSupported {
            operation: "get_object".to_string(),
        })
    }

    fn put_object(&self, name: &str, bytes: Vec<u8>) -> Result<String, ExecutionError> {
        let _ = (name, bytes);
        Err(ExecutionError::NotSupported {
            operation: "put_object".to_string(),
        })
    }
}

