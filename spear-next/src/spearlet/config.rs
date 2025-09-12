//! SPEARlet configuration / SPEARlet配置

use clap::{ArgAction, Parser};
use serde::{Deserialize, Serialize};

/// SPEARlet command line arguments / SPEARlet命令行参数
#[derive(Parser, Debug, Clone)]
#[command(
    name = "spearlet",
    version = "0.1.0",
    about = "SPEARlet - SPEAR core agent component\nSPEARlet - SPEAR核心代理组件",
long_about = "SPEARlet is the core agent component of SPEAR project, similar to kubelet in Kubernetes.\nSPEARlet是SPEAR项目的核心代理组件，类似于Kubernetes中的kubelet。"
)]
pub struct CliArgs {
    /// Configuration file path / 配置文件路径
    #[arg(short, long, value_name = "FILE", help = "Configuration file path / 配置文件路径")]
    pub config: Option<String>,

    /// Node ID / 节点ID
    #[arg(long, value_name = "ID", help = "Node identifier / 节点标识符")]
    pub node_id: Option<String>,

    /// gRPC server address / gRPC服务器地址
    #[arg(long, value_name = "ADDR", help = "gRPC server address (e.g., 0.0.0.0:50052) / gRPC服务器地址")]
    pub grpc_addr: Option<String>,

    /// HTTP gateway address / HTTP网关地址
    #[arg(long, value_name = "ADDR", help = "HTTP gateway address (e.g., 0.0.0.0:8081) / HTTP网关地址")]
    pub http_addr: Option<String>,

    /// SMS service address / SMS服务地址
    #[arg(long, value_name = "ADDR", help = "SMS service address (e.g., 127.0.0.1:50051) / SMS服务地址")]
    pub sms_addr: Option<String>,

    /// Storage backend type / 存储后端类型
    #[arg(long, value_name = "BACKEND", help = "Storage backend type (memory, sled, rocksdb) / 存储后端类型")]
    pub storage_backend: Option<String>,

    /// Storage data directory / 存储数据目录
    #[arg(long, value_name = "PATH", help = "Storage data directory / 存储数据目录")]
    pub storage_path: Option<String>,

    /// Auto register with SMS / 自动向SMS注册
    #[arg(long, help = "Auto register with SMS / 自动向SMS注册")]
    pub auto_register: Option<bool>,

    /// Log level / 日志级别
    #[arg(long, value_name = "LEVEL", help = "Log level (trace, debug, info, warn, error) / 日志级别")]
    pub log_level: Option<String>,
}

/// Spearlet application configuration / Spearlet应用配置
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AppConfig {
    /// SPEARlet service configuration / SPEARlet服务配置
    pub spearlet: SpearletConfig,
}

impl AppConfig {
    /// Load configuration with CLI arguments / 使用CLI参数加载配置
    pub fn load_with_cli(args: &CliArgs) -> Result<Self, Box<dyn std::error::Error + Send + Sync>> {
        let mut config = AppConfig::default();
        
        // Load from config file if provided / 如果提供了配置文件则从文件加载
        if let Some(config_path) = &args.config {
            let config_content = std::fs::read_to_string(config_path)?;
            config = toml::from_str(&config_content)?;
        }
        
        // Override with CLI arguments / 使用CLI参数覆盖
        if let Some(node_id) = &args.node_id {
            config.spearlet.node_id = node_id.clone();
        }
        
        if let Some(grpc_addr) = &args.grpc_addr {
            let parts: Vec<&str> = grpc_addr.split(':').collect();
            if parts.len() == 2 {
                config.spearlet.grpc.address = parts[0].to_string();
                config.spearlet.grpc.port = parts[1].parse()?;
            }
        }
        
        if let Some(http_addr) = &args.http_addr {
            let parts: Vec<&str> = http_addr.split(':').collect();
            if parts.len() == 2 {
                config.spearlet.http.address = parts[0].to_string();
                config.spearlet.http.port = parts[1].parse()?;
            }
        }
        
        if let Some(sms_addr) = &args.sms_addr {
            config.spearlet.sms_addr = sms_addr.clone();
        }
        
        if let Some(storage_backend) = &args.storage_backend {
            config.spearlet.storage.backend = storage_backend.clone();
        }
        
        if let Some(storage_path) = &args.storage_path {
            config.spearlet.storage.data_dir = storage_path.clone();
        }
        
        if let Some(auto_register) = args.auto_register {
            config.spearlet.auto_register = auto_register;
        }
        
        if let Some(log_level) = &args.log_level {
            config.spearlet.logging.level = log_level.clone();
        }
        
        Ok(config)
    }
}

impl Default for AppConfig {
    fn default() -> Self {
        Self {
            spearlet: SpearletConfig::default(),
        }
    }
}

/// SPEARlet service configuration / SPEARlet服务配置
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SpearletConfig {
    /// Node identifier / 节点标识符
    pub node_id: String,
    /// gRPC server configuration / gRPC服务器配置
    pub grpc: GrpcConfig,
    /// HTTP gateway configuration / HTTP网关配置
    pub http: HttpConfig,
    /// Storage configuration / 存储配置
    pub storage: StorageConfig,
    /// Logging configuration / 日志配置
    pub logging: LoggingConfig,
    /// SMS service address / SMS服务地址
    pub sms_addr: String,
    /// Auto register with SMS / 自动向SMS注册
    pub auto_register: bool,
    /// Heartbeat interval in seconds / 心跳间隔(秒)
    pub heartbeat_interval: u64,
    /// Cleanup interval in seconds / 清理间隔(秒)
    pub cleanup_interval: u64,
}

/// gRPC server configuration / gRPC服务器配置
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct GrpcConfig {
    /// Server bind address / 服务器绑定地址
    pub address: String,
    /// Server port / 服务器端口
    pub port: u16,
    /// Enable TLS / 启用TLS
    pub tls_enabled: bool,
    /// TLS certificate file path / TLS证书文件路径
    pub tls_cert_path: Option<String>,
    /// TLS private key file path / TLS私钥文件路径
    pub tls_key_path: Option<String>,
}

/// HTTP gateway configuration / HTTP网关配置
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct HttpConfig {
    /// Server bind address / 服务器绑定地址
    pub address: String,
    /// Server port / 服务器端口
    pub port: u16,
    /// Enable CORS / 启用CORS
    pub cors_enabled: bool,
    /// Enable Swagger UI / 启用Swagger UI
    pub swagger_enabled: bool,
}

/// Storage configuration / 存储配置
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct StorageConfig {
    /// Storage backend type / 存储后端类型
    pub backend: String,
    /// Storage data directory / 存储数据目录
    pub data_dir: String,
    /// Maximum cache size in MB / 最大缓存大小(MB)
    pub max_cache_size_mb: u64,
    /// Enable compression / 启用压缩
    pub compression_enabled: bool,
    /// Maximum object size in bytes / 最大对象大小(字节)
    pub max_object_size: u64,
}

/// Logging configuration / 日志配置
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct LoggingConfig {
    /// Log level / 日志级别
    pub level: String,
    /// Log format / 日志格式
    pub format: String,
    /// Log output file / 日志输出文件
    pub output_file: Option<String>,
}

impl Default for SpearletConfig {
    fn default() -> Self {
        Self {
            node_id: "spearlet-node".to_string(),
            grpc: GrpcConfig::default(),
            http: HttpConfig::default(),
            storage: StorageConfig::default(),
            logging: LoggingConfig::default(),
            sms_addr: "127.0.0.1:50051".to_string(),
            auto_register: false,
            heartbeat_interval: 30,
            cleanup_interval: 300,
        }
    }
}

impl Default for GrpcConfig {
    fn default() -> Self {
        Self {
            address: "0.0.0.0".to_string(),
            port: 50052,
            tls_enabled: false,
            tls_cert_path: None,
            tls_key_path: None,
        }
    }
}

impl Default for HttpConfig {
    fn default() -> Self {
        Self {
            address: "0.0.0.0".to_string(),
            port: 8081,
            cors_enabled: true,
            swagger_enabled: true,
        }
    }
}

impl Default for StorageConfig {
    fn default() -> Self {
        Self {
            backend: "rocksdb".to_string(),
            data_dir: "./data/spearlet".to_string(),
            max_cache_size_mb: 512,
            compression_enabled: true,
            max_object_size: 64 * 1024 * 1024, // 64MB default / 默认64MB
        }
    }
}

impl Default for LoggingConfig {
    fn default() -> Self {
        Self {
            level: "info".to_string(),
            format: "json".to_string(),
            output_file: None,
        }
    }
}