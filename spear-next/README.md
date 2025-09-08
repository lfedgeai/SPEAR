# Spear-Next

Next generation Spear components built with Rust and modern async technologies.

下一代Spear组件，使用Rust和现代异步技术构建。

## Components / 组件

### SMS (Spear Management Service)

SMS is a node management service that provides gRPC APIs for node registration, updates, deletion, and heartbeat operations.

SMS是一个节点管理服务，提供用于节点注册、更新、删除和心跳操作的gRPC API。

#### Features / 特性

- **Node Management / 节点管理**: Register, update, and delete nodes
- **Heartbeat System / 心跳系统**: Monitor node health with configurable timeouts
- **gRPC API / gRPC API**: High-performance gRPC service
- **HTTP Gateway / HTTP网关**: RESTful API with Swagger UI documentation
- **Automatic Cleanup / 自动清理**: Remove unhealthy nodes automatically
- **Configuration / 配置**: TOML-based configuration with environment variable support

#### Quick Start / 快速开始

1. **Build the project / 构建项目**:
   ```bash
   cargo build --release
   ```

2. **Run SMS service / 运行SMS服务**:
   ```bash
   # Basic usage / 基本使用
   cargo run --bin sms
   
   # Or use the binary directly / 或直接使用二进制文件
   ./target/debug/sms
   ```

3. **Command Line Help / 命令行帮助**:
   ```bash
   # Show help / 显示帮助
   ./target/debug/sms -h
   
   # Show detailed help / 显示详细帮助
   ./target/debug/sms --help
   
   # Show version / 显示版本
   ./target/debug/sms --version
   ```

4. **Access the API / 访问API**:
   - gRPC: `localhost:50051`
   - HTTP Gateway: `http://localhost:8080`
   - Swagger UI: `http://localhost:8080/swagger-ui/`
   - OpenAPI Spec: `http://localhost:8080/api/openapi.json`

#### Command Line Options / 命令行选项

SMS supports flexible configuration through command line arguments:

SMS支持通过命令行参数进行灵活配置：

| Option | Description | Example |
|--------|-------------|---------|
| `-c, --config <FILE>` | Configuration file path / 配置文件路径 | `--config config.toml` |
| `--grpc-addr <ADDR>` | gRPC server address / gRPC服务器地址 | `--grpc-addr 0.0.0.0:50051` |
| `--http-addr <ADDR>` | HTTP gateway address / HTTP网关地址 | `--http-addr 0.0.0.0:8080` |
| `--heartbeat-timeout <SECONDS>` | Heartbeat timeout in seconds / 心跳超时时间（秒） | `--heartbeat-timeout 120` |
| `--cleanup-interval <SECONDS>` | Cleanup interval in seconds / 清理间隔时间（秒） | `--cleanup-interval 300` |
| `--enable-swagger` | Enable Swagger UI / 启用Swagger UI | `--enable-swagger` |
| `--log-level <LEVEL>` | Log level (trace, debug, info, warn, error) / 日志级别 | `--log-level info` |
| `-h, --help` | Print help information / 打印帮助信息 | `-h` |
| `-V, --version` | Print version / 打印版本信息 | `--version` |

##### Usage Examples / 使用示例

```bash
# Custom addresses / 自定义地址
./target/debug/sms --grpc-addr 127.0.0.1:50052 --http-addr 127.0.0.1:8081

# Custom timeouts / 自定义超时时间
./target/debug/sms --heartbeat-timeout 60 --cleanup-interval 180

# Enable Swagger and set log level / 启用Swagger并设置日志级别
./target/debug/sms --enable-swagger --log-level debug

# Use configuration file with overrides / 使用配置文件并覆盖设置
./target/debug/sms --config config.toml --grpc-addr 127.0.0.1:50052 --log-level info
```

#### Configuration / 配置

Create a `config.toml` file or use environment variables with `SPEAR_` prefix:

创建`config.toml`文件或使用`SPEAR_`前缀的环境变量：

```toml
[sms]
grpc_addr = "0.0.0.0:50051"
http_addr = "0.0.0.0:8080"
cleanup_interval = 300
heartbeat_timeout = 120
enable_swagger = true
```

Environment variables / 环境变量:
```bash
export SPEAR_SMS_GRPC_ADDR="0.0.0.0:50051"
export SPEAR_SMS_HTTP_ADDR="0.0.0.0:8080"
export SPEAR_SMS_CLEANUP_INTERVAL=300
export SPEAR_SMS_HEARTBEAT_TIMEOUT=120
export SPEAR_SMS_ENABLE_SWAGGER=true
```

##### Configuration Priority / 配置优先级

The configuration is loaded in the following order (highest to lowest priority):

配置按以下顺序加载（优先级从高到低）：

1. **Command line arguments** / **命令行参数**
2. **Environment variables** / **环境变量** (with `SPEAR_` prefix)
3. **Configuration file** / **配置文件** (TOML format)
4. **Default values** / **默认值**

#### API Examples / API示例

##### Register a Node / 注册节点

```bash
curl -X POST http://localhost:8080/api/v1/nodes \
  -H "Content-Type: application/json" \
  -d '{
    "ip_address": "192.168.1.100",
    "port": 8080,
    "metadata": {
      "region": "us-west-1",
      "zone": "a"
    }
  }'
```

##### List Nodes / 列出节点

```bash
curl http://localhost:8080/api/v1/nodes
```

##### Send Heartbeat / 发送心跳

```bash
curl -X POST http://localhost:8080/api/v1/nodes/{uuid}/heartbeat \
  -H "Content-Type: application/json" \
  -d '{
    "health_info": {
      "cpu_usage": "45%",
      "memory_usage": "60%",
      "disk_usage": "30%"
    }
  }'
```

##### Update Node / 更新节点

```bash
curl -X PUT http://localhost:8080/api/v1/nodes/{uuid} \
  -H "Content-Type: application/json" \
  -d '{
    "status": "active",
    "metadata": {
      "region": "us-west-2",
      "zone": "b"
    }
  }'
```

##### Delete Node / 删除节点

```bash
curl -X DELETE http://localhost:8080/api/v1/nodes/{uuid}
```

#### gRPC API / gRPC API

The gRPC service provides the following methods:

gRPC服务提供以下方法：

- `RegisterNode`: Register a new node / 注册新节点
- `UpdateNode`: Update an existing node / 更新现有节点
- `DeleteNode`: Delete a node / 删除节点
- `Heartbeat`: Send heartbeat / 发送心跳
- `ListNodes`: List all nodes / 列出所有节点
- `GetNode`: Get specific node / 获取特定节点

#### Architecture / 架构

```
┌─────────────────┐    ┌─────────────────┐
│   HTTP Gateway  │    │   gRPC Server   │
│   (Port 8080)   │────│   (Port 50051)  │
│                 │    │                 │
│ • REST API      │    │ • Node Registry │
│ • Swagger UI    │    │ • Heartbeat     │
│ • CORS Support  │    │ • Cleanup Task  │
└─────────────────┘    └─────────────────┘
```

#### Development / 开发

1. **Prerequisites / 前置条件**:
   - Rust 1.70+
   - Protocol Buffers compiler (`protoc`)

2. **Build / 构建**:
   ```bash
   cargo build
   ```

3. **Test / 测试**:
   ```bash
   cargo test
   ```

4. **Run in development mode / 开发模式运行**:
   ```bash
   RUST_LOG=debug cargo run --bin sms
   ```

#### Docker Support / Docker支持

A Dockerfile will be provided for containerized deployment.

将提供Dockerfile用于容器化部署。

### Storage Layer / 存储层

The project includes a flexible storage abstraction layer with support for multiple backends.

项目包含一个灵活的存储抽象层，支持多种后端。

#### KV Store Factory Pattern / KV存储工厂模式

The KV storage system uses a factory pattern for dynamic backend selection and configuration.

KV存储系统使用工厂模式进行动态后端选择和配置。

##### Supported Backends / 支持的后端

- **Memory**: In-memory storage for testing and development / 内存存储，用于测试和开发
- **Sled**: Embedded database for production use (requires `sled` feature) / 嵌入式数据库，用于生产环境（需要`sled`特性）
- **RocksDB**: High-performance persistent storage for production workloads (requires `rocksdb` feature) / 高性能持久化存储，用于生产工作负载（需要`rocksdb`特性）

##### Quick Usage / 快速使用

```rust
use spear_next::storage::{KvStoreConfig, create_kv_store_from_config};

// Create memory store / 创建内存存储
let config = KvStoreConfig::memory();
let store = create_kv_store_from_config(&config).await?;

// Create sled store (requires "sled" feature) / 创建sled存储（需要"sled"特性）
#[cfg(feature = "sled")]
let config = KvStoreConfig::sled("/path/to/database");
let store = create_kv_store_from_config(&config).await?;

// Create RocksDB store (requires "rocksdb" feature) / 创建RocksDB存储（需要"rocksdb"特性）
#[cfg(feature = "rocksdb")]
let config = KvStoreConfig::rocksdb("/path/to/rocksdb");
let store = create_kv_store_from_config(&config).await?;

// Use the store / 使用存储
store.put(&"key".to_string(), &"value".as_bytes().to_vec()).await?;
let value = store.get(&"key".to_string()).await?;
```

##### Environment Configuration / 环境变量配置

```bash
# Set backend type / 设置后端类型
export KV_STORE_BACKEND=memory

# Add custom parameters / 添加自定义参数
export KV_STORE_CACHE_SIZE=5000
export KV_STORE_DEBUG=true

# For sled backend / 对于sled后端
export KV_STORE_BACKEND=sled
export KV_STORE_PATH=/var/lib/app/data

# For RocksDB backend (requires "rocksdb" feature) / 对于RocksDB后端（需要"rocksdb"特性）
export KV_STORE_BACKEND=rocksdb
export KV_STORE_PATH=/var/lib/app/rocksdb
```

Then create from environment / 然后从环境变量创建:

```rust
use spear_next::storage::create_kv_store_from_env;

let store = create_kv_store_from_env().await?;
```

##### Examples and Documentation / 示例和文档

- **Examples**: See `examples/kv_factory_usage.rs` for complete usage examples / 查看`examples/kv_factory_usage.rs`获取完整使用示例
- **Documentation**: See `ai-docs/kv-factory-pattern-en.md` and `ai-docs/kv-factory-pattern-zh.md` for detailed documentation / 查看`ai-docs/kv-factory-pattern-en.md`和`ai-docs/kv-factory-pattern-zh.md`获取详细文档

##### Running Examples / 运行示例

```bash
# Run the factory pattern example / 运行工厂模式示例
cargo run --example kv_factory_usage --features sled

# Run with RocksDB support / 使用RocksDB支持运行
cargo run --example kv_factory_usage --features rocksdb

# Run storage tests / 运行存储测试
cargo test storage --lib --features sled

# Run storage tests with RocksDB / 使用RocksDB运行存储测试
cargo test storage --lib --features rocksdb
```

## Project Structure / 项目结构

```
spear-next/
├── Cargo.toml              # Project configuration / 项目配置
├── build.rs                # Build script for protobuf / protobuf构建脚本
├── config.toml             # Example configuration / 示例配置
├── proto/                  # Protocol buffer definitions / Protocol buffer定义
│   └── sms/
│       └── sms.proto
├── src/
│   ├── lib.rs              # Library root / 库根文件
│   ├── common/             # Common utilities / 通用工具
│   │   ├── mod.rs
│   │   ├── config.rs       # Configuration management / 配置管理
│   │   ├── error.rs        # Error types / 错误类型
│   │   └── node.rs         # Node data models / 节点数据模型
│   ├── proto/              # Generated protobuf code / 生成的protobuf代码
│   │   └── mod.rs
│   └── bin/
│       └── sms/            # SMS binary / SMS二进制
│           ├── main.rs     # Main entry point / 主入口点
│           ├── service.rs  # gRPC service implementation / gRPC服务实现
│           └── gateway.rs  # HTTP gateway / HTTP网关
└── README.md               # This file / 本文件
```

## License

This project is licensed under the same license as the main Spear project.

本项目采用与主Spear项目相同的许可证。