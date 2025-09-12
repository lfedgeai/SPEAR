//! SMS service configuration
//! SMS服务配置

use anyhow;
use clap::Parser;
use serde::{Deserialize, Serialize};
use std::net::SocketAddr;
use crate::config::base::{ServerConfig, LogConfig};

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
    #[arg(short, long, value_name = "FILE", help = "Configuration file path / 配置文件路径")]
    pub config: Option<String>,

    /// gRPC server address / gRPC服务器地址
    #[arg(long, value_name = "ADDR", help = "gRPC server address (e.g., 0.0.0.0:50051) / gRPC服务器地址")]
    pub grpc_addr: Option<String>,

    /// HTTP gateway address / HTTP网关地址
    #[arg(long, value_name = "ADDR", help = "HTTP gateway address (e.g., 0.0.0.0:8080) / HTTP网关地址")]
    pub http_addr: Option<String>,

    /// Database type / 数据库类型
    #[arg(long, value_name = "TYPE", help = "Database type (sled, rocksdb) / 数据库类型")]
    pub db_type: Option<String>,

    /// Database path / 数据库路径
    #[arg(long, value_name = "PATH", help = "Database path / 数据库路径")]
    pub db_path: Option<String>,

    /// Database pool size / 数据库连接池大小
    #[arg(long, value_name = "SIZE", help = "Database connection pool size / 数据库连接池大小")]
    pub db_pool_size: Option<u32>,

    /// Enable Swagger UI / 启用Swagger UI
    #[arg(long, help = "Enable Swagger UI / 启用Swagger UI")]
    pub enable_swagger: bool,

    /// Disable Swagger UI / 禁用Swagger UI
    #[arg(long, help = "Disable Swagger UI / 禁用Swagger UI", conflicts_with = "enable_swagger")]
    pub disable_swagger: bool,

    /// Log level / 日志级别
    #[arg(long, value_name = "LEVEL", help = "Log level (trace, debug, info, warn, error) / 日志级别")]
    pub log_level: Option<String>,

    /// Heartbeat timeout in seconds / 心跳超时时间（秒）
    #[arg(long, value_name = "SECONDS", help = "Heartbeat timeout in seconds / 心跳超时时间（秒）")]
    pub heartbeat_timeout: Option<u64>,

    /// Cleanup interval in seconds / 清理间隔时间（秒）
    #[arg(long, value_name = "SECONDS", help = "Cleanup interval in seconds / 清理间隔时间（秒）")]
    pub cleanup_interval: Option<u64>,
}

/// SMS service configuration / SMS服务配置
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SmsConfig {
    /// gRPC server configuration / gRPC服务器配置
    pub grpc: ServerConfig,
    /// HTTP gateway configuration / HTTP网关配置
    pub http: ServerConfig,
    /// Logging configuration / 日志配置
    pub log: LogConfig,
    /// Enable Swagger UI / 启用Swagger UI
    pub enable_swagger: bool,
    /// Database configuration / 数据库配置
    pub database: DatabaseConfig,
}

/// Database configuration / 数据库配置
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct DatabaseConfig {
    /// Database type (rocksdb, sled) / 数据库类型
    pub db_type: String,
    /// Database path / 数据库路径
    pub path: String,
    /// Connection pool size / 连接池大小
    pub pool_size: Option<u32>,
}

impl SmsConfig {
    /// Load configuration with CLI arguments override / 使用CLI参数覆盖加载配置
    pub fn load_with_cli(args: &CliArgs) -> anyhow::Result<Self> {
        // Start with default configuration / 从默认配置开始
        let mut config = Self::default();

        // Load from config file if specified / 如果指定了配置文件则加载
        if let Some(config_path) = &args.config {
            // TODO: Implement config file loading / 待实现：配置文件加载
            // For now, use default configuration / 目前使用默认配置
            tracing::info!("Config file loading not yet implemented, using defaults");
        }

        // Override with CLI arguments / 使用CLI参数覆盖
        if let Some(grpc_addr) = &args.grpc_addr {
            config.grpc.addr = grpc_addr.parse()?;
        }

        if let Some(http_addr) = &args.http_addr {
            config.http.addr = http_addr.parse()?;
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

        if let Some(log_level) = &args.log_level {
            config.log.level = log_level.clone();
        }

        Ok(config)
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
            log: LogConfig::default(),
            enable_swagger: true,
            database: DatabaseConfig {
                db_type: "sled".to_string(),
                path: "./data/sms".to_string(),
                pool_size: Some(10),
            },
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