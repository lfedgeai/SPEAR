//! Configuration management for the Spear project
//! Spear项目的配置管理
//!
//! This module provides a unified configuration framework that supports:
//! - Command line arguments / 命令行参数
//! - Environment variables / 环境变量
//! - Configuration files (TOML) / 配置文件（TOML）
//! - Hierarchical configuration / 分层配置
//!
//! The configuration system is designed to be flexible and extensible,
//! allowing different services (SMS, Spearlet) to define their own
//! configuration structures while sharing common base configuration.
//!
//! 配置系统设计为灵活且可扩展的，允许不同的服务（SMS、Spearlet）
//! 定义自己的配置结构，同时共享通用的基础配置。

use anyhow::{Context, Result};
use figment::{
    providers::{Env, Format, Serialized, Toml},
    Figment,
};
use serde::{Deserialize, Serialize};
use std::net::SocketAddr;
use std::path::PathBuf;
use std::sync::OnceLock;

/// Base configuration shared by all applications / 所有应用程序共享的基础配置
pub mod base;
pub use crate::storage::kv::KvStoreConfig;
pub use base::*;

/// Base configuration trait / 基础配置特征
/// All application configurations should implement this trait
/// 所有应用程序配置都应该实现此特征
pub trait AppConfig: for<'de> Deserialize<'de> + Serialize + Clone + std::fmt::Debug {
    /// Load configuration from multiple sources with proper precedence
    /// 从多个源加载配置，具有适当的优先级
    ///
    /// Precedence order (highest to lowest):
    /// 优先级顺序（从高到低）：
    /// 1. Command line arguments / 命令行参数
    /// 2. Environment variables / 环境变量  
    /// 3. Configuration file / 配置文件
    /// 4. Default values / 默认值
    fn load() -> Result<Self> {
        let figment = Figment::new()
            .merge(Serialized::defaults(Self::default_config()))
            .merge(Toml::file("config.toml"))
            .merge(Env::prefixed("SPEAR_"))
            .merge(Self::cli_overrides()?);

        figment.extract().context("Failed to load configuration")
    }

    /// Load configuration from a specific file
    /// 从特定文件加载配置
    fn load_from_file(path: &PathBuf) -> Result<Self> {
        let figment = Figment::new()
            .merge(Serialized::defaults(Self::default_config()))
            .merge(Toml::file(path))
            .merge(Env::prefixed("SPEAR_"))
            .merge(Self::cli_overrides()?);

        figment
            .extract()
            .context("Failed to load configuration from file")
    }

    /// Get default configuration values
    /// 获取默认配置值
    fn default_config() -> Self;

    /// Get CLI argument overrides
    /// 获取CLI参数覆盖
    fn cli_overrides() -> Result<Serialized<Self>>;

    /// Validate the configuration
    /// 验证配置
    fn validate(&self) -> Result<()> {
        Ok(())
    }
}

/// Common network configuration / 通用网络配置
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct NetworkConfig {
    /// gRPC server bind address / gRPC服务器绑定地址
    pub grpc_addr: SocketAddr,
    /// HTTP server bind address / HTTP服务器绑定地址
    pub http_addr: SocketAddr,
    /// Maximum message size for gRPC / gRPC最大消息大小
    pub max_message_size: usize,
    /// Request timeout in seconds / 请求超时时间（秒）
    pub request_timeout: u64,
}

impl Default for NetworkConfig {
    fn default() -> Self {
        Self {
            grpc_addr: "127.0.0.1:50051".parse().unwrap(),
            http_addr: "127.0.0.1:8080".parse().unwrap(),
            max_message_size: 4 * 1024 * 1024, // 4MB
            request_timeout: 30,
        }
    }
}

/// Common logging configuration / 通用日志配置
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct LoggingConfig {
    /// Log level (trace, debug, info, warn, error) / 日志级别
    pub level: String,
    /// Log format (json, pretty) / 日志格式
    pub format: String,
    /// Enable file logging / 启用文件日志
    pub file_enabled: bool,
    /// Log file path / 日志文件路径
    pub file_path: Option<PathBuf>,
}

static FILE_LOG_GUARD: OnceLock<tracing_appender::non_blocking::WorkerGuard> = OnceLock::new();

impl Default for LoggingConfig {
    fn default() -> Self {
        Self {
            level: "info".to_string(),
            format: "pretty".to_string(),
            file_enabled: false,
            file_path: None,
        }
    }
}

/// Initialize tracing based on logging configuration
/// 基于日志配置初始化跟踪
pub fn init_tracing(config: &LoggingConfig) -> Result<()> {
    use tracing_subscriber::{layer::SubscriberExt, util::SubscriberInitExt, EnvFilter};

    let env_filter = EnvFilter::try_from_default_env().unwrap_or_else(|_| {
        if config.level.trim().is_empty() {
            EnvFilter::new("info")
        } else {
            EnvFilter::new(config.level.clone())
        }
    });

    let registry = tracing_subscriber::registry().with(env_filter);

    let file_writer = if config.file_enabled {
        if let Some(path) = config.file_path.as_ref() {
            if let Some(parent) = path.parent() {
                std::fs::create_dir_all(parent)
                    .with_context(|| format!("create log dir: {}", parent.display()))?;
            }
            let file = std::fs::OpenOptions::new()
                .create(true)
                .append(true)
                .open(path)
                .with_context(|| format!("open log file: {}", path.display()))?;
            let (file_writer, guard) = tracing_appender::non_blocking(file);
            let _ = FILE_LOG_GUARD.set(guard);
            Some(file_writer)
        } else {
            None
        }
    } else {
        None
    };

    match (config.format.as_str(), file_writer) {
        ("json", Some(file_writer)) => {
            let stdout_layer = tracing_subscriber::fmt::layer()
                .json()
                .with_target(true)
                .with_timer(tracing_subscriber::fmt::time::uptime())
                .with_level(true);
            let file_layer = tracing_subscriber::fmt::layer()
                .json()
                .with_target(true)
                .with_timer(tracing_subscriber::fmt::time::uptime())
                .with_level(true)
                .with_writer(file_writer);
            registry.with(stdout_layer).with(file_layer).init();
        }
        ("compact", Some(file_writer)) => {
            let stdout_layer = tracing_subscriber::fmt::layer()
                .compact()
                .with_target(true)
                .with_timer(tracing_subscriber::fmt::time::uptime())
                .with_level(true);
            let file_layer = tracing_subscriber::fmt::layer()
                .compact()
                .with_target(true)
                .with_timer(tracing_subscriber::fmt::time::uptime())
                .with_level(true)
                .with_writer(file_writer);
            registry.with(stdout_layer).with(file_layer).init();
        }
        (_, Some(file_writer)) => {
            let stdout_layer = tracing_subscriber::fmt::layer()
                .pretty()
                .with_target(true)
                .with_timer(tracing_subscriber::fmt::time::uptime())
                .with_level(true);
            let file_layer = tracing_subscriber::fmt::layer()
                .compact()
                .with_target(true)
                .with_timer(tracing_subscriber::fmt::time::uptime())
                .with_level(true)
                .with_writer(file_writer);
            registry.with(stdout_layer).with(file_layer).init();
        }
        ("json", None) => {
            let stdout_layer = tracing_subscriber::fmt::layer()
                .json()
                .with_target(true)
                .with_timer(tracing_subscriber::fmt::time::uptime())
                .with_level(true);
            registry.with(stdout_layer).init();
        }
        ("compact", None) => {
            let stdout_layer = tracing_subscriber::fmt::layer()
                .compact()
                .with_target(true)
                .with_timer(tracing_subscriber::fmt::time::uptime())
                .with_level(true);
            registry.with(stdout_layer).init();
        }
        (_, None) => {
            let stdout_layer = tracing_subscriber::fmt::layer()
                .pretty()
                .with_target(true)
                .with_timer(tracing_subscriber::fmt::time::uptime())
                .with_level(true);
            registry.with(stdout_layer).init();
        }
    }

    Ok(())
}
