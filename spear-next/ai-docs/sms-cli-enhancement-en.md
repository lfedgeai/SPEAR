# SMS Command Line Interface Enhancement

## Overview
This document records the enhancement of SMS (SPEAR Metadata Server) command line interface, adding comprehensive CLI argument support to improve usability and configuration flexibility.

## Problem Statement
The original SMS binary (`sms`) had no command line argument support:
- Running `sms -h` or `sms --help` produced no output
- No way to configure SMS settings via command line
- Only supported default configuration loading via `SmsConfig::default()`

## Solution Implementation

### 1. Added CliArgs Structure
Created a comprehensive `CliArgs` structure in `src/sms/config.rs` with the following command line options:

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

### 2. Enhanced Configuration Loading
Added `load_with_cli` method to `SmsConfig` that merges CLI arguments with default configuration:

```rust
impl SmsConfig {
    pub fn load_with_cli(args: &CliArgs) -> anyhow::Result<Self> {
        let mut config = Self::default();
        
        // Override with CLI arguments
        if let Some(grpc_addr) = &args.grpc_addr {
            config.grpc.addr = grpc_addr.clone();
        }
        
        if let Some(http_addr) = &args.http_addr {
            config.http.addr = http_addr.clone();
        }
        
        // ... additional overrides for all CLI options
        
        Ok(config)
    }
}
```

### 3. Updated Main Binary
Modified `src/bin/sms/main.rs` to:
- Parse command line arguments using `clap::Parser`
- Set log level from CLI arguments
- Use `SmsConfig::load_with_cli` for configuration loading
- Display version information in startup logs

## Available CLI Options

| Option | Short | Description | Example |
|--------|-------|-------------|---------|
| `--config` | `-c` | Configuration file path | `-c config.toml` |
| `--grpc-addr` | | gRPC server address | `--grpc-addr 127.0.0.1:50052` |
| `--http-addr` | | HTTP gateway address | `--http-addr 127.0.0.1:8081` |
| `--db-type` | | Database type | `--db-type memory` |
| `--db-path` | | Database path | `--db-path ./data/sms` |
| `--db-pool-size` | | Database connection pool size | `--db-pool-size 10` |
| `--enable-swagger` | | Enable Swagger UI | `--enable-swagger` |
| `--disable-swagger` | | Disable Swagger UI | `--disable-swagger` |
| `--log-level` | | Log level | `--log-level debug` |
| `--heartbeat-timeout` | | Heartbeat timeout in seconds | `--heartbeat-timeout 30` |
| `--cleanup-interval` | | Cleanup interval in seconds | `--cleanup-interval 60` |
| `--help` | `-h` | Show help information | `-h` |
| `--version` | `-V` | Show version | `-V` |

## Testing Results

### Help Output Test
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

### Version Output Test
```bash
$ sms --version
sms 0.1.0
```

### Runtime Test
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

## Benefits

1. **Improved Usability**: Users can now get help and version information
2. **Flexible Configuration**: Runtime configuration without modifying config files
3. **Better DevOps Support**: Easy integration with deployment scripts and containers
4. **Consistent Interface**: Matches the CLI pattern used by SPEARlet
5. **Bilingual Support**: All help text provided in both English and Chinese

## Files Modified

- `src/sms/config.rs`: Added `CliArgs` structure and `load_with_cli` method
- `src/bin/sms/main.rs`: Integrated CLI argument parsing and configuration loading

## Future Enhancements

1. **Configuration File Loading**: Implement actual config file loading when `--config` is specified
2. **Environment Variable Support**: Add support for environment variable overrides
3. **Validation**: Add validation for CLI argument values (e.g., valid log levels, address formats)
4. **Auto-completion**: Generate shell completion scripts for better UX

## Dependencies

The enhancement uses the existing `clap` dependency that was already available in the project for SPEARlet's CLI support.