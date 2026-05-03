use dashmap::DashMap;
use parking_lot::Mutex;
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::{Arc, OnceLock};

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum TerminationScope {
    Execution,
    Instance,
}

#[derive(Clone, Debug)]
pub struct TerminationSnapshot {
    pub scope: TerminationScope,
    pub errno: i32,
    pub message: Option<String>,
}

#[derive(Debug)]
struct TerminationState {
    terminated: AtomicBool,
    scope: TerminationScope,
    errno: i32,
    message: Mutex<Option<String>>,
}

impl TerminationState {
    fn snapshot(&self) -> TerminationSnapshot {
        TerminationSnapshot {
            scope: self.scope,
            errno: self.errno,
            message: self.message.lock().clone(),
        }
    }
}

#[derive(Clone, Debug)]
pub struct WasmTerminationRegistry {
    scope: TerminationScope,
    inner: Arc<DashMap<String, Arc<TerminationState>>>,
}

impl WasmTerminationRegistry {
    fn new(scope: TerminationScope) -> Self {
        Self {
            scope,
            inner: Arc::new(DashMap::new()),
        }
    }

    pub fn mark(&self, key: &str, errno: i32, message: Option<String>) {
        let entry = self.inner.entry(key.to_string()).or_insert_with(|| {
            Arc::new(TerminationState {
                terminated: AtomicBool::new(false),
                scope: self.scope,
                errno,
                message: Mutex::new(None),
            })
        });
        entry.terminated.store(true, Ordering::Release);
        *entry.message.lock() = message;
    }

    pub fn clear(&self, key: &str) {
        self.inner.remove(key);
    }

    pub fn check(&self, key: &str) -> Option<TerminationSnapshot> {
        let entry = self.inner.get(key)?;
        if !entry.terminated.load(Ordering::Acquire) {
            return None;
        }
        Some(entry.snapshot())
    }
}

static EXEC_REGISTRY: OnceLock<Arc<WasmTerminationRegistry>> = OnceLock::new();
static INSTANCE_REGISTRY: OnceLock<Arc<WasmTerminationRegistry>> = OnceLock::new();

pub fn exec_registry() -> Arc<WasmTerminationRegistry> {
    EXEC_REGISTRY
        .get_or_init(|| Arc::new(WasmTerminationRegistry::new(TerminationScope::Execution)))
        .clone()
}

pub fn instance_registry() -> Arc<WasmTerminationRegistry> {
    INSTANCE_REGISTRY
        .get_or_init(|| Arc::new(WasmTerminationRegistry::new(TerminationScope::Instance)))
        .clone()
}

pub fn mark_execution_terminated(execution_id: &str, errno: i32, reason: Option<String>) {
    exec_registry().mark(execution_id, errno, reason);
}

pub fn clear_execution_termination(execution_id: &str) {
    exec_registry().clear(execution_id);
}

pub fn mark_instance_destroyed(instance_id: &str, errno: i32, reason: Option<String>) {
    instance_registry().mark(instance_id, errno, reason);
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::spearlet::execution::host_api::DefaultHostApi;
    use crate::spearlet::execution::runtime::{ResourcePoolConfig, RuntimeConfig, RuntimeType};
    use std::collections::HashMap;

    fn test_runtime_config() -> RuntimeConfig {
        RuntimeConfig {
            runtime_type: RuntimeType::Wasm,
            settings: HashMap::new(),
            global_environment: HashMap::new(),
            spearlet_config: None,
            resource_pool: ResourcePoolConfig::default(),
        }
    }

    #[test]
    fn test_exec_registry_mark_check_clear() {
        let exec_id = "exec-test-1";
        let reg = exec_registry();
        reg.clear(exec_id);

        assert!(reg.check(exec_id).is_none());

        reg.mark(exec_id, -11, Some("terminated".to_string()));
        let s = reg.check(exec_id).unwrap();
        assert_eq!(s.scope, TerminationScope::Execution);
        assert_eq!(s.errno, -11);
        assert_eq!(s.message.as_deref(), Some("terminated"));

        reg.clear(exec_id);
        assert!(reg.check(exec_id).is_none());
    }

    #[test]
    fn test_instance_registry_mark_check_clear() {
        let instance_id = "inst-test-1";
        let reg = instance_registry();
        reg.clear(instance_id);

        assert!(reg.check(instance_id).is_none());

        reg.mark(instance_id, -123, Some("destroyed".to_string()));
        let s = reg.check(instance_id).unwrap();
        assert_eq!(s.scope, TerminationScope::Instance);
        assert_eq!(s.errno, -123);
        assert_eq!(s.message.as_deref(), Some("destroyed"));

        reg.clear(instance_id);
        assert!(reg.check(instance_id).is_none());
    }

    #[test]
    fn test_default_host_api_prefers_execution_over_instance() {
        let exec_id = "exec-test-2";
        let instance_id = "inst-test-2";
        exec_registry().clear(exec_id);
        instance_registry().clear(instance_id);

        mark_execution_terminated(exec_id, -libc::ECANCELED, Some("e".to_string()));
        mark_instance_destroyed(instance_id, -libc::ECANCELED, Some("i".to_string()));

        let mut api =
            DefaultHostApi::new(test_runtime_config()).with_instance_id(instance_id.to_string());
        api.set_execution_id(Some(exec_id.to_string()));

        let s = api.check_wasm_termination().unwrap();
        assert_eq!(s.scope, TerminationScope::Execution);
        assert_eq!(s.message.as_deref(), Some("e"));

        exec_registry().clear(exec_id);
        instance_registry().clear(instance_id);
    }
}
