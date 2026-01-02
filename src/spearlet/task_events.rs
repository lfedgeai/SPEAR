use std::{fs, io::Write, path::PathBuf, sync::Arc, time::Duration};
use tokio::sync::RwLock;
use tokio_stream::StreamExt;
use tonic::transport::Channel;

use crate::proto::sms::{
    task_service_client::TaskServiceClient, GetTaskRequest, SubscribeTaskEventsRequest, Task,
    TaskEvent, TaskEventKind, TaskExecutionKind,
};
use crate::proto::spearlet::{ArtifactSpec as SpearletArtifactSpec, InvokeFunctionRequest};
use crate::spearlet::config::SpearletConfig;
use crate::spearlet::execution::manager::TaskExecutionManager;
use sha2::{Digest, Sha256};
use tracing::{debug, info, warn};

pub struct TaskEventSubscriber {
    config: Arc<SpearletConfig>,
    last_event_id: Arc<RwLock<i64>>,
    execution_manager: Arc<TaskExecutionManager>,
}

impl TaskEventSubscriber {
    pub fn new(config: Arc<SpearletConfig>, execution_manager: Arc<TaskExecutionManager>) -> Self {
        let last = Self::load_cursor(&config);
        Self {
            config,
            last_event_id: Arc::new(RwLock::new(last)),
            execution_manager,
        }
    }

    fn compute_node_uuid(cfg: &SpearletConfig) -> String {
        if let Ok(u) = uuid::Uuid::parse_str(&cfg.node_name) {
            return u.to_string();
        }
        let base = format!(
            "{}:{}:{}",
            cfg.grpc.addr.ip(),
            cfg.grpc.addr.port(),
            cfg.node_name
        );
        uuid::Uuid::new_v5(&uuid::Uuid::NAMESPACE_OID, base.as_bytes()).to_string()
    }

    fn cursor_path(cfg: &SpearletConfig) -> PathBuf {
        let node = Self::compute_node_uuid(cfg);
        PathBuf::from(&cfg.storage.data_dir).join(format!("task_events_cursor_{}.json", node))
    }

    fn load_cursor(cfg: &SpearletConfig) -> i64 {
        let p = Self::cursor_path(cfg);
        if let Ok(s) = fs::read_to_string(&p) {
            s.parse::<i64>().unwrap_or(0)
        } else {
            0
        }
    }

    fn store_cursor(cfg: &SpearletConfig, v: i64) {
        let p = Self::cursor_path(cfg);
        let _ = fs::create_dir_all(p.parent().unwrap_or(std::path::Path::new(".")));
        let mut f = fs::File::create(p).unwrap();
        let _ = write!(f, "{}", v);
    }

    pub async fn start(self) {
        let cfg = self.config.clone();
        let exec_mgr = self.execution_manager.clone();
        let last_event_id = self.last_event_id.clone();
        tokio::spawn(async move {
            let node_uuid = Self::compute_node_uuid(&cfg);
            info!(node_uuid = %node_uuid, sms_grpc_addr = %cfg.sms_grpc_addr, "TaskEventSubscriber starting");
            loop {
                let sms_grpc_url = format!("http://{}", cfg.sms_grpc_addr);
                let channel = match Channel::from_shared(sms_grpc_url.clone())
                    .unwrap()
                    .connect()
                    .await
                {
                    Ok(c) => c,
                    Err(e) => {
                        warn!(error = %e, "SMS channel connect failed, retrying");
                        tokio::time::sleep(Duration::from_millis(cfg.sms_connect_retry_ms)).await;
                        continue;
                    }
                };
                let mut client = TaskServiceClient::new(channel);
                let last = *last_event_id.read().await;
                let req = SubscribeTaskEventsRequest {
                    node_uuid: node_uuid.clone(),
                    last_event_id: last,
                };
                debug!(node_uuid = %node_uuid, last_event_id = last, "Subscribing to task events");
                let mut stream = match client.subscribe_task_events(req).await {
                    Ok(r) => r.into_inner(),
                    Err(e) => {
                        warn!(error = %e, "SubscribeTaskEvents RPC failed, retrying");
                        tokio::time::sleep(Duration::from_millis(cfg.sms_connect_retry_ms)).await;
                        continue;
                    }
                };
                loop {
                    match stream.next().await {
                        Some(Ok(ev)) => {
                            debug!(event_id = ev.event_id, kind = ev.kind, task_id = %ev.task_id, node_uuid = %ev.node_uuid, "Received task event");
                            *last_event_id.write().await = ev.event_id;
                            Self::store_cursor(&cfg, ev.event_id);
                            Self::handle_event(&cfg, &mut client, &exec_mgr, ev).await;
                        }
                        Some(Err(e)) => {
                            warn!(error = %e, "Event stream error, reconnecting");
                            break;
                        }
                        None => {
                            break;
                        }
                    }
                }
                debug!(
                    delay_ms = cfg.sms_connect_retry_ms,
                    "Reconnect delay before resubscribing"
                );
                tokio::time::sleep(Duration::from_millis(cfg.sms_connect_retry_ms)).await;
            }
        });
    }

    async fn handle_event(
        cfg: &SpearletConfig,
        client: &mut TaskServiceClient<Channel>,
        mgr: &Arc<TaskExecutionManager>,
        ev: TaskEvent,
    ) {
        if ev.node_uuid != Self::compute_node_uuid(cfg) {
            debug!(event_id = ev.event_id, "Ignoring event for other node");
            return;
        }
        if ev.kind == TaskEventKind::Create as i32 {
            let execution_id = ev
                .execution_id
                .clone()
                .filter(|id| !id.is_empty())
                .or_else(|| Some(format!("task-event-{}-{}", ev.node_uuid, ev.event_id)));
            debug!(task_id = %ev.task_id, "Fetching task details");
            let task = match client
                .get_task(GetTaskRequest {
                    task_id: ev.task_id.clone(),
                })
                .await
            {
                Ok(resp) => resp.into_inner().task,
                Err(_) => None,
            };
            Self::execute_task(mgr, ev.task_id, task, execution_id);
        } else {
            debug!(event_id = ev.event_id, kind = ev.kind, task_id = %ev.task_id, "Unhandled TaskEvent kind, ignoring");
        }
    }

    fn execute_task(
        mgr: &Arc<TaskExecutionManager>,
        task_id: String,
        task: Option<Task>,
        execution_id: Option<String>,
    ) {
        if let Some(t) = task {
            let artifact_type = if let Some(exec) = &t.executable {
                match exec.r#type {
                    3 => "kubernetes".to_string(),
                    4 => "wasm".to_string(),
                    _ => "process".to_string(),
                }
            } else {
                "process".to_string()
            };

            let version = t.metadata.get("version").cloned().unwrap_or_default();
            let location = if let Some(exec) = &t.executable {
                exec.uri.clone()
            } else {
                String::new()
            };
            let checksum = if let Some(exec) = &t.executable {
                exec.checksum_sha256.clone()
            } else {
                String::new()
            };
            let artifact_id = if !checksum.is_empty() {
                checksum.clone()
            } else if !location.is_empty() {
                let d = Sha256::digest(location.as_bytes());
                d.iter().map(|b| format!("{:02x}", b)).collect()
            } else {
                uuid::Uuid::new_v4().to_string()
            };

            let spec = SpearletArtifactSpec {
                artifact_id,
                artifact_type,
                location,
                version,
                checksum,
                metadata: t.metadata.clone(),
            };

            let mut req = InvokeFunctionRequest {
                invocation_type: crate::proto::spearlet::InvocationType::ExistingTask as i32,
                task_id: t.task_id.clone(),
                artifact_spec: Some(spec),
                execution_id: execution_id.clone(),
                ..Default::default()
            };
            let is_long = t.execution_kind == TaskExecutionKind::LongRunning as i32;
            if is_long {
                req.execution_mode = crate::proto::spearlet::ExecutionMode::Async as i32;
                req.wait = false;
            } else {
                req.execution_mode = crate::proto::spearlet::ExecutionMode::Sync as i32;
                req.wait = true;
            }
            let mgr_cloned = mgr.clone();
            let t_clone = t.clone();
            tokio::spawn(async move {
                if let Ok(artifact) = mgr_cloned.ensure_artifact_from_sms(&t_clone).await {
                    let _ = mgr_cloned.ensure_task_from_sms(&t_clone, &artifact).await;
                }
                let _ = mgr_cloned.submit_execution(req).await;
            });
        } else {
            debug!(task_id = %task_id, "Task details unavailable");
        }
    }

    #[cfg(test)]
    pub async fn handle_event_for_test(&self, ev: TaskEvent, task: Option<Task>) {
        if ev.node_uuid != Self::compute_node_uuid(&self.config) {
            return;
        }
        if ev.kind == TaskEventKind::Create as i32 {
            let execution_id = ev
                .execution_id
                .clone()
                .filter(|id| !id.is_empty())
                .or_else(|| Some(format!("task-event-{}-{}", ev.node_uuid, ev.event_id)));
            Self::execute_task(&self.execution_manager, ev.task_id, task, execution_id);
        } else {
            tracing::debug!(event_id = ev.event_id, kind = ev.kind, task_id = %ev.task_id, "Unhandled TaskEvent kind in test");
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::proto::sms::{
        Task, TaskEvent, TaskEventKind, TaskExecutable, TaskPriority, TaskStatus,
    };
    use crate::spearlet::execution::instance;
    use crate::spearlet::execution::runtime::{Runtime, RuntimeCapabilities, RuntimeType};
    use crate::spearlet::execution::TaskExecutionManagerConfig;
    use async_trait::async_trait;
    use sha2::Digest;
    use std::collections::HashMap as StdHashMap;

    struct DummyRuntime {
        ty: RuntimeType,
    }

    #[async_trait]
    impl Runtime for DummyRuntime {
        fn runtime_type(&self) -> RuntimeType {
            self.ty
        }
        async fn create_instance(
            &self,
            config: &instance::InstanceConfig,
        ) -> crate::spearlet::execution::ExecutionResult<Arc<instance::TaskInstance>> {
            Ok(Arc::new(instance::TaskInstance::new(
                config.task_id.clone(),
                config.clone(),
            )))
        }
        async fn start_instance(
            &self,
            _instance: &Arc<instance::TaskInstance>,
        ) -> crate::spearlet::execution::ExecutionResult<()> {
            Ok(())
        }
        async fn stop_instance(
            &self,
            _instance: &Arc<instance::TaskInstance>,
        ) -> crate::spearlet::execution::ExecutionResult<()> {
            Ok(())
        }
        async fn execute(
            &self,
            _instance: &Arc<instance::TaskInstance>,
            _context: crate::spearlet::execution::runtime::ExecutionContext,
        ) -> crate::spearlet::execution::ExecutionResult<
            crate::spearlet::execution::runtime::RuntimeExecutionResponse,
        > {
            Ok(crate::spearlet::execution::runtime::RuntimeExecutionResponse::default())
        }
        async fn health_check(
            &self,
            _instance: &Arc<instance::TaskInstance>,
        ) -> crate::spearlet::execution::ExecutionResult<bool> {
            Ok(true)
        }
        async fn get_metrics(
            &self,
            _instance: &Arc<instance::TaskInstance>,
        ) -> crate::spearlet::execution::ExecutionResult<StdHashMap<String, serde_json::Value>>
        {
            Ok(StdHashMap::new())
        }
        async fn scale_instance(
            &self,
            _instance: &Arc<instance::TaskInstance>,
            _new_limits: &instance::InstanceResourceLimits,
        ) -> crate::spearlet::execution::ExecutionResult<()> {
            Ok(())
        }
        async fn cleanup_instance(
            &self,
            _instance: &Arc<instance::TaskInstance>,
        ) -> crate::spearlet::execution::ExecutionResult<()> {
            Ok(())
        }
        fn validate_config(
            &self,
            _config: &instance::InstanceConfig,
        ) -> crate::spearlet::execution::ExecutionResult<()> {
            Ok(())
        }
        fn get_capabilities(&self) -> RuntimeCapabilities {
            RuntimeCapabilities::default()
        }
    }

    #[tokio::test]
    async fn test_event_existing_task_uses_task_id_and_checksum_artifact_id() {
        let mut rm = crate::spearlet::execution::runtime::RuntimeManager::new();
        rm.register_runtime(
            RuntimeType::Process,
            Box::new(DummyRuntime {
                ty: RuntimeType::Process,
            }),
        )
        .unwrap();
        let rm = Arc::new(rm);
        let mgr = TaskExecutionManager::new(
            TaskExecutionManagerConfig::default(),
            rm,
            Arc::new(SpearletConfig::default()),
        )
        .await
        .unwrap();

        let mut cfg = SpearletConfig::default();
        cfg.node_name = uuid::Uuid::new_v4().to_string();
        let sub = TaskEventSubscriber::new(Arc::new(cfg.clone()), mgr.clone());
        let node_uuid = TaskEventSubscriber::compute_node_uuid(&cfg);

        let mut meta = std::collections::HashMap::new();
        meta.insert("version".to_string(), "v1".to_string());
        let sms_task = Task {
            task_id: "task-x".to_string(),
            name: "t".to_string(),
            description: String::new(),
            status: TaskStatus::Registered as i32,
            priority: TaskPriority::Normal as i32,
            node_uuid: node_uuid.clone(),
            endpoint: String::new(),
            version: "v1".to_string(),
            capabilities: vec![],
            registered_at: chrono::Utc::now().timestamp(),
            last_heartbeat: chrono::Utc::now().timestamp(),
            metadata: meta,
            config: std::collections::HashMap::new(),
            executable: Some(TaskExecutable {
                r#type: 5,
                uri: "http://example/bin".to_string(),
                name: String::new(),
                checksum_sha256: "deadbeef".to_string(),
                args: vec![],
                env: std::collections::HashMap::new(),
            }),
            result_uris: Vec::new(),
            last_result_uri: String::new(),
            last_result_status: String::new(),
            last_completed_at: 0,
            last_result_metadata: std::collections::HashMap::new(),
            execution_kind: TaskExecutionKind::ShortRunning as i32,
        };
        let ev = TaskEvent {
            event_id: 1,
            ts: chrono::Utc::now().timestamp(),
            node_uuid: node_uuid.clone(),
            task_id: "task-x".to_string(),
            kind: TaskEventKind::Create as i32,
            execution_kind: TaskExecutionKind::ShortRunning as i32,
            execution_id: None,
        };
        sub.handle_event_for_test(ev, Some(sms_task)).await;
        tokio::time::sleep(Duration::from_millis(50)).await;

        assert!(mgr.get_task(&"task-x".to_string()).is_some());
        assert!(mgr.get_artifact(&"deadbeef".to_string()).is_some());
    }

    #[tokio::test]
    async fn test_event_artifact_id_hashes_uri_when_no_checksum() {
        let mut rm = crate::spearlet::execution::runtime::RuntimeManager::new();
        rm.register_runtime(
            RuntimeType::Process,
            Box::new(DummyRuntime {
                ty: RuntimeType::Process,
            }),
        )
        .unwrap();
        let rm = Arc::new(rm);
        let mgr = TaskExecutionManager::new(
            TaskExecutionManagerConfig::default(),
            rm,
            Arc::new(SpearletConfig::default()),
        )
        .await
        .unwrap();

        let mut cfg = SpearletConfig::default();
        cfg.node_name = uuid::Uuid::new_v4().to_string();
        let sub = TaskEventSubscriber::new(Arc::new(cfg.clone()), mgr.clone());
        let node_uuid = TaskEventSubscriber::compute_node_uuid(&cfg);

        let uri = "http://example/abc";
        let sms_task = Task {
            task_id: "task-y".to_string(),
            name: "t".to_string(),
            description: String::new(),
            status: TaskStatus::Registered as i32,
            priority: TaskPriority::Normal as i32,
            node_uuid: node_uuid.clone(),
            endpoint: String::new(),
            version: "v1".to_string(),
            capabilities: vec![],
            registered_at: chrono::Utc::now().timestamp(),
            last_heartbeat: chrono::Utc::now().timestamp(),
            metadata: std::collections::HashMap::new(),
            config: std::collections::HashMap::new(),
            executable: Some(TaskExecutable {
                r#type: 5,
                uri: uri.to_string(),
                name: String::new(),
                checksum_sha256: String::new(),
                args: vec![],
                env: std::collections::HashMap::new(),
            }),
            result_uris: Vec::new(),
            last_result_uri: String::new(),
            last_result_status: String::new(),
            last_completed_at: 0,
            last_result_metadata: std::collections::HashMap::new(),
            execution_kind: TaskExecutionKind::ShortRunning as i32,
        };
        let ev = TaskEvent {
            event_id: 2,
            ts: chrono::Utc::now().timestamp(),
            node_uuid: node_uuid.clone(),
            task_id: "task-y".to_string(),
            kind: TaskEventKind::Create as i32,
            execution_kind: TaskExecutionKind::ShortRunning as i32,
            execution_id: None,
        };
        sub.handle_event_for_test(ev, Some(sms_task)).await;
        tokio::time::sleep(Duration::from_millis(50)).await;

        let d = sha2::Sha256::digest(uri.as_bytes());
        let expected: String = d.iter().map(|b| format!("{:02x}", b)).collect();
        assert!(mgr.get_task(&"task-y".to_string()).is_some());
        assert!(mgr.get_artifact(&expected).is_some());
    }
}
