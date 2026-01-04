//! SPEARlet configuration / SPEARlet配置

use clap::Parser;
use serde::{Deserialize, Serialize};

use crate::config::base::{LogConfig, ServerConfig};

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
    #[arg(
        short,
        long,
        value_name = "FILE",
        help = "Configuration file path / 配置文件路径"
    )]
    pub config: Option<String>,

    /// Node name / 节点名称
    #[arg(
        long,
        value_name = "NAME",
        help = "Node name (not unique) / 节点名称（可重复）"
    )]
    pub node_name: Option<String>,

    /// gRPC server address / gRPC服务器地址
    #[arg(
        long,
        value_name = "ADDR",
        help = "gRPC server address (e.g., 0.0.0.0:50052) / gRPC服务器地址"
    )]
    pub grpc_addr: Option<String>,

    /// HTTP gateway address / HTTP网关地址
    #[arg(
        long,
        value_name = "ADDR",
        help = "HTTP gateway address (e.g., 0.0.0.0:8081) / HTTP网关地址"
    )]
    pub http_addr: Option<String>,

    /// SMS service address / SMS服务地址
    #[arg(
        long = "sms-grpc-addr",
        value_name = "ADDR",
        help = "SMS gRPC address (e.g., 127.0.0.1:50051) / SMS gRPC地址"
    )]
    pub sms_grpc_addr: Option<String>,

    /// SMS HTTP gateway address / SMS HTTP网关地址
    #[arg(
        long,
        value_name = "ADDR",
        help = "SMS HTTP gateway address (e.g., 127.0.0.1:8080) / SMS HTTP网关地址"
    )]
    pub sms_http_addr: Option<String>,

    /// Storage backend type / 存储后端类型
    #[arg(
        long,
        value_name = "BACKEND",
        help = "Storage backend type (memory, sled, rocksdb) / 存储后端类型"
    )]
    pub storage_backend: Option<String>,

    /// Storage data directory / 存储数据目录
    #[arg(
        long,
        value_name = "PATH",
        help = "Storage data directory / 存储数据目录"
    )]
    pub storage_path: Option<String>,

    /// Auto register with SMS / 自动向SMS注册
    #[arg(long, help = "Auto register with SMS / 自动向SMS注册")]
    pub auto_register: Option<bool>,

    /// Log level / 日志级别
    #[arg(
        long,
        value_name = "LEVEL",
        help = "Log level (trace, debug, info, warn, error) / 日志级别"
    )]
    pub log_level: Option<String>,

    #[arg(long, value_name = "MS")]
    pub sms_connect_timeout_ms: Option<u64>,

    #[arg(long, value_name = "MS")]
    pub sms_connect_retry_ms: Option<u64>,

    /// Total reconnect timeout after disconnection / 断线后的总重连超时（毫秒）
    #[arg(
        long,
        value_name = "MS",
        help = "Total reconnect timeout after disconnect / 断线后的总重连超时（毫秒）"
    )]
    pub reconnect_total_timeout_ms: Option<u64>,
}

/// Spearlet application configuration / Spearlet应用配置
#[derive(Debug, Clone, Serialize, Deserialize, Default)]
pub struct AppConfig {
    /// SPEARlet service configuration / SPEARlet服务配置
    pub spearlet: SpearletConfig,
}

impl AppConfig {
    /// Load configuration with CLI arguments / 使用CLI参数加载配置
    pub fn load_with_cli(args: &CliArgs) -> Result<Self, Box<dyn std::error::Error + Send + Sync>> {
        let mut config = AppConfig::default();

        // Apply environment variables (low priority) / 应用环境变量（较低优先级）
        // Prefix: SPEARLET_  例如：SPEARLET_GRPC_ADDR, SPEARLET_HTTP_ADDR
        if let Ok(v) = std::env::var("SPEARLET_NODE_NAME") {
            if !v.is_empty() {
                config.spearlet.node_name = v;
            }
        }
        if let Ok(v) = std::env::var("SPEARLET_SMS_GRPC_ADDR") {
            if !v.is_empty() {
                config.spearlet.sms_grpc_addr = v;
            }
        }
        if let Ok(v) = std::env::var("SPEARLET_SMS_HTTP_ADDR") {
            if !v.is_empty() {
                config.spearlet.sms_http_addr = v;
            }
        }
        if let Ok(v) = std::env::var("SPEARLET_AUTO_REGISTER") {
            if let Ok(b) = v.parse::<bool>() {
                config.spearlet.auto_register = b;
            }
        }
        if let Ok(v) = std::env::var("SPEARLET_HEARTBEAT_INTERVAL") {
            if let Ok(n) = v.parse::<u64>() {
                config.spearlet.heartbeat_interval = n;
            }
        }
        if let Ok(v) = std::env::var("SPEARLET_CLEANUP_INTERVAL") {
            if let Ok(n) = v.parse::<u64>() {
                config.spearlet.cleanup_interval = n;
            }
        }

        if let Ok(v) = std::env::var("SPEARLET_GRPC_ADDR") {
            if let Ok(a) = v.parse::<std::net::SocketAddr>() {
                config.spearlet.grpc.addr = a;
            }
        }
        if let Ok(v) = std::env::var("SPEARLET_HTTP_ADDR") {
            if let Ok(a) = v.parse::<std::net::SocketAddr>() {
                config.spearlet.http.server.addr = a;
            }
        }

        if let Ok(v) = std::env::var("SPEARLET_STORAGE_BACKEND") {
            if !v.is_empty() {
                config.spearlet.storage.backend = v;
            }
        }
        if let Ok(v) = std::env::var("SPEARLET_STORAGE_DATA_DIR") {
            if !v.is_empty() {
                config.spearlet.storage.data_dir = v;
            }
        }
        if let Ok(v) = std::env::var("SPEARLET_STORAGE_MAX_CACHE_MB") {
            if let Ok(n) = v.parse::<u64>() {
                config.spearlet.storage.max_cache_size_mb = n;
            }
        }
        if let Ok(v) = std::env::var("SPEARLET_STORAGE_COMPRESSION_ENABLED") {
            if let Ok(b) = v.parse::<bool>() {
                config.spearlet.storage.compression_enabled = b;
            }
        }
        if let Ok(v) = std::env::var("SPEARLET_STORAGE_MAX_OBJECT_SIZE") {
            if let Ok(n) = v.parse::<u64>() {
                config.spearlet.storage.max_object_size = n;
            }
        }

        if let Ok(v) = std::env::var("SPEARLET_LOG_LEVEL") {
            if !v.is_empty() {
                config.spearlet.logging.level = v;
            }
        }
        if let Ok(v) = std::env::var("SPEARLET_LOG_FORMAT") {
            if !v.is_empty() {
                config.spearlet.logging.format = v;
            }
        }
        if let Ok(v) = std::env::var("SPEARLET_LOG_FILE") {
            if !v.is_empty() {
                config.spearlet.logging.file = Some(v);
            }
        }

        if let Ok(v) = std::env::var("SPEARLET_SMS_CONNECT_TIMEOUT_MS") {
            if let Ok(n) = v.parse::<u64>() {
                config.spearlet.sms_connect_timeout_ms = n;
            }
        }
        if let Ok(v) = std::env::var("SPEARLET_SMS_CONNECT_RETRY_MS") {
            if let Ok(n) = v.parse::<u64>() {
                config.spearlet.sms_connect_retry_ms = n;
            }
        }
        if let Ok(v) = std::env::var("SPEARLET_RECONNECT_TOTAL_TIMEOUT_MS") {
            if let Ok(n) = v.parse::<u64>() {
                config.spearlet.reconnect_total_timeout_ms = n;
            }
        }

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
                        Ok(c) => {
                            config = c;
                        }
                        Err(e) => {
                            tracing::warn!("Failed to parse home config: {}", e);
                            // fall back to defaults / 回退到默认值
                        }
                    }
                }
            }
        }

        // Load from CLI-provided path (highest file priority) / 从命令行提供的路径加载（文件最高优先级）
        if let Some(config_path) = &args.config {
            let config_content = std::fs::read_to_string(config_path)?;
            config = toml::from_str(&config_content)?;
        }

        // Override with CLI arguments / 使用CLI参数覆盖
        if let Some(node_name) = &args.node_name {
            config.spearlet.node_name = node_name.clone();
        }

        if let Some(grpc_addr) = &args.grpc_addr {
            config.spearlet.grpc.addr = grpc_addr.parse()?;
        }

        if let Some(http_addr) = &args.http_addr {
            config.spearlet.http.server.addr = http_addr.parse()?;
        }

        if let Some(sms_grpc_addr) = &args.sms_grpc_addr {
            config.spearlet.sms_grpc_addr = sms_grpc_addr.clone();
        }
        if let Some(sms_http_addr) = &args.sms_http_addr {
            config.spearlet.sms_http_addr = sms_http_addr.clone();
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

        if let Some(t) = args.sms_connect_timeout_ms {
            config.spearlet.sms_connect_timeout_ms = t;
        }
        if let Some(r) = args.sms_connect_retry_ms {
            config.spearlet.sms_connect_retry_ms = r;
        }
        if let Some(rt) = args.reconnect_total_timeout_ms {
            config.spearlet.reconnect_total_timeout_ms = rt;
        }

        // Implicit auto-register rule: when SMS address is provided via CLI or env, enable auto_register by default
        // 隐式自动注册规则：当通过CLI或环境变量提供了SMS地址时，默认启用auto_register
        let env_sms = std::env::var("SPEARLET_SMS_GRPC_ADDR")
            .ok()
            .map(|v| !v.is_empty())
            .unwrap_or(false);
        let cli_sms = args
            .sms_grpc_addr
            .as_ref()
            .map(|v| !v.is_empty())
            .unwrap_or(false);
        if args.auto_register.is_none() && (env_sms || cli_sms) {
            config.spearlet.auto_register = true;
        }

        Ok(config)
    }
}

/// SPEARlet service configuration / SPEARlet服务配置
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(default)]
pub struct SpearletConfig {
    /// Node name / 节点名称
    pub node_name: String,
    /// gRPC server configuration / gRPC服务器配置
    pub grpc: ServerConfig,
    /// HTTP gateway configuration / HTTP网关配置
    pub http: HttpConfig,
    /// Storage configuration / 存储配置
    pub storage: StorageConfig,
    /// Logging configuration / 日志配置
    pub logging: LogConfig,
    /// SMS service address / SMS服务地址
    pub sms_grpc_addr: String,
    /// SMS HTTP gateway address / SMS HTTP网关地址
    pub sms_http_addr: String,
    /// Auto register with SMS / 自动向SMS注册
    pub auto_register: bool,
    /// Heartbeat interval in seconds / 心跳间隔(秒)
    pub heartbeat_interval: u64,
    /// Cleanup interval in seconds / 清理间隔(秒)
    pub cleanup_interval: u64,
    pub sms_connect_timeout_ms: u64,
    pub sms_connect_retry_ms: u64,
    /// Total reconnect timeout after disconnection / 断线后的总重连超时（毫秒）
    pub reconnect_total_timeout_ms: u64,
    pub llm: LlmConfig,
}

#[derive(Debug, Clone, Serialize, Deserialize, Default)]
#[serde(default)]
pub struct LlmConfig {
    pub default_policy: Option<String>,
    pub backends: Vec<LlmBackendConfig>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(default)]
pub struct LlmBackendConfig {
    pub name: String,
    pub kind: String,
    pub base_url: String,
    pub api_key_env: Option<String>,
    pub weight: u32,
    pub priority: i32,
    pub ops: Vec<String>,
    pub features: Vec<String>,
    pub transports: Vec<String>,
}

impl Default for LlmBackendConfig {
    fn default() -> Self {
        Self {
            name: String::new(),
            kind: String::new(),
            base_url: String::new(),
            api_key_env: None,
            weight: 100,
            priority: 0,
            ops: Vec::new(),
            features: Vec::new(),
            transports: Vec::new(),
        }
    }
}

// gRPC server configuration / gRPC服务器配置
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
            node_name: "spearlet-node".to_string(),
            grpc: ServerConfig {
                addr: "0.0.0.0:50052".parse().unwrap(),
                ..Default::default()
            },
            http: HttpConfig::default(),
            storage: StorageConfig::default(),
            logging: LogConfig::default(),
            sms_grpc_addr: "127.0.0.1:50051".to_string(),
            sms_http_addr: "127.0.0.1:8080".to_string(),
            auto_register: false,
            heartbeat_interval: 30,
            cleanup_interval: 300,
            sms_connect_timeout_ms: 15000,
            sms_connect_retry_ms: 500,
            reconnect_total_timeout_ms: 300_000,
            llm: LlmConfig::default(),
        }
    }
}

// Grpc defaults provided via ServerConfig::default with override in SpearletConfig

impl Default for HttpConfig {
    fn default() -> Self {
        Self {
            server: ServerConfig {
                addr: "0.0.0.0:8081".parse().unwrap(),
                ..Default::default()
            },
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
