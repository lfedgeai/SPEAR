#[cfg(test)]
mod tests {
    use std::sync::Arc;
    use tempfile::tempdir;
    use crate::spearlet::config::{SpearletConfig, HttpConfig, StorageConfig};
    use crate::spearlet::task_events::TaskEventSubscriber;

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

    #[test]
    fn test_cursor_store_roundtrip() {
        let cfg = tmp_cfg();
        let sub = TaskEventSubscriber::new(cfg.clone());
        TaskEventSubscriber::store_cursor(&cfg, 42);
        let v = TaskEventSubscriber::load_cursor(&cfg);
        assert_eq!(v, 42);
        drop(sub);
    }
}

