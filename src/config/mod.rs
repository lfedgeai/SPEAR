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
use serde::{Deserialize, Serialize};
use std::path::PathBuf;
use std::sync::OnceLock;

/// Base configuration shared by all applications / 所有应用程序共享的基础配置
pub mod base;
pub use crate::storage::kv::KvStoreConfig;
pub use base::*;

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
