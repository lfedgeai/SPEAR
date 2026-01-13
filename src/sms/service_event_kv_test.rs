#[cfg(test)]
mod tests {
    use std::sync::Arc;
    use tokio_stream::StreamExt;
    use tonic::Request;

    use crate::proto::sms::{
        task_service_server::TaskService as TaskServiceTrait, RegisterTaskRequest,
        SubscribeTaskEventsRequest, TaskExecutionKind, TaskPriority,
    };
    use crate::sms::config::SmsConfig;
    use crate::sms::service::SmsServiceImpl;
    use crate::sms::services::{node_service::NodeService, resource_service::ResourceService};
    use crate::storage::kv::KvStoreConfig;

    fn make_request(node_uuid: &str) -> RegisterTaskRequest {
        RegisterTaskRequest {
            name: "t".to_string(),
            description: "d".to_string(),
            priority: TaskPriority::Normal as i32,
            node_uuid: node_uuid.to_string(),
            endpoint: "http://localhost".to_string(),
            version: "v1".to_string(),
            capabilities: vec![],
            metadata: std::collections::HashMap::new(),
            config: std::collections::HashMap::new(),
            executable: None,
            execution_kind: TaskExecutionKind::ShortRunning as i32,
        }
    }

    async fn register_and_replay(cfg: SmsConfig) -> i32 {
        let svc = SmsServiceImpl::new(
            Arc::new(tokio::sync::RwLock::new(NodeService::new())),
            Arc::new(ResourceService::new()),
            Arc::new(cfg),
        )
        .await;
        let node_uuid = uuid::Uuid::new_v4().to_string();
        let _ = TaskServiceTrait::register_task(&svc, Request::new(make_request(&node_uuid)))
            .await
            .unwrap();
        let req = SubscribeTaskEventsRequest {
            node_uuid,
            last_event_id: 0,
        };
        let mut stream = TaskServiceTrait::subscribe_task_events(&svc, Request::new(req))
            .await
            .unwrap()
            .into_inner();
        let mut count = 0;
        if let Some(Ok(_ev)) = stream.next().await {
            count += 1;
        }
        count
    }

    #[tokio::test]
    async fn test_event_kv_memory_independent_from_database() {
        let mut cfg = SmsConfig::default();
        cfg.database.db_type = "rocksdb".to_string();
        cfg.event_kv = Some(KvStoreConfig::memory());
        let count = register_and_replay(cfg).await;
        assert!(count >= 1);
    }

    #[tokio::test]
    async fn test_event_kv_unsupported_backend_fallback_to_memory() {
        let mut cfg = SmsConfig::default();
        cfg.event_kv = Some(KvStoreConfig {
            backend: "unsupported".to_string(),
            params: std::collections::HashMap::new(),
        });
        let count = register_and_replay(cfg).await;
        assert!(count >= 1);
    }

    #[tokio::test]
    async fn test_event_kv_loaded_from_toml() {
        let tmp = tempfile::tempdir().unwrap();
        let path = tmp.path().join("sms-config.toml");
        let content = r#"
[grpc]
addr = "127.0.0.1:50051"

[http]
addr = "127.0.0.1:8080"

[database]
db_type = "sled"
path = "./data/sms.db"
pool_size = 10

[event_kv]
backend = "memory"
params = {}
"#;
        std::fs::write(&path, content).unwrap();
        let args = crate::sms::config::CliArgs {
            config: Some(path.to_string_lossy().to_string()),
            grpc_addr: None,
            http_addr: None,
            db_type: None,
            db_path: None,
            db_pool_size: None,
            enable_swagger: false,
            disable_swagger: false,
            enable_web_admin: false,
            disable_web_admin: false,
            web_admin_addr: None,
            mcp_dir: None,
            log_level: None,
            heartbeat_timeout: None,
            cleanup_interval: None,
            max_upload_bytes: None,
        };
        let cfg = crate::sms::config::SmsConfig::load_with_cli(&args).unwrap();
        assert!(cfg.event_kv.is_some());
        assert_eq!(cfg.event_kv.as_ref().unwrap().backend, "memory");
    }
}
