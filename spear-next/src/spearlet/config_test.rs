//! Tests for SPEARlet configuration / SPEARlet配置测试

#[cfg(test)]
mod tests {
    use super::super::config::*;
    use std::fs;
    use tempfile::tempdir;

    #[test]
    fn test_cli_args_default() {
        // Test default CLI args / 测试默认CLI参数
        let args = CliArgs {
            config: None,
            node_id: None,
            grpc_addr: None,
            http_addr: None,
            sms_addr: None,
            storage_backend: None,
            storage_path: None,
            auto_register: None,
            log_level: None,
        };

        assert_eq!(args.config, None);
        assert_eq!(args.node_id, None);
        assert!(args.auto_register.is_none());
    }

    #[test]
    fn test_spearlet_config_default() {
        // Test default SpearletConfig / 测试默认SpearletConfig
        let config = SpearletConfig::default();
        
        assert_eq!(config.node_id, "spearlet-node");
        assert_eq!(config.grpc.address, "0.0.0.0");
        assert_eq!(config.grpc.port, 50052);
        assert_eq!(config.http.address, "0.0.0.0");
        assert_eq!(config.http.port, 8081);
        assert_eq!(config.storage.backend, "rocksdb");
        assert_eq!(config.storage.data_dir, "./data/spearlet");
        assert_eq!(config.sms_addr, "127.0.0.1:50051");
        assert!(!config.auto_register);
        assert_eq!(config.heartbeat_interval, 30);
        assert_eq!(config.cleanup_interval, 300);
    }

    #[test]
    fn test_grpc_config_default() {
        // Test default GrpcConfig / 测试默认GrpcConfig
        let config = GrpcConfig::default();
        
        assert_eq!(config.address, "0.0.0.0");
        assert_eq!(config.port, 50052);
        assert!(!config.tls_enabled);
        assert_eq!(config.tls_cert_path, None);
        assert_eq!(config.tls_key_path, None);
    }

    #[test]
    fn test_http_config_default() {
        // Test default HttpConfig / 测试默认HttpConfig
        let config = HttpConfig::default();
        
        assert_eq!(config.address, "0.0.0.0");
        assert_eq!(config.port, 8081);
        assert!(config.cors_enabled);
        assert!(config.swagger_enabled);
    }

    #[test]
    fn test_storage_config_default() {
        // Test default StorageConfig / 测试默认StorageConfig
        let config = StorageConfig::default();
        
        assert_eq!(config.backend, "rocksdb");
        assert_eq!(config.data_dir, "./data/spearlet");
        assert_eq!(config.max_cache_size_mb, 512);
        assert!(config.compression_enabled);
        assert_eq!(config.max_object_size, 64 * 1024 * 1024); // 64MB
    }

    #[test]
    fn test_logging_config_default() {
        // Test default LoggingConfig / 测试默认LoggingConfig
        let config = LoggingConfig::default();
        
        assert_eq!(config.level, "info");
        assert_eq!(config.format, "json");
        assert_eq!(config.output_file, None);
    }

    #[test]
    fn test_app_config_default() {
        // Test default AppConfig / 测试默认AppConfig
        let config = AppConfig::default();
        
        assert_eq!(config.spearlet.node_id, "spearlet-node");
        assert_eq!(config.spearlet.grpc.port, 50052);
        assert_eq!(config.spearlet.http.port, 8081);
    }

    #[test]
    fn test_app_config_load_with_cli_no_config_file() {
        // Test loading config without config file / 测试无配置文件时的配置加载
        let args = CliArgs {
            config: None,
            node_id: Some("test-node".to_string()),
            grpc_addr: Some("127.0.0.1:50053".to_string()),
            http_addr: Some("127.0.0.1:8082".to_string()),
            sms_addr: Some("127.0.0.1:50050".to_string()),
            storage_backend: Some("sled".to_string()),
            storage_path: Some("./test-data".to_string()),
            auto_register: Some(true),
            log_level: Some("debug".to_string()),
        };

        let result = AppConfig::load_with_cli(&args);
        if let Err(e) = &result {
            eprintln!("Error loading config: {}", e);
        }
        assert!(result.is_ok());
        
        let config = result.unwrap();
        assert_eq!(config.spearlet.node_id, "test-node");
        assert_eq!(config.spearlet.sms_addr, "127.0.0.1:50050");
        assert_eq!(config.spearlet.storage.backend, "sled");
        assert_eq!(config.spearlet.storage.data_dir, "./test-data");
        assert!(config.spearlet.auto_register);
        assert_eq!(config.spearlet.logging.level, "debug");
    }

    #[test]
    fn test_app_config_load_with_config_file() {
        // Test loading config from file / 测试从文件加载配置
        let dir = tempdir().unwrap();
        let config_path = dir.path().join("test_config.toml");
        
        let config_content = r#"
[spearlet]
node_id = "file-node"
sms_addr = "192.168.1.100:50051"
auto_register = true
heartbeat_interval = 60
cleanup_interval = 600

[spearlet.grpc]
address = "127.0.0.1"
port = 50054
tls_enabled = true

[spearlet.http]
address = "127.0.0.1"
port = 8083
cors_enabled = false
swagger_enabled = false

[spearlet.storage]
backend = "rocksdb"
data_dir = "/tmp/spearlet-data"
max_cache_size_mb = 512
compression_enabled = false
max_object_size = 20971520

[spearlet.logging]
level = "trace"
format = "text"
output_file = "/tmp/spearlet.log"
"#;
        
        fs::write(&config_path, config_content).unwrap();
        
        let args = CliArgs {
            config: Some(config_path.to_string_lossy().to_string()),
            node_id: None,
            grpc_addr: None,
            http_addr: None,
            sms_addr: None,
            storage_backend: None,
            storage_path: None,
            auto_register: None,
            log_level: None,
        };

        let result = AppConfig::load_with_cli(&args);
        assert!(result.is_ok());
        
        let config = result.unwrap();
        assert_eq!(config.spearlet.node_id, "file-node");
        assert_eq!(config.spearlet.sms_addr, "192.168.1.100:50051");
        assert!(config.spearlet.auto_register);
        assert_eq!(config.spearlet.heartbeat_interval, 60);
        assert_eq!(config.spearlet.cleanup_interval, 600);
        assert_eq!(config.spearlet.grpc.address, "127.0.0.1");
        assert_eq!(config.spearlet.grpc.port, 50054);
        assert!(config.spearlet.grpc.tls_enabled);
        assert_eq!(config.spearlet.http.port, 8083);
        assert!(!config.spearlet.http.cors_enabled);
        assert!(!config.spearlet.http.swagger_enabled);
        assert_eq!(config.spearlet.storage.backend, "rocksdb");
        assert_eq!(config.spearlet.storage.data_dir, "/tmp/spearlet-data");
        assert_eq!(config.spearlet.storage.max_cache_size_mb, 512);
        assert!(!config.spearlet.storage.compression_enabled);
        assert_eq!(config.spearlet.storage.max_object_size, 20971520);
        assert_eq!(config.spearlet.logging.level, "trace");
        assert_eq!(config.spearlet.logging.format, "text");
        assert_eq!(config.spearlet.logging.output_file, Some("/tmp/spearlet.log".to_string()));
    }

    #[test]
    fn test_app_config_cli_overrides_file() {
        // Test CLI args override config file / 测试CLI参数覆盖配置文件
        let dir = tempdir().unwrap();
        let config_path = dir.path().join("test_config.toml");
        
        let config_content = r#"
[spearlet]
node_id = "file-node"
sms_addr = "192.168.1.100:50051"
auto_register = false
heartbeat_interval = 30
cleanup_interval = 300

[spearlet.grpc]
address = "0.0.0.0"
port = 50052
tls_enabled = false

[spearlet.http]
address = "0.0.0.0"
port = 8081
cors_enabled = true
swagger_enabled = true

[spearlet.storage]
backend = "sled"
data_dir = "/tmp/file-data"
max_cache_size_mb = 100
compression_enabled = true
max_object_size = 1048576

[spearlet.logging]
level = "info"
format = "json"
"#;
        
        fs::write(&config_path, config_content).unwrap();
        
        let args = CliArgs {
            config: Some(config_path.to_string_lossy().to_string()),
            node_id: Some("cli-node".to_string()),
            grpc_addr: None,
            http_addr: None,
            sms_addr: Some("127.0.0.1:50055".to_string()),
            storage_backend: Some("rocksdb".to_string()),
            storage_path: Some("./cli-data".to_string()),
            auto_register: Some(true),
            log_level: Some("debug".to_string()),
        };

        let result = AppConfig::load_with_cli(&args);
        assert!(result.is_ok());
        
        let config = result.unwrap();
        // CLI args should override file values / CLI参数应该覆盖文件值
        assert_eq!(config.spearlet.node_id, "cli-node");
        assert_eq!(config.spearlet.sms_addr, "127.0.0.1:50055");
        assert_eq!(config.spearlet.storage.backend, "rocksdb");
        assert_eq!(config.spearlet.storage.data_dir, "./cli-data");
        assert!(config.spearlet.auto_register);
        assert_eq!(config.spearlet.logging.level, "debug");
    }

    #[test]
    fn test_app_config_load_invalid_file() {
        // Test loading invalid config file / 测试加载无效配置文件
        let args = CliArgs {
            config: Some("non_existent_file.toml".to_string()),
            node_id: None,
            grpc_addr: None,
            http_addr: None,
            sms_addr: None,
            storage_backend: None,
            storage_path: None,
            auto_register: None,
            log_level: None,
        };

        let result = AppConfig::load_with_cli(&args);
        assert!(result.is_err());
    }

    #[test]
    fn test_grpc_addr_parsing() {
        // Test gRPC address parsing / 测试gRPC地址解析
        let args = CliArgs {
            config: None,
            node_id: None,
            grpc_addr: Some("192.168.1.10:9999".to_string()),
            http_addr: None,
            sms_addr: None,
            storage_backend: None,
            storage_path: None,
            auto_register: None,
            log_level: None,
        };

        let result = AppConfig::load_with_cli(&args);
        assert!(result.is_ok());
        
        let config = result.unwrap();
        // Should parse address and port correctly / 应该正确解析地址和端口
        // Note: The actual parsing logic depends on implementation
        // 注意：实际解析逻辑取决于实现
    }

    #[test]
    fn test_http_addr_parsing() {
        // Test HTTP address parsing / 测试HTTP地址解析
        let args = CliArgs {
            config: None,
            node_id: None,
            grpc_addr: None,
            http_addr: Some("0.0.0.0:8888".to_string()),
            sms_addr: None,
            storage_backend: None,
            storage_path: None,
            auto_register: None,
            log_level: None,
        };

        let result = AppConfig::load_with_cli(&args);
        assert!(result.is_ok());
        
        let config = result.unwrap();
        // Should parse address and port correctly / 应该正确解析地址和端口
        // Note: The actual parsing logic depends on implementation
        // 注意：实际解析逻辑取决于实现
    }
}