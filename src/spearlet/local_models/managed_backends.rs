use std::sync::Arc;

use tokio::sync::RwLock;

use crate::proto::sms::BackendInfo;

#[derive(Clone, Debug, Default)]
pub struct ManagedBackendRegistry {
    backends: Arc<RwLock<Vec<BackendInfo>>>,
}

impl ManagedBackendRegistry {
    pub fn new() -> Self {
        Self::default()
    }

    pub async fn set_backends(&self, backends: Vec<BackendInfo>) {
        let mut guard = self.backends.write().await;
        *guard = backends;
    }

    pub async fn list(&self) -> Vec<BackendInfo> {
        self.backends.read().await.clone()
    }
}
