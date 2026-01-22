#[cfg(test)]
mod tests {
    use super::super::events::TaskEventBus;
    use crate::proto::sms::{Task, TaskPriority, TaskStatus};
    use crate::storage::kv::MemoryKvStore;
    use std::sync::Arc;

    fn sample_task(node_uuid: &str, name: &str) -> Task {
        Task {
            task_id: uuid::Uuid::new_v4().to_string(),
            name: name.to_string(),
            description: "".to_string(),
            status: TaskStatus::Registered as i32,
            priority: TaskPriority::Normal as i32,
            node_uuid: node_uuid.to_string(),
            endpoint: "".to_string(),
            version: "v1".to_string(),
            capabilities: vec![],
            registered_at: chrono::Utc::now().timestamp(),
            last_heartbeat: chrono::Utc::now().timestamp(),
            metadata: Default::default(),
            config: Default::default(),
            executable: None,
            result_uris: Vec::new(),
            last_result_uri: String::new(),
            last_result_status: String::new(),
            last_completed_at: 0,
            last_result_metadata: Default::default(),
        }
    }

    #[tokio::test]
    async fn test_publish_and_subscribe() {
        let kv: Arc<dyn crate::storage::kv::KvStore> = Arc::new(MemoryKvStore::new());
        let bus = TaskEventBus::new(kv);

        let node_uuid = uuid::Uuid::new_v4().to_string();
        let mut rx = bus.subscribe(&node_uuid).await;

        let t = sample_task(&node_uuid, "t1");
        let ev = bus.publish_create(&t).await.unwrap();

        let recv = rx.recv().await.unwrap();
        assert_eq!(recv.task_id, t.task_id);
        assert_eq!(recv.event_id, ev.event_id);
        assert_eq!(recv.node_uuid, node_uuid);
    }

    #[tokio::test]
    async fn test_durable_replay_since() {
        let kv: Arc<dyn crate::storage::kv::KvStore> = Arc::new(MemoryKvStore::new());
        let bus = TaskEventBus::new(kv);
        let node_uuid = uuid::Uuid::new_v4().to_string();
        let t1 = sample_task(&node_uuid, "t1");
        let e1 = bus.publish_create(&t1).await.unwrap();
        let t2 = sample_task(&node_uuid, "t2");
        let e2 = bus.publish_create(&t2).await.unwrap();

        let replay = bus
            .replay_since(&node_uuid, e1.event_id, 100)
            .await
            .unwrap();
        assert_eq!(replay.len(), 1);
        assert_eq!(replay[0].event_id, e2.event_id);
        assert_eq!(replay[0].task_id, t2.task_id);
    }
}
