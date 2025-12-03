use std::{fs, io::Write, path::PathBuf, sync::Arc, time::Duration};
use tokio::sync::RwLock;
use tokio_stream::StreamExt;
use tonic::transport::Channel;

use crate::proto::sms::{task_service_client::TaskServiceClient, SubscribeTaskEventsRequest, TaskEvent, TaskEventKind, GetTaskRequest, Task};
use crate::proto::spearlet::{ArtifactSpec as SpearletArtifactSpec, InvokeFunctionRequest};
use crate::spearlet::execution::manager::TaskExecutionManager;
use crate::spearlet::config::SpearletConfig;
use tracing::{debug, info, warn};

pub struct TaskEventSubscriber {
    config: Arc<SpearletConfig>,
    last_event_id: Arc<RwLock<i64>>, 
    execution_manager: Arc<TaskExecutionManager>,
}

impl TaskEventSubscriber {
    pub fn new(config: Arc<SpearletConfig>, execution_manager: Arc<TaskExecutionManager>) -> Self {
        let last = Self::load_cursor(&config);
        Self { config, last_event_id: Arc::new(RwLock::new(last)), execution_manager }
    }

    fn compute_node_uuid(cfg: &SpearletConfig) -> String {
        if let Ok(u) = uuid::Uuid::parse_str(&cfg.node_name) { return u.to_string(); }
        let base = format!("{}:{}:{}", cfg.grpc.addr.ip(), cfg.grpc.addr.port(), cfg.node_name);
        uuid::Uuid::new_v5(&uuid::Uuid::NAMESPACE_OID, base.as_bytes()).to_string()
    }

    fn cursor_path(cfg: &SpearletConfig) -> PathBuf {
        let node = Self::compute_node_uuid(cfg);
        PathBuf::from(&cfg.storage.data_dir).join(format!("task_events_cursor_{}.json", node))
    }

    fn load_cursor(cfg: &SpearletConfig) -> i64 {
        let p = Self::cursor_path(cfg);
        if let Ok(s) = fs::read_to_string(&p) { s.parse::<i64>().unwrap_or(0) } else { 0 }
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
                let channel = match Channel::from_shared(sms_grpc_url.clone()).unwrap().connect().await { Ok(c)=>c, Err(e)=>{ warn!(error = %e, "SMS channel connect failed, retrying"); tokio::time::sleep(Duration::from_millis(cfg.sms_connect_retry_ms)).await; continue; } };
                let mut client = TaskServiceClient::new(channel);
                let last = *last_event_id.read().await;
                let req = SubscribeTaskEventsRequest { node_uuid: node_uuid.clone(), last_event_id: last };
                debug!(node_uuid = %node_uuid, last_event_id = last, "Subscribing to task events");
                let mut stream = match client.subscribe_task_events(req).await { Ok(r)=> r.into_inner(), Err(e)=>{ warn!(error = %e, "SubscribeTaskEvents RPC failed, retrying"); tokio::time::sleep(Duration::from_millis(cfg.sms_connect_retry_ms)).await; continue; } };
                loop {
                    match stream.next().await { 
                        Some(Ok(ev)) => { 
                            debug!(event_id = ev.event_id, kind = ev.kind, task_id = %ev.task_id, node_uuid = %ev.node_uuid, "Received task event");
                            *last_event_id.write().await = ev.event_id; 
                            Self::store_cursor(&cfg, ev.event_id);
                            Self::handle_event(&cfg, &mut client, &exec_mgr, ev).await; 
                        },
                        Some(Err(e)) => { warn!(error = %e, "Event stream error, reconnecting"); break; },
                        None => { break; },
                    }
                }
                debug!(delay_ms = cfg.sms_connect_retry_ms, "Reconnect delay before resubscribing");
                tokio::time::sleep(Duration::from_millis(cfg.sms_connect_retry_ms)).await;
            }
        });
    }

    async fn handle_event(cfg: &SpearletConfig, client: &mut TaskServiceClient<Channel>, mgr: &Arc<TaskExecutionManager>, ev: TaskEvent) {
        if ev.node_uuid != Self::compute_node_uuid(cfg) { debug!(event_id = ev.event_id, "Ignoring event for other node"); return; }
        if ev.kind == TaskEventKind::Create as i32 {
            debug!(task_id = %ev.task_id, "Fetching task details");
            let task = match client.get_task(GetTaskRequest{ task_id: ev.task_id.clone() }).await {
                Ok(resp) => resp.into_inner().task,
                Err(_) => None,
            };
            Self::execute_task(mgr, ev.task_id, task);
        }
    }

    fn execute_task(mgr: &Arc<TaskExecutionManager>, _task_id: String, task: Option<Task>) {
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
            let location = if let Some(exec) = &t.executable { exec.uri.clone() } else { String::new() };
            let checksum = if let Some(exec) = &t.executable { exec.checksum_sha256.clone() } else { String::new() };

            let spec = SpearletArtifactSpec {
                artifact_id: t.task_id.clone(),
                artifact_type,
                location,
                version,
                checksum,
                metadata: t.metadata.clone(),
            };

            let req = InvokeFunctionRequest {
                artifact_spec: Some(spec),
                execution_mode: crate::proto::spearlet::ExecutionMode::Async as i32,
                ..Default::default()
            };
            let mgr_cloned = mgr.clone();
            tokio::spawn(async move {
                let _ = mgr_cloned.submit_execution(req).await;
            });
        } else {
            debug!(task_id = %_task_id, "Task details unavailable");
        }
    }

    #[cfg(test)]
    pub async fn handle_event_for_test(&self, ev: TaskEvent, task: Option<Task>) {
        if ev.node_uuid != Self::compute_node_uuid(&self.config) { return; }
        if ev.kind == TaskEventKind::Create as i32 {
            Self::execute_task(&self.execution_manager, ev.task_id, task);
        } else {
            // ignore update/cancel in current implementation
        }
    }
}
