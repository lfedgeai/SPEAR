//! Tests for SMS configuration / SMS配置测试

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
            grpc_addr: None,
            http_addr: None,
            db_type: None,
            db_path: None,
            db_pool_size: None,
            enable_swagger: false,
            disable_swagger: false,
            log_level: None,
            heartbeat_timeout: None,
            cleanup_interval: None,
        };

        assert_eq!(args.config, None);
        assert_eq!(args.grpc_addr, None);
        assert_eq!(args.http_addr, None);
        assert_eq!(args.db_type, None);
        assert_eq!(args.db_path, None);
        assert_eq!(args.db_pool_size, None);
        assert!(!args.enable_swagger);
        assert!(!args.disable_swagger);
        assert_eq!(args.log_level, None);
        assert_eq!(args.heartbeat_timeout, None);
        assert_eq!(args.cleanup_interval, None);
    }

    #[test]
    fn test_sms_config_default() {
        // Test default SmsConfig / 测试默认SmsConfig
        let config = SmsConfig::default();
        
        assert_eq!(config.grpc.addr.to_string(), "127.0.0.1:50051");
        assert_eq!(config.http.addr.to_string(), "127.0.0.1:8080");
        assert_eq!(config.log.level, "info");
        assert!(config.enable_swagger); // Default is true
        assert_eq!(config.database.db_type, "sled");
        assert_eq!(config.database.path, "./data/sms");
        assert_eq!(config.database.pool_size, Some(10));
    }

    #[test]
    fn test_database_config_default() {
        // Test default DatabaseConfig / 测试默认DatabaseConfig
        let config = DatabaseConfig::default();
        
        assert_eq!(config.db_type, "sled");
        assert_eq!(config.path, "./data");
        assert_eq!(config.pool_size, Some(10));
    }

    #[test]
    fn test_sms_config_load_with_cli_no_config_file() {
        // Test loading config without config file / 测试无配置文件时的配置加载
        let args = CliArgs {
            config: None,
            grpc_addr: Some("127.0.0.1:50052".to_string()),
            http_addr: Some("127.0.0.1:8081".to_string()),
            db_type: Some("rocksdb".to_string()),
            db_path: Some("./test-data".to_string()),
            db_pool_size: Some(5),
            enable_swagger: true,
            disable_swagger: false,
            log_level: Some("debug".to_string()),
            heartbeat_timeout: Some(30),
            cleanup_interval: Some(60),
        };

        let result = SmsConfig::load_with_cli(&args);
        assert!(result.is_ok());
        
        let config = result.unwrap();
        assert_eq!(config.grpc.addr.to_string(), "127.0.0.1:50052");
        assert_eq!(config.http.addr.to_string(), "127.0.0.1:8081");
        assert_eq!(config.database.db_type, "rocksdb");
        assert_eq!(config.database.path, "./test-data");
        assert_eq!(config.database.pool_size, Some(5));
        assert!(config.enable_swagger);
        assert_eq!(config.log.level, "debug");
    }

    #[test]
    fn test_sms_config_load_with_disable_swagger() {
        // Test loading config with disable swagger flag / 测试禁用Swagger标志的配置加载
        let args = CliArgs {
            config: None,
            grpc_addr: None,
            http_addr: None,
            db_type: None,
            db_path: None,
            db_pool_size: None,
            enable_swagger: false,
            disable_swagger: true,
            log_level: None,
            heartbeat_timeout: None,
            cleanup_interval: None,
        };

        let result = SmsConfig::load_with_cli(&args);
        assert!(result.is_ok());
        
        let config = result.unwrap();
        assert!(!config.enable_swagger);
    }

    #[test]
    fn test_sms_config_load_invalid_grpc_addr() {
        // Test loading config with invalid gRPC address / 测试无效gRPC地址的配置加载
        let args = CliArgs {
            config: None,
            grpc_addr: Some("invalid-address".to_string()),
            http_addr: None,
            db_type: None,
            db_path: None,
            db_pool_size: None,
            enable_swagger: false,
            disable_swagger: false,
            log_level: None,
            heartbeat_timeout: None,
            cleanup_interval: None,
        };

        let result = SmsConfig::load_with_cli(&args);
        assert!(result.is_err());
    }

    #[test]
    fn test_sms_config_load_invalid_http_addr() {
        // Test loading config with invalid HTTP address / 测试无效HTTP地址的配置加载
        let args = CliArgs {
            config: None,
            grpc_addr: None,
            http_addr: Some("invalid-address".to_string()),
            db_type: None,
            db_path: None,
            db_pool_size: None,
            enable_swagger: false,
            disable_swagger: false,
            log_level: None,
            heartbeat_timeout: None,
            cleanup_interval: None,
        };

        let result = SmsConfig::load_with_cli(&args);
        assert!(result.is_err());
    }

    #[test]
    fn test_database_config_validation() {
        // Test database configuration validation / 测试数据库配置验证
        let valid_types = vec!["sled", "rocksdb"];
        
        for db_type in valid_types {
            let config = DatabaseConfig {
                db_type: db_type.to_string(),
                path: "./test-data".to_string(),
                pool_size: Some(5),
            };
            
            assert!(config.db_type == "sled" || config.db_type == "rocksdb");
            assert!(!config.path.is_empty());
            assert!(config.pool_size.unwrap() > 0);
        }
    }

    #[test]
    fn test_sms_config_with_config_file_path() {
        // Test config file path handling / 测试配置文件路径处理
        let args = CliArgs {
            config: Some("./test-config.toml".to_string()),
            grpc_addr: None,
            http_addr: None,
            db_type: None,
            db_path: None,
            db_pool_size: None,
            enable_swagger: false,
            disable_swagger: false,
            log_level: None,
            heartbeat_timeout: None,
            cleanup_interval: None,
        };

        // This should not fail even if file doesn't exist since we use defaults
        // 即使文件不存在也不应该失败，因为我们使用默认值
        let result = SmsConfig::load_with_cli(&args);
        assert!(result.is_ok());
    }

    #[test]
    fn test_sms_config_edge_cases() {
        // Test edge cases for SMS configuration / 测试SMS配置的边界情况
        let args = CliArgs {
            config: None,
            grpc_addr: Some("0.0.0.0:0".to_string()), // Port 0 should be valid
            http_addr: Some("127.0.0.1:65535".to_string()), // Max port
            db_type: Some("sled".to_string()),
            db_path: Some("/".to_string()), // Root path
            db_pool_size: Some(1), // Minimum pool size
            enable_swagger: true,
            disable_swagger: false,
            log_level: Some("trace".to_string()),
            heartbeat_timeout: Some(1),
            cleanup_interval: Some(1),
        };

        let result = SmsConfig::load_with_cli(&args);
        assert!(result.is_ok());
        
        let config = result.unwrap();
        assert_eq!(config.grpc.addr.port(), 0);
        assert_eq!(config.http.addr.port(), 65535);
        assert_eq!(config.database.pool_size, Some(1));
        assert_eq!(config.log.level, "trace");
    }

    #[test]
    fn test_sms_config_both_swagger_flags() {
        // Test behavior when both swagger flags are set / 测试同时设置两个Swagger标志的行为
        let args = CliArgs {
            config: None,
            grpc_addr: None,
            http_addr: None,
            db_type: None,
            db_path: None,
            db_pool_size: None,
            enable_swagger: true,
            disable_swagger: true,
            log_level: None,
            heartbeat_timeout: None,
            cleanup_interval: None,
        };

        let result = SmsConfig::load_with_cli(&args);
        assert!(result.is_ok());
        
        let config = result.unwrap();
        // enable_swagger should take precedence / enable_swagger应该优先
        assert!(config.enable_swagger);
    }
}