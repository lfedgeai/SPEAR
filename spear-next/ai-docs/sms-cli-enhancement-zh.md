# SMS 命令行界面增强

## 概述
本文档记录了 SMS（SPEAR Metadata Server）命令行界面的增强工作，为其添加了全面的 CLI 参数支持，以提高可用性和配置灵活性。

## 问题描述
原始的 SMS 二进制文件（`sms`）没有命令行参数支持：
- 运行 `sms -h` 或 `sms --help` 没有任何输出
- 无法通过命令行配置 SMS 设置
- 仅支持通过 `SmsConfig::default()` 加载默认配置

## 解决方案实现

### 1. 添加 CliArgs 结构体
在 `src/sms/config.rs` 中创建了全面的 `CliArgs` 结构体，包含以下命令行选项：

```rust
#[derive(Parser, Debug, Clone)]
#[command(
    name = "sms",
    about = "SMS provides centralized management for Spearlet nodes and storage resources.\nSMS为Spearlet节点和存储资源提供集中管理。",
    version
)]
pub struct CliArgs {
    #[arg(short = 'c', long = "config", value_name = "FILE", help = "Configuration file path / 配置文件路径")]
    pub config: Option<PathBuf>,
    
    #[arg(long = "grpc-addr", value_name = "ADDR", help = "gRPC server address (e.g., 0.0.0.0:50051) / gRPC服务器地址")]
    pub grpc_addr: Option<String>,
    
    #[arg(long = "http-addr", value_name = "ADDR", help = "HTTP gateway address (e.g., 0.0.0.0:8080) / HTTP网关地址")]
    pub http_addr: Option<String>,
    
    #[arg(long = "db-type", value_name = "TYPE", help = "Database type (sled, rocksdb) / 数据库类型")]
    pub db_type: Option<String>,
    
    #[arg(long = "db-path", value_name = "PATH", help = "Database path / 数据库路径")]
    pub db_path: Option<PathBuf>,
    
    #[arg(long = "db-pool-size", value_name = "SIZE", help = "Database connection pool size / 数据库连接池大小")]
    pub db_pool_size: Option<u32>,
    
    #[arg(long = "enable-swagger", help = "Enable Swagger UI / 启用Swagger UI")]
    pub enable_swagger: bool,
    
    #[arg(long = "disable-swagger", help = "Disable Swagger UI / 禁用Swagger UI")]
    pub disable_swagger: bool,
    
    #[arg(long = "log-level", value_name = "LEVEL", help = "Log level (trace, debug, info, warn, error) / 日志级别")]
    pub log_level: Option<String>,
    
    #[arg(long = "heartbeat-timeout", value_name = "SECONDS", help = "Heartbeat timeout in seconds / 心跳超时时间（秒）")]
    pub heartbeat_timeout: Option<u64>,
    
    #[arg(long = "cleanup-interval", value_name = "SECONDS", help = "Cleanup interval in seconds / 清理间隔时间（秒）")]
    pub cleanup_interval: Option<u64>,
}
```

### 2. 增强配置加载
为 `SmsConfig` 添加了 `load_with_cli` 方法，将 CLI 参数与默认配置合并：

```rust
impl SmsConfig {
    pub fn load_with_cli(args: &CliArgs) -> anyhow::Result<Self> {
        let mut config = Self::default();
        
        // 使用 CLI 参数覆盖默认配置
        if let Some(grpc_addr) = &args.grpc_addr {
            config.grpc.addr = grpc_addr.clone();
        }
        
        if let Some(http_addr) = &args.http_addr {
            config.http.addr = http_addr.clone();
        }
        
        // ... 为所有 CLI 选项添加额外的覆盖逻辑
        
        Ok(config)
    }
}
```

### 3. 更新主二进制文件
修改了 `src/bin/sms/main.rs` 以：
- 使用 `clap::Parser` 解析命令行参数
- 从 CLI 参数设置日志级别
- 使用 `SmsConfig::load_with_cli` 进行配置加载
- 在启动日志中显示版本信息

## 可用的 CLI 选项

| 选项 | 简写 | 描述 | 示例 |
|------|------|------|------|
| `--config` | `-c` | 配置文件路径 | `-c config.toml` |
| `--grpc-addr` | | gRPC 服务器地址 | `--grpc-addr 127.0.0.1:50052` |
| `--http-addr` | | HTTP 网关地址 | `--http-addr 127.0.0.1:8081` |
| `--db-type` | | 数据库类型 | `--db-type memory` |
| `--db-path` | | 数据库路径 | `--db-path ./data/sms` |
| `--db-pool-size` | | 数据库连接池大小 | `--db-pool-size 10` |
| `--enable-swagger` | | 启用 Swagger UI | `--enable-swagger` |
| `--disable-swagger` | | 禁用 Swagger UI | `--disable-swagger` |
| `--log-level` | | 日志级别 | `--log-level debug` |
| `--heartbeat-timeout` | | 心跳超时时间（秒） | `--heartbeat-timeout 30` |
| `--cleanup-interval` | | 清理间隔时间（秒） | `--cleanup-interval 60` |
| `--help` | `-h` | 显示帮助信息 | `-h` |
| `--version` | `-V` | 显示版本 | `-V` |

## 测试结果

### 帮助输出测试
```bash
$ sms -h
SMS provides centralized management for Spearlet nodes and storage resources.
SMS为Spearlet节点和存储资源提供集中管理。

Usage: sms [OPTIONS]

Options:
  -c, --config <FILE>          Configuration file path / 配置文件路径
      --grpc-addr <ADDR>       gRPC server address (e.g., 0.0.0.0:50051) / gRPC服务器地址
      --http-addr <ADDR>       HTTP gateway address (e.g., 0.0.0.0:8080) / HTTP网关地址
      --db-type <TYPE>         Database type (sled, rocksdb) / 数据库类型
      --db-path <PATH>         Database path / 数据库路径
      --db-pool-size <SIZE>    Database connection pool size / 数据库连接池大小
      --enable-swagger         Enable Swagger UI / 启用Swagger UI
      --disable-swagger        Disable Swagger UI / 禁用Swagger UI
      --log-level <LEVEL>      Log level (trace, debug, info, warn, error) / 日志级别
      --heartbeat-timeout <SECONDS>  Heartbeat timeout in seconds / 心跳超时时间（秒）
      --cleanup-interval <SECONDS>   Cleanup interval in seconds / 清理间隔时间（秒）
  -h, --help                   Print help (see a summary with '-h')
  -V, --version                Print version
```

### 版本输出测试
```bash
$ sms --version
sms 0.1.0
```

### 运行时测试
```bash
$ sms --grpc-addr 127.0.0.1:50052 --http-addr 127.0.0.1:8081 --log-level debug --db-type memory
2025-09-12T07:03:45.776922Z  INFO sms: Starting SMS (SPEAR Metadata Server) v0.1.0
2025-09-12T07:03:45.777017Z  INFO sms: Configuration loaded:
2025-09-12T07:03:45.777025Z  INFO sms:   gRPC server: 127.0.0.1:50052
2025-09-12T07:03:45.777033Z  INFO sms:   HTTP gateway: 127.0.0.1:8081
2025-09-12T07:03:45.777040Z  INFO sms:   Swagger UI enabled: true
2025-09-12T07:03:45.777047Z  INFO sms:   Database type: memory
2025-09-12T07:03:45.777053Z  INFO sms:   Database path: ./data/sms
```

## 改进带来的好处

1. **提高可用性**：用户现在可以获取帮助和版本信息
2. **灵活配置**：无需修改配置文件即可进行运行时配置
3. **更好的 DevOps 支持**：易于与部署脚本和容器集成
4. **一致的界面**：与 SPEARlet 使用的 CLI 模式保持一致
5. **双语支持**：所有帮助文本都提供英文和中文版本

## 修改的文件

- `src/sms/config.rs`：添加了 `CliArgs` 结构体和 `load_with_cli` 方法
- `src/bin/sms/main.rs`：集成了 CLI 参数解析和配置加载

## 未来增强

1. **配置文件加载**：当指定 `--config` 时实现实际的配置文件加载
2. **环境变量支持**：添加对环境变量覆盖的支持
3. **验证**：为 CLI 参数值添加验证（例如，有效的日志级别、地址格式）
4. **自动补全**：生成 shell 补全脚本以提供更好的用户体验

## 依赖项

此增强使用了项目中已有的 `clap` 依赖项，该依赖项之前已用于 SPEARlet 的 CLI 支持。