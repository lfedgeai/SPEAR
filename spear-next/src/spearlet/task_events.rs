use std::{fs, io::Write, path::PathBuf, sync::Arc, time::Duration};
use tokio::sync::RwLock;
use tokio_stream::StreamExt;
use tonic::transport::Channel;

use crate::proto::sms::{task_service_client::TaskServiceClient, SubscribeTaskEventsRequest, TaskEvent, TaskEventKind, GetTaskRequest, Task};
use crate::spearlet::config::SpearletConfig;
use tracing::{debug, info, warn};

pub struct TaskEventSubscriber {
    config: Arc<SpearletConfig>,
    last_event_id: Arc<RwLock<i64>>, 
}

impl TaskEventSubscriber {
    pub fn new(config: Arc<SpearletConfig>) -> Self {
        let last = Self::load_cursor(&config);
        Self { config, last_event_id: Arc::new(RwLock::new(last)) }
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
        let last_event_id = self.last_event_id.clone();
        tokio::spawn(async move {
            let node_uuid = Self::compute_node_uuid(&cfg);
            info!(node_uuid = %node_uuid, sms_addr = %cfg.sms_addr, "TaskEventSubscriber starting");
            loop {
                let sms_url = format!("http://{}", cfg.sms_addr);
                let channel = match Channel::from_shared(sms_url.clone()).unwrap().connect().await { Ok(c)=>c, Err(e)=>{ warn!(error = %e, "SMS channel connect failed, retrying"); tokio::time::sleep(Duration::from_millis(cfg.sms_connect_retry_ms)).await; continue; } };
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
                            Self::handle_event(&cfg, &mut client, ev).await; 
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

    async fn handle_event(cfg: &SpearletConfig, client: &mut TaskServiceClient<Channel>, ev: TaskEvent) {
        if ev.node_uuid != Self::compute_node_uuid(cfg) { debug!(event_id = ev.event_id, "Ignoring event for other node"); return; }
        if ev.kind == TaskEventKind::Create as i32 {
            debug!(task_id = %ev.task_id, "Fetching task details for execution placeholder");
            let task = match client.get_task(GetTaskRequest{ task_id: ev.task_id.clone() }).await {
                Ok(resp) => resp.into_inner().task,
                Err(_) => None,
            };
            Self::execute_task(ev.task_id, task);
        }
    }

    fn execute_task(_task_id: String, task: Option<Task>) {
        if let Some(t) = task {
            debug!(
                task_id = %t.task_id,
                name = %t.name,
                endpoint = %t.endpoint,
                status = t.status,
                priority = t.priority,
                node_uuid = %t.node_uuid,
                registered_at = t.registered_at,
                last_heartbeat = t.last_heartbeat,
                capabilities_count = t.capabilities.len(),
                metadata_count = t.metadata.len(),
                "Task details"
            );
        } else {
            debug!(task_id = %_task_id, "Task details unavailable");
        }
        info!(task_id = %_task_id, "Executing task placeholder (TODO)");
        todo!("Implement task execution dispatch");
    }
}
