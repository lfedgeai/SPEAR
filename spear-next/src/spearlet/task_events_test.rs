#[cfg(test)]
mod tests {
    use std::sync::Arc;
    use tempfile::tempdir;
    use crate::spearlet::config::{SpearletConfig, HttpConfig, StorageConfig};
    use crate::spearlet::task_events::TaskEventSubscriber;
    use crate::spearlet::execution::runtime::RuntimeManager;
    use crate::spearlet::execution::manager::{TaskExecutionManager, TaskExecutionManagerConfig};
    use crate::proto::sms::{Task, TaskExecutable, TaskEvent, TaskEventKind};

    fn tmp_cfg() -> Arc<SpearletConfig> {
        let dir = tempdir().unwrap();
        let p = dir.path().to_string_lossy().to_string();
        Arc::new(SpearletConfig {
            node_name: "test-node".to_string(),
            grpc: crate::config::base::ServerConfig { addr: "127.0.0.1:50052".parse().unwrap(), ..Default::default() },
            http: HttpConfig { server: crate::config::base::ServerConfig { addr: "127.0.0.1:8081".parse().unwrap(), ..Default::default() } },
            storage: StorageConfig { backend: "memory".to_string(), data_dir: p, max_cache_size_mb: 16, compression_enabled: false, max_object_size: 1024*1024 },
            logging: crate::config::base::LogConfig::default(),
            sms_addr: "127.0.0.1:50051".to_string(),
            auto_register: false,
            heartbeat_interval: 5,
            cleanup_interval: 60,
            sms_connect_timeout_ms: 1000,
            sms_connect_retry_ms: 200,
            reconnect_total_timeout_ms: 2000,
        })
    }

    #[tokio::test]
    async fn test_cursor_store_roundtrip() {
        let cfg = tmp_cfg();
        let runtime_manager = Arc::new(RuntimeManager::new());
        let mgr = TaskExecutionManager::new(TaskExecutionManagerConfig::default(), runtime_manager)
            .await
            .unwrap();
        let sub = TaskEventSubscriber::new(cfg.clone(), mgr);
        TaskEventSubscriber::store_cursor(&cfg, 42);
        let v = TaskEventSubscriber::load_cursor(&cfg);
        assert_eq!(v, 42);
        drop(sub);
    }

    #[tokio::test]
    async fn test_start_unreachable_sms_does_not_panic() {
        let mut cfg = (*tmp_cfg()).clone();
        cfg.sms_addr = "127.0.0.1:65535".to_string();
        let cfg = Arc::new(cfg);
        let runtime_manager = Arc::new(RuntimeManager::new());
        let mgr = TaskExecutionManager::new(TaskExecutionManagerConfig::default(), runtime_manager)
            .await
            .unwrap();
        let sub = TaskEventSubscriber::new(cfg.clone(), mgr);
        sub.start().await;
        tokio::time::sleep(std::time::Duration::from_millis(100)).await;
    }

    fn make_task(node_uuid: &str, task_id: &str) -> Task {
        Task {
            task_id: task_id.to_string(),
            name: String::new(),
            description: String::new(),
            status: 0,
            priority: 0,
            node_uuid: node_uuid.to_string(),
            endpoint: String::new(),
            version: String::new(),
            capabilities: vec![],
            registered_at: 0,
            last_heartbeat: 0,
            metadata: std::collections::HashMap::new(),
            config: std::collections::HashMap::new(),
            executable: Some(TaskExecutable { r#type: 5, uri: String::new(), name: String::new(), checksum_sha256: String::new(), args: vec![], env: std::collections::HashMap::new() }),
        }
    }

    #[tokio::test]
    async fn test_handle_update_event_no_execution() {
        let mut cfg = (*tmp_cfg()).clone();
        cfg.node_name = "00000000-0000-0000-0000-000000000000".to_string();
        let cfg = Arc::new(cfg);
        let runtime_manager = Arc::new(RuntimeManager::new());
        let mgr = TaskExecutionManager::new(TaskExecutionManagerConfig::default(), runtime_manager)
            .await
            .unwrap();
        let sub = TaskEventSubscriber::new(cfg.clone(), mgr.clone());

        let ev = TaskEvent { event_id: 1, ts: 0, node_uuid: cfg.node_name.clone(), task_id: "t1".to_string(), kind: TaskEventKind::Update as i32 };
        let task = make_task(&cfg.node_name, "t1");
        sub.handle_event_for_test(ev, Some(task)).await;
        tokio::time::sleep(std::time::Duration::from_millis(50)).await;
        let stats = mgr.get_statistics();
        assert_eq!(stats.total_executions, 0);
    }

    #[tokio::test]
    async fn test_handle_cancel_event_no_execution() {
        let mut cfg = (*tmp_cfg()).clone();
        cfg.node_name = "00000000-0000-0000-0000-000000000000".to_string();
        let cfg = Arc::new(cfg);
        let runtime_manager = Arc::new(RuntimeManager::new());
        let mgr = TaskExecutionManager::new(TaskExecutionManagerConfig::default(), runtime_manager)
            .await
            .unwrap();
        let sub = TaskEventSubscriber::new(cfg.clone(), mgr.clone());

        let ev = TaskEvent { event_id: 1, ts: 0, node_uuid: cfg.node_name.clone(), task_id: "t2".to_string(), kind: TaskEventKind::Cancel as i32 };
        let task = make_task(&cfg.node_name, "t2");
        sub.handle_event_for_test(ev, Some(task)).await;
        tokio::time::sleep(std::time::Duration::from_millis(50)).await;
        let stats = mgr.get_statistics();
        assert_eq!(stats.total_executions, 0);
    }

    #[tokio::test]
    async fn test_handle_create_event_triggers_execution() {
        let mut cfg = (*tmp_cfg()).clone();
        cfg.node_name = "00000000-0000-0000-0000-000000000000".to_string();
        let cfg = Arc::new(cfg);
        let runtime_manager = Arc::new(RuntimeManager::new());
        let mgr = TaskExecutionManager::new(TaskExecutionManagerConfig::default(), runtime_manager)
            .await
            .unwrap();
        let sub = TaskEventSubscriber::new(cfg.clone(), mgr.clone());

        let ev = TaskEvent { event_id: 1, ts: 0, node_uuid: cfg.node_name.clone(), task_id: "t3".to_string(), kind: TaskEventKind::Create as i32 };
        let task = make_task(&cfg.node_name, "t3");
        sub.handle_event_for_test(ev, Some(task)).await;
        tokio::time::sleep(std::time::Duration::from_millis(200)).await;
        let stats = mgr.get_statistics();
        assert!(stats.total_executions >= 1);
    }
}
