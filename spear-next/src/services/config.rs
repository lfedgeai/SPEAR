//! Configuration management for spear-next components
//! spear-next组件的配置管理

use figment::{Figment, providers::{Format, Toml, Env, Serialized}, Provider, Metadata, Profile, value::{Map, Value}};
use serde::{Deserialize, Serialize};
use std::net::SocketAddr;

use clap::Parser;
use crate::storage::KvStoreConfig;

/// SPEAR Metadata Server command line arguments / SPEAR元数据服务器命令行参数
#[derive(Parser, Debug, Clone)]
#[command(
    name = "sms",
    version = "0.1.0",
    about = "SPEAR Metadata Server - Node management and resource monitoring service\nSPEAR元数据服务器 - 节点管理和资源监控服务",
    long_about = "SMS (SPEAR Metadata Server) provides centralized node management and resource monitoring capabilities.\nSMS（SPEAR元数据服务器）提供集中式节点管理和资源监控功能。"
)]
pub struct CliArgs {
    /// Configuration file path / 配置文件路径
    #[arg(short, long, value_name = "FILE", help = "Configuration file path / 配置文件路径")]
    pub config: Option<String>,

    /// gRPC server address / gRPC服务器地址
    #[arg(long, value_name = "ADDR", help = "gRPC server address (e.g., 0.0.0.0:50051) / gRPC服务器地址")]
    pub grpc_addr: Option<String>,

    /// HTTP gateway address / HTTP网关地址
    #[arg(long, value_name = "ADDR", help = "HTTP gateway address (e.g., 0.0.0.0:8080) / HTTP网关地址")]
    pub http_addr: Option<String>,

    /// Heartbeat timeout in seconds / 心跳超时时间（秒）
    #[arg(long, value_name = "SECONDS", help = "Heartbeat timeout in seconds / 心跳超时时间（秒）")]
    pub heartbeat_timeout: Option<u64>,

    /// Cleanup interval in seconds / 清理间隔时间（秒）
    #[arg(long, value_name = "SECONDS", help = "Cleanup interval in seconds / 清理间隔时间（秒）")]
    pub cleanup_interval: Option<u64>,

    /// Enable Swagger UI / 启用Swagger UI
    #[arg(long, help = "Enable Swagger UI / 启用Swagger UI")]
    pub enable_swagger: bool,

    /// Log level / 日志级别
    #[arg(long, value_name = "LEVEL", help = "Log level (trace, debug, info, warn, error) / 日志级别")]
    pub log_level: Option<String>,

    /// Storage backend type / 存储后端类型
    #[arg(long, value_name = "BACKEND", help = "Storage backend type (memory, sled, rocksdb) / 存储后端类型")]
    pub storage_backend: Option<String>,

    /// Storage path for file-based backends / 基于文件的后端存储路径
    #[arg(long, value_name = "PATH", help = "Storage path for file-based backends / 基于文件的后端存储路径")]
    pub storage_path: Option<String>,
}

/// Custom Figment provider for command line arguments
/// 命令行参数的自定义Figment提供者
impl Provider for CliArgs {
    fn metadata(&self) -> Metadata {
        Metadata::named("CLI Arguments")
    }

    fn data(&self) -> Result<Map<Profile, Map<String, Value>>, figment::Error> {
        let mut map: Map<String, Value> = Map::new();
        let mut sms_map: Map<String, Value> = Map::new();

        // Only add values that were explicitly provided / 只添加明确提供的值
        if let Some(ref addr) = self.grpc_addr {
            sms_map.insert("grpc_addr".to_string(), Value::from(addr.clone()));
        }
        if let Some(ref addr) = self.http_addr {
            sms_map.insert("http_addr".to_string(), Value::from(addr.clone()));
        }
        if let Some(timeout) = self.heartbeat_timeout {
            sms_map.insert("heartbeat_timeout".to_string(), Value::from(timeout));
        }
        if let Some(interval) = self.cleanup_interval {
            sms_map.insert("cleanup_interval".to_string(), Value::from(interval));
        }
        // Note: enable_swagger is a flag, so we only set it if it's true
        // 注意：enable_swagger是一个标志，只有在为true时才设置
        if self.enable_swagger {
            sms_map.insert("enable_swagger".to_string(), Value::from(true));
        }

        // Add KV store configuration from CLI / 从CLI添加KV存储配置
        if self.storage_backend.is_some() || self.storage_path.is_some() {
            let mut kv_store_map: Map<String, Value> = Map::new();
            
            if let Some(ref backend) = self.storage_backend {
                kv_store_map.insert("backend".to_string(), Value::from(backend.clone()));
            }
            
            if let Some(ref path) = self.storage_path {
                let mut params_map: Map<String, Value> = Map::new();
                params_map.insert("path".to_string(), Value::from(path.clone()));
                kv_store_map.insert("params".to_string(), Value::from(params_map));
            }
            
            if !kv_store_map.is_empty() {
                sms_map.insert("kv_store".to_string(), Value::from(kv_store_map));
            }
        }

        if !sms_map.is_empty() {
            map.insert("sms".to_string(), Value::from(sms_map));
        }

        Ok(Map::from([(Profile::Default, map)]))
    }
}

/// SPEAR Metadata Server service configuration / SPEAR元数据服务器服务配置
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SmsConfig {
    /// gRPC server address / gRPC服务器地址
    pub grpc_addr: SocketAddr,
    /// HTTP gateway address / HTTP网关地址
    pub http_addr: SocketAddr,
    /// Node cleanup interval in seconds / 节点清理间隔（秒）
    pub cleanup_interval: u64,
    /// Node heartbeat timeout in seconds / 节点心跳超时（秒）
    pub heartbeat_timeout: u64,
    /// Enable Swagger UI / 启用Swagger UI
    pub enable_swagger: bool,
    /// KV store configuration / KV存储配置
    pub kv_store: KvStoreConfig,
}

impl Default for SmsConfig {
    fn default() -> Self {
        Self {
            grpc_addr: "0.0.0.0:50051".parse().unwrap(),
            http_addr: "0.0.0.0:8080".parse().unwrap(),
            cleanup_interval: 300, // 5 minutes
            heartbeat_timeout: 120, // 2 minutes
            enable_swagger: true,
            kv_store: KvStoreConfig::memory(),
        }
    }
}

/// Load configuration from file and environment variables
/// 从文件和环境变量加载配置
pub fn load_config() -> Result<SmsConfig, figment::Error> {
    Figment::new()
        .merge(Toml::file("config.toml"))
        .merge(Env::prefixed("SPEAR_"))
        .extract()
}

impl AppConfig {
    /// Load configuration from file and environment / 从文件和环境变量加载配置
    pub fn load() -> Result<Self, figment::Error> {
        Figment::new()
            .merge(Toml::file("config.toml"))
            .merge(Env::prefixed("SPEAR_"))
            .extract()
    }

    /// Load configuration from specified file and environment / 从指定文件和环境变量加载配置
    pub fn load_from_file(config_path: &str) -> Result<Self, figment::Error> {
        Figment::new()
            .merge(Toml::file(config_path))
            .merge(Env::prefixed("SPEAR_"))
            .extract()
    }

    /// Load configuration with command line arguments override
    /// 加载配置并使用命令行参数覆盖
    pub fn load_with_cli(args: &CliArgs) -> Result<Self, figment::Error> {
        let mut figment = Figment::new();

        // Start with defaults / 从默认值开始
        figment = figment.merge(Serialized::defaults(Self::default()));

        // Add config file if specified / 如果指定了配置文件则添加
        if let Some(ref config_path) = args.config {
            figment = figment.merge(Toml::file(config_path));
        } else {
            figment = figment.merge(Toml::file("config.toml"));
        }

        // Add environment variables / 添加环境变量
        figment = figment.merge(Env::prefixed("SPEAR_"));

        // Add command line arguments (highest priority) / 添加命令行参数（最高优先级）
        figment = figment.merge(args);

        figment.extract()
    }
}

/// Application-wide configuration / 应用程序全局配置
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AppConfig {
    pub sms: SmsConfig,
}

impl Default for AppConfig {
    fn default() -> Self {
        Self {
            sms: SmsConfig::default(),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    use std::fs;
    use tempfile::NamedTempFile;

    /// Test CLI arguments parsing / 测试CLI参数解析
    #[test]
    fn test_cli_args_parsing() {
        // Test with all arguments / 测试所有参数
        let args = CliArgs {
            config: Some("test-config.toml".to_string()),
            grpc_addr: Some("127.0.0.1:50052".to_string()),
            http_addr: Some("127.0.0.1:8081".to_string()),
            heartbeat_timeout: Some(180),
            cleanup_interval: Some(600),
            enable_swagger: true,
            log_level: Some("debug".to_string()),
            storage_backend: Some("sled".to_string()),
            storage_path: Some("/tmp/test.db".to_string()),
        };

        assert_eq!(args.config, Some("test-config.toml".to_string()));
        assert_eq!(args.grpc_addr, Some("127.0.0.1:50052".to_string()));
        assert_eq!(args.http_addr, Some("127.0.0.1:8081".to_string()));
        assert_eq!(args.heartbeat_timeout, Some(180));
        assert_eq!(args.cleanup_interval, Some(600));
        assert!(args.enable_swagger);
        assert_eq!(args.log_level, Some("debug".to_string()));
        assert_eq!(args.storage_backend, Some("sled".to_string()));
        assert_eq!(args.storage_path, Some("/tmp/test.db".to_string()));
    }

    /// Test CLI arguments with minimal values / 测试最小值CLI参数
    #[test]
    fn test_cli_args_minimal() {
        let args = CliArgs {
            config: None,
            grpc_addr: None,
            http_addr: None,
            heartbeat_timeout: None,
            cleanup_interval: None,
            enable_swagger: false,
            log_level: None,
            storage_backend: None,
            storage_path: None,
        };

        assert!(args.config.is_none());
        assert!(args.grpc_addr.is_none());
        assert!(args.http_addr.is_none());
        assert!(args.heartbeat_timeout.is_none());
        assert!(args.cleanup_interval.is_none());
        assert!(!args.enable_swagger);
        assert!(args.log_level.is_none());
    }

    /// Test CliArgs as Figment Provider / 测试CliArgs作为Figment Provider
    #[test]
    fn test_cli_args_provider() {
        let args = CliArgs {
            config: None,
            grpc_addr: Some("127.0.0.1:50053".to_string()),
            http_addr: Some("127.0.0.1:8082".to_string()),
            heartbeat_timeout: Some(240),
            cleanup_interval: Some(900),
            enable_swagger: true,
            log_level: None,
            storage_backend: None,
            storage_path: None,
        };

        let data = args.data().expect("Failed to get provider data");
        let default_profile = data.get(&Profile::Default).expect("No default profile");
        let sms_config = default_profile.get("sms").expect("No sms config");

        if let Value::Dict(_, sms_dict) = sms_config {
            assert_eq!(sms_dict.get("grpc_addr").unwrap(), &Value::from("127.0.0.1:50053"));
            assert_eq!(sms_dict.get("http_addr").unwrap(), &Value::from("127.0.0.1:8082"));
            assert_eq!(sms_dict.get("heartbeat_timeout").unwrap(), &Value::from(240u64));
            assert_eq!(sms_dict.get("cleanup_interval").unwrap(), &Value::from(900u64));
            assert_eq!(sms_dict.get("enable_swagger").unwrap(), &Value::from(true));
        } else {
            panic!("Expected sms config to be a dictionary");
        }
    }

    /// Test CliArgs provider with no values / 测试没有值的CliArgs provider
    #[test]
    fn test_cli_args_provider_empty() {
        let args = CliArgs {
            config: None,
            grpc_addr: None,
            http_addr: None,
            heartbeat_timeout: None,
            cleanup_interval: None,
            enable_swagger: false, // false means it won't be included
            log_level: None,
            storage_backend: None,
            storage_path: None,
        };

        let data = args.data().expect("Failed to get provider data");
        let default_profile = data.get(&Profile::Default).expect("No default profile");
        
        // Should be empty since no values were provided
        // 应该为空，因为没有提供值
        assert!(default_profile.is_empty());
    }

    /// Test SmsConfig default values / 测试SmsConfig默认值
    #[test]
    fn test_sms_config_default() {
        let config = SmsConfig::default();
        
        assert_eq!(config.grpc_addr.to_string(), "0.0.0.0:50051");
        assert_eq!(config.http_addr.to_string(), "0.0.0.0:8080");
        assert_eq!(config.cleanup_interval, 300);
        assert_eq!(config.heartbeat_timeout, 120);
        assert!(config.enable_swagger);
    }

    /// Test AppConfig default values / 测试AppConfig默认值
    #[test]
    fn test_app_config_default() {
        let config = AppConfig::default();
        
        assert_eq!(config.sms.grpc_addr.to_string(), "0.0.0.0:50051");
        assert_eq!(config.sms.http_addr.to_string(), "0.0.0.0:8080");
        assert_eq!(config.sms.cleanup_interval, 300);
        assert_eq!(config.sms.heartbeat_timeout, 120);
        assert!(config.sms.enable_swagger);
    }

    /// Test configuration loading with CLI arguments / 测试使用CLI参数加载配置
    #[test]
    fn test_load_with_cli_args() {
        let args = CliArgs {
            config: None,
            grpc_addr: Some("127.0.0.1:50054".to_string()),
            http_addr: Some("127.0.0.1:8083".to_string()),
            heartbeat_timeout: Some(300),
            cleanup_interval: Some(1200),
            enable_swagger: false,
            log_level: Some("info".to_string()),
            storage_backend: None,
            storage_path: None,
        };

        let config = AppConfig::load_with_cli(&args).expect("Failed to load config");
        
        // CLI args should override defaults / CLI参数应该覆盖默认值
        assert_eq!(config.sms.grpc_addr.to_string(), "127.0.0.1:50054");
        assert_eq!(config.sms.http_addr.to_string(), "127.0.0.1:8083");
        assert_eq!(config.sms.heartbeat_timeout, 300);
        assert_eq!(config.sms.cleanup_interval, 1200);
        // enable_swagger should remain default (true) since CLI arg was false
        // enable_swagger应该保持默认值(true)，因为CLI参数为false
        assert!(config.sms.enable_swagger);
    }

    /// Test configuration loading with config file / 测试使用配置文件加载配置
    #[test]
    fn test_load_with_config_file() {
        // Create a temporary config file / 创建临时配置文件
        let config_content = r#"
[sms]
grpc_addr = "0.0.0.0:50055"
http_addr = "0.0.0.0:8084"
heartbeat_timeout = 180
cleanup_interval = 600
enable_swagger = false
"#;

        let temp_file = NamedTempFile::new().expect("Failed to create temp file");
        fs::write(temp_file.path(), config_content).expect("Failed to write config file");

        let args = CliArgs {
            config: Some(temp_file.path().to_string_lossy().to_string()),
            grpc_addr: None,
            http_addr: None,
            heartbeat_timeout: None,
            cleanup_interval: None,
            enable_swagger: false,
            log_level: None,
            storage_backend: None,
            storage_path: None,
        };

        let config = AppConfig::load_with_cli(&args).expect("Failed to load config");
        
        // Config file values should be loaded / 应该加载配置文件的值
        assert_eq!(config.sms.grpc_addr.to_string(), "0.0.0.0:50055");
        assert_eq!(config.sms.http_addr.to_string(), "0.0.0.0:8084");
        assert_eq!(config.sms.heartbeat_timeout, 180);
        assert_eq!(config.sms.cleanup_interval, 600);
        assert!(!config.sms.enable_swagger);
    }

    /// Test configuration priority: CLI > file / 测试配置优先级：CLI > 文件
    #[test]
    fn test_config_priority_cli_over_file() {
        // Create a temporary config file / 创建临时配置文件
        let config_content = r#"
[sms]
grpc_addr = "0.0.0.0:50055"
http_addr = "0.0.0.0:8084"
heartbeat_timeout = 180
cleanup_interval = 600
enable_swagger = false
"#;

        let temp_file = NamedTempFile::new().expect("Failed to create temp file");
        fs::write(temp_file.path(), config_content).expect("Failed to write config file");

        let args = CliArgs {
            config: Some(temp_file.path().to_string_lossy().to_string()),
            grpc_addr: Some("127.0.0.1:50056".to_string()), // Override file value
            http_addr: None, // Use file value
            heartbeat_timeout: Some(240), // Override file value
            cleanup_interval: None, // Use file value
            enable_swagger: true, // Override file value
            log_level: None,
            storage_backend: None,
            storage_path: None,
        };

        let config = AppConfig::load_with_cli(&args).expect("Failed to load config");
        
        // CLI args should override file values / CLI参数应该覆盖文件值
        assert_eq!(config.sms.grpc_addr.to_string(), "127.0.0.1:50056"); // CLI override
        assert_eq!(config.sms.http_addr.to_string(), "0.0.0.0:8084"); // From file
        assert_eq!(config.sms.heartbeat_timeout, 240); // CLI override
        assert_eq!(config.sms.cleanup_interval, 600); // From file
        assert!(config.sms.enable_swagger); // CLI override
    }

    /// Test configuration with environment variables / 测试环境变量配置
    #[test]
    fn test_config_with_env_vars() {
        // Test that config loads with default values when no env vars are set
        // 测试当没有设置环境变量时配置使用默认值
        let args = CliArgs {
            config: None,
            grpc_addr: None,
            http_addr: None,
            heartbeat_timeout: None,
            cleanup_interval: None,
            enable_swagger: false,
            log_level: None,
            storage_backend: None,
            storage_path: None,
        };

        let config = AppConfig::load_with_cli(&args).expect("Failed to load config");
        
        // Should use default values / 应该使用默认值
        assert_eq!(config.sms.grpc_addr.to_string(), "0.0.0.0:50051");
        assert_eq!(config.sms.http_addr.to_string(), "0.0.0.0:8080");
        assert_eq!(config.sms.heartbeat_timeout, 120);
        assert_eq!(config.sms.cleanup_interval, 300);
    }

    /// Test invalid configuration values / 测试无效配置值
    #[test]
    fn test_invalid_config_values() {
        let config_content = r#"
[sms]
grpc_addr = "invalid-address"
http_addr = "0.0.0.0:8080"
"#;

        let temp_file = NamedTempFile::new().expect("Failed to create temp file");
        fs::write(temp_file.path(), config_content).expect("Failed to write config file");

        let args = CliArgs {
            config: Some(temp_file.path().to_string_lossy().to_string()),
            grpc_addr: None,
            http_addr: None,
            heartbeat_timeout: None,
            cleanup_interval: None,
            enable_swagger: false,
            log_level: None,
            storage_backend: None,
            storage_path: None,
        };

        // Should fail due to invalid address / 应该因为无效地址而失败
        let result = AppConfig::load_with_cli(&args);
        assert!(result.is_err());
    }

    /// Test provider metadata / 测试provider元数据
    #[test]
    fn test_cli_args_provider_metadata() {
        let args = CliArgs {
            config: None,
            grpc_addr: None,
            http_addr: None,
            heartbeat_timeout: None,
            cleanup_interval: None,
            enable_swagger: false,
            log_level: None,
            storage_backend: None,
            storage_path: None,
        };

        let metadata = args.metadata();
        assert_eq!(metadata.name.as_ref(), "CLI Arguments");
    }

    /// Test CLI storage backend configuration / 测试CLI存储后端配置
    #[test]
    fn test_cli_storage_backend_config() {
        // Test with storage backend only / 仅测试存储后端
        let args = CliArgs {
            config: None,
            grpc_addr: None,
            http_addr: None,
            heartbeat_timeout: None,
            cleanup_interval: None,
            enable_swagger: false,
            log_level: None,
            storage_backend: Some("sled".to_string()),
            storage_path: None,
        };

        let config = AppConfig::load_with_cli(&args).unwrap();
        assert_eq!(config.sms.kv_store.backend, "sled");

        // Test with both backend and path / 测试后端和路径
        let args = CliArgs {
            config: None,
            grpc_addr: None,
            http_addr: None,
            heartbeat_timeout: None,
            cleanup_interval: None,
            enable_swagger: false,
            log_level: None,
            storage_backend: Some("sled".to_string()),
            storage_path: Some("/tmp/test.db".to_string()),
        };

        let config = AppConfig::load_with_cli(&args).unwrap();
        assert_eq!(config.sms.kv_store.backend, "sled");
        assert_eq!(config.sms.kv_store.get_param("path"), Some(&"/tmp/test.db".to_string()));

        // Test with path only (should use default backend) / 仅测试路径（应使用默认后端）
        let args = CliArgs {
            config: None,
            grpc_addr: None,
            http_addr: None,
            heartbeat_timeout: None,
            cleanup_interval: None,
            enable_swagger: false,
            log_level: None,
            storage_backend: None,
            storage_path: Some("/tmp/test.db".to_string()),
        };

        let config = AppConfig::load_with_cli(&args).unwrap();
        assert_eq!(config.sms.kv_store.get_param("path"), Some(&"/tmp/test.db".to_string()));
    }
}