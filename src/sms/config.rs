//! SMS service configuration
//! SMS服务配置

use crate::config::base::{LogConfig, ServerConfig};
use crate::storage::kv::KvStoreConfig;
use anyhow;
use clap::Parser;
use serde::{Deserialize, Serialize};

/// SMS command line arguments / SMS命令行参数
#[derive(Parser, Debug, Clone)]
#[command(
    name = "sms",
    version = "0.1.0",
    about = "SMS - SPEAR Metadata Server\nSMS - SPEAR元数据服务器",
    long_about = "SMS provides centralized management for SPEARlet nodes and storage resources.\nSMS为SPEARlet节点和存储资源提供集中管理。"
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

    /// gRPC server address / gRPC服务器地址
    #[arg(
        long,
        value_name = "ADDR",
        help = "gRPC server address (e.g., 0.0.0.0:50051) / gRPC服务器地址"
    )]
    pub grpc_addr: Option<String>,

    /// HTTP gateway address / HTTP网关地址
    #[arg(
        long,
        value_name = "ADDR",
        help = "HTTP gateway address (e.g., 0.0.0.0:8080) / HTTP网关地址"
    )]
    pub http_addr: Option<String>,

    /// Database type / 数据库类型
    #[arg(
        long,
        value_name = "TYPE",
        help = "Database type (sled, rocksdb) / 数据库类型"
    )]
    pub db_type: Option<String>,

    /// Database path / 数据库路径
    #[arg(long, value_name = "PATH", help = "Database path / 数据库路径")]
    pub db_path: Option<String>,

    /// Database pool size / 数据库连接池大小
    #[arg(
        long,
        value_name = "SIZE",
        help = "Database connection pool size / 数据库连接池大小"
    )]
    pub db_pool_size: Option<u32>,

    /// Enable Swagger UI / 启用Swagger UI
    #[arg(long, help = "Enable Swagger UI / 启用Swagger UI")]
    pub enable_swagger: bool,

    /// Disable Swagger UI / 禁用Swagger UI
    #[arg(
        long,
        help = "Disable Swagger UI / 禁用Swagger UI",
        conflicts_with = "enable_swagger"
    )]
    pub disable_swagger: bool,

    /// Log level / 日志级别
    #[arg(
        long,
        value_name = "LEVEL",
        help = "Log level (trace, debug, info, warn, error) / 日志级别"
    )]
    pub log_level: Option<String>,

    #[arg(long, value_name = "FORMAT")]
    pub log_format: Option<String>,

    #[arg(long, value_name = "FILE")]
    pub log_file: Option<String>,

    /// Heartbeat timeout in seconds / 心跳超时时间（秒）
    #[arg(
        long,
        value_name = "SECONDS",
        help = "Heartbeat timeout in seconds / 心跳超时时间（秒）"
    )]
    pub heartbeat_timeout: Option<u64>,

    /// Cleanup interval in seconds / 清理间隔时间（秒）
    #[arg(
        long,
        value_name = "SECONDS",
        help = "Cleanup interval in seconds / 清理间隔时间（秒）"
    )]
    pub cleanup_interval: Option<u64>,
    /// Periodic task assignment reconcile interval in seconds / 周期性任务分配收敛间隔（秒）
    #[arg(
        long,
        value_name = "SECONDS",
        help = "Periodic task assignment reconcile interval in seconds / 周期性任务分配收敛间隔（秒）"
    )]
    pub assignment_reconcile_interval: Option<u64>,
    /// Max upload size in bytes / 最大上传字节数
    #[arg(
        long,
        value_name = "BYTES",
        help = "Max upload bytes for embedded object store / 内嵌对象存储的最大上传字节数"
    )]
    pub max_upload_bytes: Option<u64>,

    /// Enable Web Admin / 启用Web管理页面
    #[arg(long, help = "Enable Web Admin / 启用Web管理页面")]
    pub enable_web_admin: bool,

    /// Disable Web Admin / 禁用Web管理页面
    #[arg(
        long,
        help = "Disable Web Admin / 禁用Web管理页面",
        conflicts_with = "enable_web_admin"
    )]
    pub disable_web_admin: bool,

    /// Enable SPEAR Console / 启用控制台
    #[arg(long, help = "Enable SPEAR Console / 启用控制台")]
    pub enable_console: bool,

    /// Disable SPEAR Console / 禁用控制台
    #[arg(
        long,
        help = "Disable SPEAR Console / 禁用控制台",
        conflicts_with = "enable_console"
    )]
    pub disable_console: bool,

    /// Web Admin address / Web管理页面监听地址
    #[arg(
        long,
        value_name = "ADDR",
        help = "Web Admin address (e.g., 0.0.0.0:8081) / Web管理页面监听地址"
    )]
    pub web_admin_addr: Option<String>,

    #[arg(
        long,
        value_name = "PATH",
        help = "MCP server configs directory / MCP server 配置目录"
    )]
    pub mcp_dir: Option<String>,

    #[arg(
        long,
        value_name = "PATH",
        help = "Execution logs directory / 执行日志存储目录"
    )]
    pub execution_logs_dir: Option<String>,
}

/// SMS service configuration / SMS服务配置
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(default)]
pub struct SmsConfig {
    /// gRPC server configuration / gRPC服务器配置
    pub grpc: ServerConfig,
    /// HTTP gateway configuration / HTTP网关配置
    pub http: ServerConfig,
    pub logging: LogConfig,
    /// Enable Swagger UI / 启用Swagger UI
    pub enable_swagger: bool,
    /// Database configuration / 数据库配置
    pub database: DatabaseConfig,
    /// Enable Web Admin / 启用Web管理页面
    pub enable_web_admin: bool,
    /// Enable SPEAR Console (/console) / 启用控制台（/console）
    pub enable_console: bool,
    /// Web Admin server configuration / Web管理页面服务器配置
    pub web_admin: ServerConfig,
    /// Heartbeat timeout in seconds / 心跳超时时间（秒）
    pub heartbeat_timeout: u64,
    /// Cleanup interval in seconds / 清理间隔时间（秒）
    pub cleanup_interval: u64,
    /// Periodic task assignment reconcile interval in seconds / 周期性任务分配收敛间隔（秒）
    pub assignment_reconcile_interval: u64,
    /// Max upload size in bytes for embedded object store / 内嵌对象存储的最大上传字节数
    pub max_upload_bytes: u64,
    /// Structured object storage backend configuration / 结构化对象存储后端配置
    pub object_storage: ObjectStorageConfig,
    /// Execution logs directory / 执行日志目录
    pub execution_logs_dir: String,
    /// Optional KV config for task events outbox / 任务事件Outbox的可选KV配置
    pub event_kv: Option<KvStoreConfig>,

    pub mcp: McpConfig,
}

/// Object storage backend kind / 对象存储后端类型
#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq, Eq, Default)]
#[serde(rename_all = "snake_case")]
pub enum ObjectStorageBackend {
    /// Local filesystem compatibility backend / 本地文件系统兼容后端
    #[default]
    LocalFs,
    /// RustFS blob backend / RustFS Blob 后端
    Rustfs,
}

/// Structured object storage configuration / 结构化对象存储配置
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(default)]
pub struct ObjectStorageConfig {
    /// Selected backend implementation / 选中的后端实现
    pub backend: ObjectStorageBackend,
    /// Local filesystem backend settings / 本地文件系统后端配置
    pub local_fs: LocalFsObjectStorageConfig,
    /// RustFS backend settings / RustFS 后端配置
    pub rustfs: RustfsObjectStorageConfig,
}

/// Local filesystem blob store settings / 本地文件系统 BlobStore 配置
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(default)]
pub struct LocalFsObjectStorageConfig {
    /// Root directory used by the local compatibility store / 本地兼容存储使用的根目录
    pub root_dir: String,
}

/// RustFS blob store settings / RustFS BlobStore 配置
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(default)]
pub struct RustfsObjectStorageConfig {
    /// RustFS service endpoint / RustFS 服务地址
    pub endpoint: String,
    /// RustFS bucket or namespace / RustFS bucket 或命名空间
    pub bucket: String,
    /// Optional key prefix inside the bucket / bucket 内的可选前缀
    pub root: String,
    /// S3-compatible region for RustFS requests / RustFS 请求使用的 S3-compatible region
    pub region: String,
    /// Optional static access key, empty means using the default credential chain / 可选静态 access key，留空表示使用默认凭证链
    pub access_key: String,
    /// Optional static secret key, empty means using the default credential chain / 可选静态 secret key，留空表示使用默认凭证链
    pub secret_key: String,
    /// Use S3 path-style addressing for RustFS / 对 RustFS 使用 S3 path-style 寻址
    pub path_style: bool,
    /// Connect timeout in milliseconds / 连接超时时间（毫秒）
    pub connect_timeout_ms: u64,
    /// Read timeout in milliseconds / 读取超时时间（毫秒）
    pub read_timeout_ms: u64,
    /// Write timeout in milliseconds / 写入超时时间（毫秒）
    pub write_timeout_ms: u64,
    /// Create bucket automatically when missing / 缺失时自动创建 bucket
    pub create_bucket_if_missing: bool,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(default)]
pub struct McpConfig {
    pub dir: String,
}

impl Default for McpConfig {
    fn default() -> Self {
        Self { dir: String::new() }
    }
}

impl Default for ObjectStorageConfig {
    fn default() -> Self {
        Self {
            backend: ObjectStorageBackend::LocalFs,
            local_fs: LocalFsObjectStorageConfig::default(),
            rustfs: RustfsObjectStorageConfig::default(),
        }
    }
}

impl Default for LocalFsObjectStorageConfig {
    fn default() -> Self {
        Self {
            root_dir: "./data/objects".to_string(),
        }
    }
}

impl Default for RustfsObjectStorageConfig {
    fn default() -> Self {
        Self {
            endpoint: String::new(),
            bucket: String::new(),
            root: String::new(),
            region: "us-east-1".to_string(),
            access_key: String::new(),
            secret_key: String::new(),
            path_style: true,
            connect_timeout_ms: 3000,
            read_timeout_ms: 10000,
            write_timeout_ms: 10000,
            create_bucket_if_missing: true,
        }
    }
}

/// Database configuration / 数据库配置
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(default)]
pub struct DatabaseConfig {
    /// Database type (rocksdb, sled) / 数据库类型
    pub db_type: String,
    /// Database path / 数据库路径
    pub path: String,
    /// Connection pool size / 连接池大小
    pub pool_size: Option<u32>,
}

impl SmsConfig {
    /// Parse SMS config from TOML and normalize compatibility aliases.
    /// 从 TOML 解析 SMS 配置并规范化兼容别名。
    pub fn from_toml_str(toml_str: &str) -> anyhow::Result<Self> {
        let mut config: Self = toml::from_str(toml_str)?;
        config.normalize_object_storage_config();

        if config.execution_logs_dir.trim().is_empty() {
            config.execution_logs_dir = "./data/execution_logs".to_string();
        }

        Ok(config)
    }

    /// Load configuration with CLI arguments override / 使用CLI参数覆盖加载配置
    pub fn load_with_cli(args: &CliArgs) -> anyhow::Result<Self> {
        // Start with default configuration / 从默认配置开始
        let mut config = Self::default();

        // Apply environment variables (low priority) / 应用环境变量（较低优先级）
        // Prefix: SMS_  例如：SMS_GRPC_ADDR, SMS_HTTP_ADDR
        if let Ok(v) = std::env::var("SMS_GRPC_ADDR") {
            if let Ok(a) = v.parse::<std::net::SocketAddr>() {
                config.grpc.addr = a;
            }
        }
        if let Ok(v) = std::env::var("SMS_HTTP_ADDR") {
            if let Ok(a) = v.parse::<std::net::SocketAddr>() {
                config.http.addr = a;
            }
        }
        if let Ok(v) = std::env::var("SMS_ENABLE_SWAGGER") {
            if let Ok(b) = v.parse::<bool>() {
                config.enable_swagger = b;
            }
        }
        if let Ok(v) = std::env::var("SMS_ENABLE_CONSOLE") {
            if let Ok(b) = v.parse::<bool>() {
                config.enable_console = b;
            }
        }

        if let Ok(v) = std::env::var("SMS_DB_TYPE") {
            if !v.is_empty() {
                config.database.db_type = v;
            }
        }
        if let Ok(v) = std::env::var("SMS_DB_PATH") {
            if !v.is_empty() {
                config.database.path = v;
            }
        }
        if let Ok(v) = std::env::var("SMS_DB_POOL_SIZE") {
            if let Ok(n) = v.parse::<u32>() {
                config.database.pool_size = Some(n);
            }
        }

        if let Ok(v) = std::env::var("SMS_MCP_DIR") {
            if !v.is_empty() {
                config.mcp.dir = v;
            }
        }

        if let Ok(v) = std::env::var("SMS_LOG_LEVEL") {
            if !v.is_empty() {
                config.logging.level = v;
            }
        }
        if let Ok(v) = std::env::var("SMS_LOG_FORMAT") {
            if !v.is_empty() {
                config.logging.format = v;
            }
        }
        if let Ok(v) = std::env::var("SMS_LOG_FILE") {
            if !v.is_empty() {
                config.logging.file = Some(v);
            }
        }

        // Web Admin env / Web管理页面环境变量
        if let Ok(v) = std::env::var("SMS_ENABLE_WEB_ADMIN") {
            if let Ok(b) = v.parse::<bool>() {
                config.enable_web_admin = b;
            }
        }
        if let Ok(v) = std::env::var("SMS_WEB_ADMIN_ADDR") {
            if let Ok(a) = v.parse::<std::net::SocketAddr>() {
                config.web_admin.addr = a;
            }
        }
        if let Ok(v) = std::env::var("SMS_EXECUTION_LOGS_DIR") {
            if !v.is_empty() {
                config.execution_logs_dir = v;
            }
        }
        if let Ok(v) = std::env::var("SMS_OBJECT_STORAGE_BACKEND") {
            if let Some(backend) = parse_object_storage_backend(&v) {
                config.object_storage.backend = backend;
            }
        }
        if let Ok(v) = std::env::var("SMS_OBJECT_STORAGE_LOCAL_FS_ROOT_DIR") {
            if !v.is_empty() {
                config.object_storage.local_fs.root_dir = v;
            }
        }
        if let Ok(v) = std::env::var("SMS_OBJECT_STORAGE_RUSTFS_ENDPOINT") {
            if !v.is_empty() {
                config.object_storage.rustfs.endpoint = v;
            }
        }
        if let Ok(v) = std::env::var("SMS_OBJECT_STORAGE_RUSTFS_BUCKET") {
            if !v.is_empty() {
                config.object_storage.rustfs.bucket = v;
            }
        }
        if let Ok(v) = std::env::var("SMS_OBJECT_STORAGE_RUSTFS_ROOT") {
            if !v.is_empty() {
                config.object_storage.rustfs.root = v;
            }
        }
        if let Ok(v) = std::env::var("SMS_OBJECT_STORAGE_RUSTFS_REGION") {
            if !v.is_empty() {
                config.object_storage.rustfs.region = v;
            }
        }
        if let Ok(v) = std::env::var("SMS_OBJECT_STORAGE_RUSTFS_ACCESS_KEY") {
            if !v.is_empty() {
                config.object_storage.rustfs.access_key = v;
            }
        }
        if let Ok(v) = std::env::var("SMS_OBJECT_STORAGE_RUSTFS_SECRET_KEY") {
            if !v.is_empty() {
                config.object_storage.rustfs.secret_key = v;
            }
        }
        if let Ok(v) = std::env::var("SMS_OBJECT_STORAGE_RUSTFS_PATH_STYLE") {
            if let Ok(b) = v.parse::<bool>() {
                config.object_storage.rustfs.path_style = b;
            }
        }

        // Try loading from home directory first / 优先从用户主目录加载配置
        // Home path: ~/.sms/config.toml
        // 主目录路径：~/.sms/config.toml
        if args.config.is_none() {
            // Prefer SMS_HOME if set to avoid interfering with global HOME in tests
            // 若设置了SMS_HOME则优先使用，避免测试中修改全局HOME产生干扰
            let base_home = std::env::var_os("SMS_HOME").or_else(|| std::env::var_os("HOME"));
            if let Some(home_dir) = base_home {
                let home_path = std::path::PathBuf::from(home_dir)
                    .join(".sms")
                    .join("config.toml");
                if home_path.exists() {
                    let cfg = std::fs::read_to_string(&home_path)?;
                    match Self::from_toml_str(&cfg) {
                        Ok(c) => {
                            config = c;
                        }
                        Err(e) => {
                            eprintln!(
                                "SMS home config parse error at {}: {}",
                                home_path.display(),
                                e
                            );
                            return Err(e.into());
                        }
                    }
                }
            }
        }

        // Load from CLI-provided path (highest file priority) / 从命令行提供的路径加载（文件最高优先级）
        if let Some(config_path) = &args.config {
            let p = std::path::PathBuf::from(config_path);
            if p.exists() {
                let cfg = std::fs::read_to_string(&p)?;
                config = Self::from_toml_str(&cfg)?;
            } else {
                tracing::info!("Config file '{}' not found, using defaults", config_path);
            }
        }

        // Override with CLI arguments / 使用CLI参数覆盖
        if let Some(grpc_addr) = &args.grpc_addr {
            config.grpc.addr = grpc_addr.parse()?;
        }

        if let Some(http_addr) = &args.http_addr {
            config.http.addr = http_addr.parse()?;
        }

        if let Some(p) = &args.mcp_dir {
            config.mcp.dir = p.clone();
        }
        if let Some(p) = &args.execution_logs_dir {
            config.execution_logs_dir = p.clone();
        }

        if let Some(db_type) = &args.db_type {
            config.database.db_type = db_type.clone();
        }

        if let Some(db_path) = &args.db_path {
            config.database.path = db_path.clone();
        }

        if let Some(pool_size) = args.db_pool_size {
            config.database.pool_size = Some(pool_size);
        }

        // Handle Swagger UI flags / 处理Swagger UI标志
        if args.enable_swagger {
            config.enable_swagger = true;
        } else if args.disable_swagger {
            config.enable_swagger = false;
        }

        // Console flags / 控制台标志
        if args.enable_console {
            config.enable_console = true;
        } else if args.disable_console {
            config.enable_console = false;
        }

        // Web Admin flags / Web管理页面标志
        if args.enable_web_admin {
            config.enable_web_admin = true;
        } else if args.disable_web_admin {
            config.enable_web_admin = false;
        }

        if let Some(addr) = &args.web_admin_addr {
            config.web_admin.addr = addr.parse()?;
        }

        if let Some(log_level) = &args.log_level {
            config.logging.level = log_level.clone();
        }
        if let Some(log_format) = &args.log_format {
            config.logging.format = log_format.clone();
        }
        if let Some(log_file) = &args.log_file {
            if !log_file.is_empty() {
                config.logging.file = Some(log_file.clone());
            }
        }

        // Heartbeat & cleanup overrides / 心跳与清理覆盖
        if let Ok(v) = std::env::var("SMS_HEARTBEAT_TIMEOUT") {
            if let Ok(n) = v.parse::<u64>() {
                config.heartbeat_timeout = n;
            }
        }
        if let Ok(v) = std::env::var("SMS_CLEANUP_INTERVAL") {
            if let Ok(n) = v.parse::<u64>() {
                config.cleanup_interval = n;
            }
        }
        if let Ok(v) = std::env::var("SMS_ASSIGNMENT_RECONCILE_INTERVAL") {
            if let Ok(n) = v.parse::<u64>() {
                config.assignment_reconcile_interval = n;
            }
        }
        if let Ok(v) = std::env::var("SMS_MAX_UPLOAD_BYTES") {
            if let Ok(n) = v.parse::<u64>() {
                config.max_upload_bytes = n;
            }
        }
        if let Some(n) = args.heartbeat_timeout {
            config.heartbeat_timeout = n;
        }
        if let Some(n) = args.cleanup_interval {
            config.cleanup_interval = n;
        }
        if let Some(n) = args.assignment_reconcile_interval {
            config.assignment_reconcile_interval = n;
        }
        if let Some(n) = args.max_upload_bytes {
            config.max_upload_bytes = n;
        }

        if config.execution_logs_dir.trim().is_empty() {
            config.execution_logs_dir = "./data/execution_logs".to_string();
        }

        config.normalize_object_storage_config();

        // Optional: load event KV from environment / 可选：从环境变量加载事件KV
        let ev_backend = std::env::var("SMS_EVENTS_KV_BACKEND").ok();
        let ev_path = std::env::var("SMS_EVENTS_KV_PATH").ok();
        if ev_backend.is_some() || ev_path.is_some() {
            let backend = ev_backend.unwrap_or_else(|| "memory".to_string());
            let mut params = std::collections::HashMap::new();
            if let Some(p) = ev_path {
                params.insert("path".to_string(), p);
            }
            config.event_kv = Some(KvStoreConfig { backend, params });
        }

        Ok(config)
    }

    /// Normalize structured object storage settings after layered overrides.
    /// 在多层覆盖之后规范化结构化对象存储配置。
    fn normalize_object_storage_config(&mut self) {
        if self.object_storage.local_fs.root_dir.trim().is_empty() {
            self.object_storage.local_fs.root_dir =
                LocalFsObjectStorageConfig::default().root_dir;
        }
    }
}

impl Default for SmsConfig {
    fn default() -> Self {
        Self {
            grpc: ServerConfig {
                addr: "127.0.0.1:50051".parse().unwrap(),
                ..Default::default()
            },
            http: ServerConfig {
                addr: "127.0.0.1:8080".parse().unwrap(),
                ..Default::default()
            },
            logging: LogConfig::default(),
            enable_swagger: true,
            database: DatabaseConfig {
                db_type: "sled".to_string(),
                path: "./data/sms".to_string(),
                pool_size: Some(10),
            },
            enable_web_admin: false,
            enable_console: true,
            web_admin: ServerConfig {
                addr: "127.0.0.1:8081".parse().unwrap(),
                ..Default::default()
            },
            heartbeat_timeout: 90,
            cleanup_interval: 30,
            assignment_reconcile_interval: 60,
            max_upload_bytes: 64 * 1024 * 1024,
            object_storage: ObjectStorageConfig::default(),
            execution_logs_dir: String::new(),
            event_kv: None,
            mcp: McpConfig::default(),
        }
    }
}

impl Default for DatabaseConfig {
    fn default() -> Self {
        Self {
            db_type: "sled".to_string(),
            path: "./data".to_string(),
            pool_size: Some(10),
        }
    }
}

fn parse_object_storage_backend(value: &str) -> Option<ObjectStorageBackend> {
    match value.trim().to_ascii_lowercase().as_str() {
        "local_fs" | "local-fs" | "localfs" => Some(ObjectStorageBackend::LocalFs),
        "rustfs" => Some(ObjectStorageBackend::Rustfs),
        _ => None,
    }
}
