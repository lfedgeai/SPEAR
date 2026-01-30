# 命令行配置指南

## 概述

SPEAR节点服务支持通过命令行参数、配置文件和环境变量进行灵活配置。本指南涵盖所有可用的配置选项及其使用方法。

## 命令行参数

### 基本用法

```bash
cargo run --bin sms -- [选项]
```

### 可用选项

| 选项 | 类型 | 描述 | 默认值 |
|------|------|------|--------|
| `--config <文件>` | String | 配置文件路径 | 无 |
| `--grpc-addr <地址>` | String | gRPC服务器地址 | 0.0.0.0:50051 |
| `--http-addr <地址>` | String | HTTP网关地址 | 0.0.0.0:8080 |
| `--sms-http-addr <地址>` | String | SMS HTTP网关地址（供 spearlet 访问 smsfile） | 127.0.0.1:8080 |
| `--heartbeat-timeout <秒数>` | u64 | 节点心跳超时时间 | 120 |
| `--cleanup-interval <秒数>` | u64 | 节点清理间隔 | 300 |
| `--enable-swagger` | 标志 | 启用Swagger UI | false |
| `--log-level <级别>` | String | 日志级别 (trace, debug, info, warn, error) | info |
| `--storage-backend <后端>` | String | KV存储后端 (memory, sled, rocksdb) | memory |
| `--storage-path <路径>` | String | 基于文件的后端存储路径 | 无 |

### 存储后端配置

#### 内存后端（默认）
```bash
cargo run --bin sms -- --storage-backend memory
```

#### Sled后端
```bash
# 启用sled特性并指定后端
cargo run --features sled --bin sms -- --storage-backend sled --storage-path ./data/sled.db
```

#### RocksDB后端
```bash
# 启用rocksdb特性并指定后端
cargo run --features rocksdb --bin sms -- --storage-backend rocksdb --storage-path ./data/rocksdb
```

## 配置文件

### 文件格式

配置文件使用TOML格式。您可以使用`--config`选项指定配置文件。

### 配置文件示例

#### 基本配置 (config.toml)
```toml
[sms]
grpc_addr = "0.0.0.0:50051"
http_addr = "0.0.0.0:8080"
cleanup_interval = 300
heartbeat_timeout = 120
enable_swagger = true

[sms.kv_store]
backend = "memory"
```

#### Sled后端配置 (config-sled.toml)
```toml
[sms]
grpc_addr = "0.0.0.0:50051"
http_addr = "0.0.0.0:8080"
cleanup_interval = 300
heartbeat_timeout = 120
enable_swagger = true

[sms.kv_store]
backend = "sled"

[sms.kv_store.params]
path = "./data/node-service.db"
cache_capacity = "10000"
compression = "true"
flush_every_ms = "5000"
```

#### RocksDB后端配置 (config-rocksdb.toml)
```toml
[sms]
grpc_addr = "0.0.0.0:50051"
http_addr = "0.0.0.0:8080"
cleanup_interval = 300
heartbeat_timeout = 120
enable_swagger = true

[sms.kv_store]
backend = "rocksdb"

[sms.kv_store.params]
path = "./data/rocksdb-node-service"
cache_size = "67108864"  # 64MB
write_buffer_size = "16777216"  # 16MB
max_write_buffer_number = "3"
compression = "snappy"
```

### 加载配置文件
```bash
cargo run --bin sms -- --config config.toml
cargo run --features sled --bin sms -- --config config-sled.toml
cargo run --features rocksdb --bin sms -- --config config-rocksdb.toml
```

## 环境变量

### 通过环境变量配置KV存储

```bash
# 设置后端类型
export KV_STORE_BACKEND=sled

# 设置存储路径
export KV_STORE_PATH=./data/env-sled.db

# 设置额外参数
export KV_STORE_CACHE_SIZE=5000
export KV_STORE_COMPRESSION=true

# 运行应用程序
cargo run --features sled --bin sms
```

### 支持的环境变量

| 变量 | 描述 | 示例 |
|------|------|------|
| `KV_STORE_BACKEND` | 存储后端类型 | `memory`, `sled`, `rocksdb` |
| `KV_STORE_PATH` | 存储路径 | `./data/storage.db` |
| `KV_STORE_*` | 额外参数 | `KV_STORE_CACHE_SIZE=1000` |
| `SPEARLET_SMS_HTTP_ADDR` | SPEARlet 使用的 SMS HTTP 网关地址 | `127.0.0.1:8080` |

## 配置优先级

配置值按以下顺序应用（优先级从高到低）：

1. **命令行参数** - 覆盖所有其他来源
2. **配置文件** - 通过`--config`选项指定
3. **环境变量** - 系统环境设置
4. **默认值** - 内置默认值

### 默认值与空值处理（SPEARlet）

- `sms_http_addr` 默认值为 `127.0.0.1:8080`。
- 若配置文件或环境变量提供了空字符串，加载时会自动归一化为默认值，避免在 `smsfile://<id>` 下载时出现空基址导致的错误。

## 示例

### 开发环境设置
```bash
# 使用内存后端快速启动
cargo run --bin sms

# 开发环境使用文件持久化
cargo run --features sled --bin sms -- --storage-backend sled --storage-path ./dev-data.db
```

### 生产环境设置
```bash
# 使用配置文件
cargo run --features sled --bin sms -- --config production-config.toml

# 使用自定义地址
cargo run --features sled --bin sms -- \
  --config production-config.toml \
  --grpc-addr 0.0.0.0:9090 \
  --http-addr 0.0.0.0:8090
```

### 测试不同后端
```bash
# 测试内存后端
cargo run --bin sms -- --storage-backend memory

# 测试sled后端
cargo run --features sled --bin sms -- --storage-backend sled --storage-path ./test-sled.db

# 测试rocksdb后端（如果依赖可用）
cargo run --features rocksdb --bin sms -- --storage-backend rocksdb --storage-path ./test-rocksdb
```

## 故障排除

### 常见问题

1. **特性未启用**：确保为您的存储后端启用所需的特性
   ```bash
   # 错误：cargo run --bin sms -- --storage-backend sled
   # 正确：cargo run --features sled --bin sms -- --storage-backend sled
   ```

2. **缺少存储路径**：基于文件的后端需要存储路径
   ```bash
   cargo run --features sled --bin sms -- --storage-backend sled --storage-path ./data.db
   ```

3. **配置文件未找到**：确保配置文件存在
   ```bash
   # 检查文件是否存在
   ls -la config.toml
   ```

### 验证

应用程序在启动时验证配置，并会报告以下错误：
- 不支持的存储后端
- 无效的文件路径
- 格式错误的配置文件
- 缺少必需参数

## 最佳实践

1. **生产环境使用配置文件** - 比命令行参数更易维护
2. **启用适当的特性** - 只编译需要的存储后端
3. **设置适当的超时时间** - 根据您的环境调整心跳和清理间隔
4. **使用环境变量存储敏感信息** - 不要在配置文件中放置敏感数据
5. **测试配置更改** - 在部署到生产环境之前验证配置
