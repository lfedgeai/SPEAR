//! SPEARlet configuration / SPEARlet配置

use clap::Parser;
use serde::{Deserialize, Serialize};

use crate::config;
use crate::config::base::{ServerConfig, LogConfig};

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

        // Try loading from home directory first / 优先从用户主目录加载配置
        // Home path: ~/.spear/config.toml
        // 主目录路径：~/.spear/config.toml
        if args.config.is_none() {
            // Prefer SPEAR_HOME if set to avoid interfering with global HOME in tests
            // 若设置了SPEAR_HOME则优先使用，避免测试中修改全局HOME产生干扰
            let base_home = std::env::var_os("SPEAR_HOME").or_else(|| std::env::var_os("HOME"));
            if let Some(home_dir) = base_home {
                let home_path = std::path::PathBuf::from(home_dir)
                    .join(".spear")
                    .join("config.toml");
                if home_path.exists() {
                    let cfg = std::fs::read_to_string(&home_path)?;
                    match toml::from_str::<AppConfig>(&cfg) {
                        Ok(c) => { config = c; }
                        Err(e) => {
                            tracing::warn!("Failed to parse home config: {}", e);
                            // fall back to defaults / 回退到默认值
                        }
                    }
                }
            }
        }

        // If home config not found, load from CLI-provided path if any
        // 如果未找到主目录配置，则从命令行提供的路径加载
        if args.config.is_some() {
            if let Some(config_path) = &args.config {
                let config_content = std::fs::read_to_string(config_path)?;
                config = toml::from_str(&config_content)?;
            }
        }
        
        // Override with CLI arguments / 使用CLI参数覆盖
        if let Some(node_id) = &args.node_id {
            config.spearlet.node_id = node_id.clone();
        }
        
        if let Some(grpc_addr) = &args.grpc_addr {
            config.spearlet.grpc.addr = grpc_addr.parse()?;
        }
        
        if let Some(http_addr) = &args.http_addr {
            config.spearlet.http.server.addr = http_addr.parse()?;
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
#[serde(default)]
pub struct SpearletConfig {
    /// Node identifier / 节点标识符
    pub node_id: String,
    /// gRPC server configuration / gRPC服务器配置
    pub grpc: ServerConfig,
    /// HTTP gateway configuration / HTTP网关配置
    pub http: HttpConfig,
    /// Storage configuration / 存储配置
    pub storage: StorageConfig,
    /// Logging configuration / 日志配置
    pub logging: LogConfig,
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
// gRPC uses base ServerConfig / gRPC使用基础ServerConfig

/// HTTP gateway configuration / HTTP网关配置
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(default)]
pub struct HttpConfig {
    /// HTTP server settings / HTTP服务器设置
    pub server: ServerConfig,
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
            grpc: ServerConfig { addr: "0.0.0.0:50052".parse().unwrap(), ..Default::default() },
            http: HttpConfig::default(),
            storage: StorageConfig::default(),
            logging: LogConfig::default(),
            sms_addr: "127.0.0.1:50051".to_string(),
            auto_register: false,
            heartbeat_interval: 30,
            cleanup_interval: 300,
        }
    }
}

// Grpc defaults provided via ServerConfig::default with override in SpearletConfig

impl Default for HttpConfig {
    fn default() -> Self {
        Self {
            server: ServerConfig { addr: "0.0.0.0:8081".parse().unwrap(), ..Default::default() },
            cors_enabled: true,
            swagger_enabled: true,
        }
    }
}

impl Default for StorageConfig {
    fn default() -> Self {
        Self {
            backend: "memory".to_string(), // Changed from rocksdb to memory / 从rocksdb改为memory
            data_dir: "./data/spearlet".to_string(),
            max_cache_size_mb: 512,
            compression_enabled: true,
            max_object_size: 64 * 1024 * 1024, // 64MB default / 默认64MB
        }
    }
}

// Logging uses base LogConfig / 日志使用基础LogConfig
