use std::sync::{Arc, OnceLock};
use std::sync::atomic::{AtomicU64, Ordering};

use parking_lot::RwLock;

use crate::proto::sms::BackendInfo;

#[derive(Clone, Debug, Default)]
pub struct ManagedBackendRegistry {
    backends: Arc<RwLock<Vec<BackendInfo>>>,
    revision: Arc<AtomicU64>,
}

impl ManagedBackendRegistry {
    pub fn new() -> Self {
        Self::default()
    }

    pub fn set_backends(&self, backends: Vec<BackendInfo>) {
        let mut guard = self.backends.write();
        *guard = backends;
        self.revision.fetch_add(1, Ordering::Relaxed);
    }

    pub fn list(&self) -> Vec<BackendInfo> {
        self.backends.read().clone()
    }

    pub fn revision(&self) -> u64 {
        self.revision.load(Ordering::Relaxed)
    }
}

static GLOBAL_MANAGED_BACKENDS: OnceLock<ManagedBackendRegistry> = OnceLock::new();

pub fn global_managed_backends() -> ManagedBackendRegistry {
    GLOBAL_MANAGED_BACKENDS
        .get_or_init(ManagedBackendRegistry::new)
        .clone()
}
