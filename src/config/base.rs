//! Base configuration structures and utilities
//! 基础配置结构和工具

use serde::{Deserialize, Serialize};
use std::net::SocketAddr;

/// Base server configuration / 基础服务器配置
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(default)]
pub struct ServerConfig {
    /// Server bind address / 服务器绑定地址
    #[serde(deserialize_with = "deserialize_socket_addr")]
    pub addr: SocketAddr,
    /// Enable TLS / 启用TLS
    pub enable_tls: bool,
    /// TLS certificate path / TLS证书路径
    pub cert_path: Option<String>,
    /// TLS private key path / TLS私钥路径
    pub key_path: Option<String>,
}

impl Default for ServerConfig {
    fn default() -> Self {
        Self {
            addr: "127.0.0.1:8080".parse().unwrap(),
            enable_tls: false,
            cert_path: None,
            key_path: None,
        }
    }
}

fn deserialize_socket_addr<'de, D>(deserializer: D) -> Result<SocketAddr, D::Error>
where
    D: serde::Deserializer<'de>,
{
    let s = String::deserialize(deserializer)?;
    s.parse().map_err(serde::de::Error::custom)
}

/// Base logging configuration / 基础日志配置
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(default)]
pub struct LogConfig {
    /// Log level / 日志级别
    pub level: String,
    /// Log format / 日志格式
    pub format: String,
    /// Log output file / 日志输出文件
    pub file: Option<String>,
}

impl Default for LogConfig {
    fn default() -> Self {
        Self {
            level: "info".to_string(),
            format: "json".to_string(),
            file: None,
        }
    }
}

impl LogConfig {
    /// Convert to the common LoggingConfig used by init_tracing
    /// 转换为init_tracing使用的通用LoggingConfig
    pub fn to_logging_config(&self) -> crate::config::LoggingConfig {
        crate::config::LoggingConfig {
            level: self.level.clone(),
            format: self.format.clone(),
            file_enabled: self.file.is_some(),
            file_path: self.file.as_ref().map(|p| std::path::PathBuf::from(p)),
        }
    }
}

/// Storage configuration / 存储配置
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct StorageConfig {
    /// Storage backend type (rocksdb, sled) / 存储后端类型
    pub backend: String,
    /// Storage data directory / 存储数据目录
    pub data_dir: String,
    /// Maximum cache size in MB / 最大缓存大小(MB)
    pub max_cache_size_mb: u64,
    /// Enable compression / 启用压缩
    pub compression_enabled: bool,
    /// Connection pool size / 连接池大小
    pub pool_size: u32,
}

impl Default for StorageConfig {
    fn default() -> Self {
        Self {
            backend: "rocksdb".to_string(),
            data_dir: "./data".to_string(),
            max_cache_size_mb: 256,
            compression_enabled: true,
            pool_size: 10,
        }
    }
}
