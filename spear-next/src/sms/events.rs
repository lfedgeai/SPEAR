use std::collections::HashMap;
use std::sync::Arc;
use tokio::sync::broadcast;

use crate::proto::sms::{Task, TaskEvent, TaskEventKind};
use crate::sms::services::error::SmsError;
use crate::storage::kv::{serialization, KvPair, KvStore};
use tracing::{debug, warn};

const OUTBOX_PREFIX: &str = "task_events:"; // key: task_events:{node_uuid}:{event_id}

#[derive(serde::Serialize, serde::Deserialize, Debug, Clone)]
struct StoredEvent {
    event_id: i64,
    ts: i64,
    node_uuid: String,
    task_id: String,
    kind: i32,
    execution_kind: i32,
}

#[derive(Debug, Clone)]
pub struct TaskEventBus {
    kv: Arc<dyn KvStore>,
    channels: Arc<tokio::sync::RwLock<HashMap<String, broadcast::Sender<TaskEvent>>>>,
    counters: Arc<tokio::sync::RwLock<HashMap<String, i64>>>,
}

impl TaskEventBus {
    pub fn new(kv: Arc<dyn KvStore>) -> Self {
        Self {
            kv,
            channels: Arc::new(tokio::sync::RwLock::new(HashMap::new())),
            counters: Arc::new(tokio::sync::RwLock::new(HashMap::new())),
        }
    }

    async fn next_id(&self, node_uuid: &str) -> i64 {
        let mut map = self.counters.write().await;
        let e = map.entry(node_uuid.to_string()).or_insert(0);
        *e += 1;
        *e
    }

    async fn get_sender(&self, node_uuid: &str) -> broadcast::Sender<TaskEvent> {
        let mut chans = self.channels.write().await;
        if let Some(tx) = chans.get(node_uuid) {
            return tx.clone();
        }
        let (tx, _rx) = broadcast::channel(1024);
        chans.insert(node_uuid.to_string(), tx.clone());
        tx
    }

    pub async fn subscribe(&self, node_uuid: &str) -> broadcast::Receiver<TaskEvent> {
        let tx = self.get_sender(node_uuid).await;
        tx.subscribe()
    }

    pub async fn replay_since(
        &self,
        node_uuid: &str,
        last_event_id: i64,
        limit: usize,
    ) -> Result<Vec<TaskEvent>, SmsError> {
        debug!(node_uuid = %node_uuid, last_event_id, limit, "Replaying task events");
        let prefix = format!("{}{}:", OUTBOX_PREFIX, node_uuid);
        let all = self.kv.scan_prefix(&prefix).await?;
        let mut events: Vec<TaskEvent> = Vec::new();
        for KvPair { key, value } in all.into_iter() {
            if let Some(id_str) = key.strip_prefix(&prefix) {
                if let Ok(id) = id_str.parse::<i64>() {
                    if id > last_event_id {
                        let se: StoredEvent = serialization::deserialize(&value)?;
                        let ev = TaskEvent {
                            event_id: se.event_id,
                            ts: se.ts,
                            node_uuid: se.node_uuid,
                            task_id: se.task_id,
                            kind: se.kind,
                            execution_kind: se.execution_kind,
                        };
                        events.push(ev);
                    }
                }
            }
        }
        events.sort_by_key(|e| e.event_id);
        debug!(count = events.len(), "Replay collected events");
        if events.len() > limit {
            events.truncate(limit);
        }
        Ok(events)
    }

    pub async fn publish_create(&self, task: &Task) -> Result<TaskEvent, SmsError> {
        self.publish(task, TaskEventKind::Create).await
    }

    pub async fn publish_update(&self, task: &Task) -> Result<TaskEvent, SmsError> {
        self.publish(task, TaskEventKind::Update).await
    }

    pub async fn publish_cancel(&self, task: &Task) -> Result<TaskEvent, SmsError> {
        self.publish(task, TaskEventKind::Cancel).await
    }

    async fn publish(&self, task: &Task, kind: TaskEventKind) -> Result<TaskEvent, SmsError> {
        let node_uuid = task.node_uuid.clone();
        let id = self.next_id(&node_uuid).await;
        let ek_val = if task.execution_kind
            == crate::proto::sms::TaskExecutionKind::LongRunning as i32
        {
            crate::proto::sms::TaskExecutionKind::LongRunning as i32
        } else if task.execution_kind == crate::proto::sms::TaskExecutionKind::ShortRunning as i32 {
            crate::proto::sms::TaskExecutionKind::ShortRunning as i32
        } else {
            crate::proto::sms::TaskExecutionKind::ShortRunning as i32
        };
        let ev = TaskEvent {
            event_id: id,
            ts: chrono::Utc::now().timestamp(),
            node_uuid: node_uuid.clone(),
            task_id: task.task_id.clone(),
            kind: kind as i32,
            execution_kind: ek_val,
        };
        let key = format!("{}{}:{}", OUTBOX_PREFIX, node_uuid, id);
        let se = StoredEvent {
            event_id: ev.event_id,
            ts: ev.ts,
            node_uuid: ev.node_uuid.clone(),
            task_id: ev.task_id.clone(),
            kind: ev.kind,
            execution_kind: ek_val,
        };
        let val = serialization::serialize(&se)?;
        self.kv.put(&key, &val).await?;
        debug!(node_uuid = %node_uuid, event_id = id, kind = ev.kind, task_id = %ev.task_id, "Published task event to KV and broadcasting");
        let tx = self.get_sender(&node_uuid).await;
        if let Err(e) = tx.send(ev.clone()) {
            warn!(error = %e, "Broadcast send failed");
        }
        Ok(ev)
    }
}
