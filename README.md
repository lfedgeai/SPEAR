# SPEAR-Next

Next generation SPEAR components built with Rust and modern async technologies.

下一代SPEAR组件，使用Rust和现代异步技术构建。

## Components / 组件

### SMS (SPEAR Metadata Server)

SMS is a metadata server that provides gRPC APIs for node registration, updates, deletion, and heartbeat operations.

SMS是一个元数据服务器，提供用于节点注册、更新、删除和心跳操作的gRPC API。

#### Features / 特性

- **Node Management / 节点管理**: Register, update, and delete nodes
- **Heartbeat System / 心跳系统**: Monitor node health with configurable timeouts
- **gRPC API / gRPC API**: High-performance gRPC service
- **HTTP Gateway / HTTP网关**: RESTful API with Swagger UI documentation
- **Web Admin / 管理页面**: 独立端口的管理界面（节点列表、统计、SSE流、主题与时区）
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
- Web Admin: `http://localhost:8081/`（启用后）

#### Command Line Options / 命令行选项

SMS supports flexible configuration through command line arguments:

SMS支持通过命令行参数进行灵活配置：

| Option | Description | Example |
|--------|-------------|---------|
| `-c, --config <FILE>` | Configuration file path / 配置文件路径 | `--config config.toml` |
| `--grpc-addr <ADDR>` | gRPC server address / gRPC服务器地址 | `--grpc-addr 0.0.0.0:50051` |
| `--http-addr <ADDR>` | HTTP gateway address / HTTP网关地址 | `--http-addr 0.0.0.0:8080` |
| `--enable-web-admin` | Enable Web Admin / 启用管理页面 | `--enable-web-admin` |
| `--web-admin-addr <ADDR>` | Web Admin address / 管理页面地址 | `--web-admin-addr 0.0.0.0:8081` |
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

Current codebase uses configuration files with a home-first strategy and unified schema for both SMS and SPEARlet.

当前代码使用“家目录优先”的配置加载策略，并为 SMS 与 SPEARlet 统一了配置结构。

SMS config file location / SMS配置文件位置：
- `~/.sms/config.toml` (preferred) / 优先
- or `--config <path>` via CLI / 或通过CLI指定 `--config <路径>`
- otherwise defaults / 否则使用默认值

Example SMS config / SMS配置示例：
```toml
[grpc]
addr = "127.0.0.1:50051"
enable_tls = false

[http]
addr = "127.0.0.1:8080"
enable_tls = false

enable_swagger = true

[database]
db_type = "sled"
path = "./data/sms.db"
pool_size = 10

[log]
level = "debug"
format = "json"
file = "./logs/sms.log"
```

SPEARlet config file location / SPEARlet配置文件位置：
- `~/.spear/config.toml` (preferred) / 优先
- or `--config <path>` via CLI / 或通过CLI指定 `--config <路径>`
- otherwise defaults / 否则使用默认值

Example SPEARlet config / SPEARlet配置示例：
```toml
[spearlet]
node_name = ""
sms_grpc_addr = "127.0.0.1:50051"
sms_http_addr = "127.0.0.1:8080"
auto_register = true
heartbeat_interval = 30
cleanup_interval = 300
# Reconnect behavior / 重连行为
sms_connect_timeout_ms = 15000
sms_connect_retry_ms = 500
reconnect_total_timeout_ms = 300000

[spearlet.grpc]
addr = "0.0.0.0:50052"
enable_tls = false

[spearlet.http]
cors_enabled = true
swagger_enabled = true

[spearlet.http.server]
addr = "0.0.0.0:8081"

[spearlet.storage]
backend = "memory"
data_dir = "./data/spearlet"
max_cache_size_mb = 512
compression_enabled = true
max_object_size = 67108864

[spearlet.logging]
level = "debug"
format = "pretty"
file = "./logs/spearlet.log"
```

Configuration priority / 配置优先级：
- 1) CLI `--config` path / CLI指定的`--config`路径（最高）
- 2) Home config (`~/.sms/config.toml` or `~/.spear/config.toml`) / 家目录配置
- 3) Environment variables / 环境变量（如 `SPEARLET_*`、`SMS_*`）
- 4) Built-in defaults / 代码内置默认值

### Runtime Config Propagation / 运行时配置传递

- SPEARlet now passes the full `SpearletConfig` into each `Runtime` via `RuntimeConfig.spearlet_config`.
- 不再依赖环境变量或 `global_environment` 传递地址等信息，运行时在 `create_instance` 内直接读取 `spearlet_config`。
- Example / 示例：

```rust
// Build RuntimeConfig with full spearlet config / 构建RuntimeConfig并携带完整Spearlet配置
let rt_cfg = RuntimeConfig {
    runtime_type: RuntimeType::Wasm,
    settings: std::collections::HashMap::new(),
    global_environment: std::collections::HashMap::new(),
    spearlet_config: Some(config.clone()),
    resource_pool: ResourcePoolConfig::default(),
};
```

### WASM Artifact Download / WASM制品下载

- Supported `sms+file` forms:
  - Explicit override: `sms+file://<host:port>/<id>`
  - Short form: `sms+file://<id>` (runtime uses `SpearletConfig.sms_http_addr` for the HTTP gateway)
- WASM runtime constructs path `"/api/v1/files/<id>"` and downloads via `artifact_fetch::fetch_sms_file`.
- API:

```rust
// spear-next/src/spearlet/execution/artifact_fetch.rs
pub async fn fetch_sms_file(sms_http_addr: &str, path: &str) -> ExecutionResult<Vec<u8>>
```

### Function Service Initialization / 函数服务初始化

- `FunctionServiceImpl::new` now requires `Arc<SpearletConfig>`
- 现在需要传入 `Arc<SpearletConfig>`，以便下游 Runtime 读取完整配置。
- Example / 示例：

```rust
let function_service = FunctionServiceImpl::new(Arc::new(SpearletConfig::default())).await?;
```

Environment variables / 环境变量支持：

SPEARlet (`SPEARLET_*`):
- `SPEARLET_NODE_NAME`, `SPEARLET_SMS_GRPC_ADDR`, `SPEARLET_AUTO_REGISTER`, `SPEARLET_HEARTBEAT_INTERVAL`, `SPEARLET_CLEANUP_INTERVAL`
- `SPEARLET_SMS_HTTP_ADDR`
- `SPEARLET_GRPC_ADDR`, `SPEARLET_HTTP_ADDR`
- `SPEARLET_STORAGE_BACKEND`, `SPEARLET_STORAGE_DATA_DIR`, `SPEARLET_STORAGE_MAX_CACHE_MB`, `SPEARLET_STORAGE_COMPRESSION_ENABLED`, `SPEARLET_STORAGE_MAX_OBJECT_SIZE`
- `SPEARLET_LOG_LEVEL`, `SPEARLET_LOG_FORMAT`, `SPEARLET_LOG_FILE`
- `SPEARLET_SMS_CONNECT_TIMEOUT_MS`, `SPEARLET_SMS_CONNECT_RETRY_MS`, `SPEARLET_RECONNECT_TOTAL_TIMEOUT_MS`

SMS (`SMS_*`):
- `SMS_GRPC_ADDR`, `SMS_HTTP_ADDR`, `SMS_ENABLE_SWAGGER`
- `SMS_DB_TYPE`, `SMS_DB_PATH`, `SMS_DB_POOL_SIZE`
- `SMS_LOG_LEVEL`, `SMS_LOG_FORMAT`, `SMS_LOG_FILE`
- `SMS_ENABLE_WEB_ADMIN`, `SMS_WEB_ADMIN_ADDR`, `SMS_HEARTBEAT_TIMEOUT`, `SMS_CLEANUP_INTERVAL`

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

##### Register Task / 注册任务

```bash
curl -X POST http://localhost:8080/api/v1/tasks \
  -H "Content-Type: application/json" \
  -d '{
    "name": "image-processing-task",
    "description": "Process images using AI models",
    "node_uuid": "93f9a7ca-e033-4bb7-8b5a-c0899f9a52b8",
    "endpoint": "http://127.0.0.1:8081/process",
    "version": "1.0.0",
    "capabilities": ["image-processing", "ai-inference"],
    "priority": "high",
    "executable": {
      "type": "wasm",
      "uri": "sms+file://<file_id>",
      "name": "hello.wasm",
      "args": [],
      "env": {}
    }
  }'
```

##### List Tasks / 列出任务

```bash
curl "http://localhost:8080/api/v1/tasks?status=registered&priority=normal"
```

#### gRPC API / gRPC API

The gRPC service provides the following methods:

gRPC服务提供以下方法：

- `RegisterNode`: Register a new node / 注册新节点
- `UpdateNode`: Update an existing node / 更新现有节点
- `DeleteNode`: Delete a node / 删除节点
- `Heartbeat`: Send heartbeat / 发送心跳
- `ListNodes`: List all nodes / 列出所有节点

### WASM Runtime Notes / WASM运行时说明

- For `type=wasm` executables, SPEARlet strictly validates the module bytes during instance creation. Invalid or non-WASM content results in `InvalidConfiguration`.
- Recommended to build WASM with `zig cc -target wasm32-wasi`, or use `clang --target=wasm32-wasi --sysroot=$WASI_SYSROOT`.

### Samples / 示例

- Build the sample: `make samples`
- Source: `samples/wasm-c/hello.c`
- Output: `samples/build/hello.wasm`
- Makefile only includes the `samples` target; upload/register is handled via API flows (see docs).

### Documentation / 文档

- API Usage Guide: `ai-docs/api-usage-guide-en.md`, `ai-docs/api-usage-guide-zh.md`
- Task API Refactor (CN): `ai-docs/task-api-refactor-zh.md`
- WASM Runtime Usage: `ai-docs/wasm-runtime-usage-en.md`, `ai-docs/wasm-runtime-usage-zh.md`
- Samples Build Guide: `ai-docs/samples-build-guide-en.md`, `ai-docs/samples-build-guide-zh.md`
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
   make build
   # 或
   cargo build
   ```

3. **Test / 测试**:
   ```bash
make test
# 或
cargo test

# 运行特定测试模块（例如重连测试） / Run specific test module (e.g., reconnect)
cargo test spearlet_reconnect_tests -- --nocapture
   ```

4. **Run in development mode / 开发模式运行**:
   ```bash
   # SMS
   make run-sms
   # SPEARlet
   make run-spearlet
   ```

### Web Admin / 管理页面

- 地址：`http://127.0.0.1:8081/`（启用 `--enable-web-admin` 且指定 `--web-admin-addr`）
- 功能：节点列表、统计卡片、文件管理、任务管理、设置（主题/时区/Token）
- 管理 Token：在 Nodes 页工具栏或 Settings 页输入后点击 `Apply` 应用到前端与本地存储
- 文件页：
  - `Choose File` 选择文件（美化按钮，旁边显示文件名）
  - `Upload` 上传到内置对象服务；列表支持下载、复制 URI、删除
  - URI 形如：`sms+file://<id>`
- 任务创建：
  - 可执行类型：`No Executable | Binary | Script | Container | WASM | Process`
  - Scheme：`sms+file | s3 | minio | https`；选择 `sms+file` 时预填 `sms+file://`
  - 选择本地 SMS 文件：点击 `Choose Local` 弹窗，`Use` 将 URI 与名称带回表单
  - 时区：所有时间采用 Settings 页所选时区显示

文档：`ai-docs/web-admin-overview-zh.md`、`ai-docs/web-admin-ui-guide-zh.md`

### UI Tests / 前端测试

- 位置：`spear-next/ui-tests`
- 框架：Playwright
- 启动方式：`npm test`（测试会自动启动内置 SMS WebAdmin 服务）
- 全局初始化：`global-setup.ts` 会清理 `data/files` 目录保证幂等
- 关键用例：
  - 任务模态 Scheme 预填与本地文件选择
  - 可执行类型选择稳定性（使用隐藏原生 `select` 作为测试钩子）
  - 文件上传、删除及列表刷新
- 配置：`playwright.config.ts`

文档：`ai-docs/ui-tests-guide-zh.md`

### E2E Testing / 端到端测试

Container-based E2E tests verify SPEARlet ↔ SMS connectivity using Docker:

基于容器的端到端测试使用 Docker 验证 SPEARlet 与 SMS 的连通性：

- Test file / 测试文件：`tests/testcontainers_e2e.rs`
- Dev dependency / 开发依赖：`testcontainers = "0.15"`
- Run (ignored by default) / 运行（默认忽略）：

```bash
cargo build
DOCKER=1 cargo test --test testcontainers_e2e -- --ignored --nocapture
# 或使用Make目标
make e2e

# 在macOS或非Linux主机上，建议先交叉编译Linux二进制：
make e2e-linux
```

提示：E2E测试在容器中运行Linux环境，需要Linux目标的二进制。若本机为非Linux（如macOS），请使用 `make e2e-linux` 构建 `x86_64-unknown-linux-musl` 目标后再运行。
```

Documentation / 文档：
- `ai-docs/e2e-testing-en.md`
- `ai-docs/e2e-testing-zh.md`

CLI-gated SMS connect / 通过CLI控制的SMS连接：
- 指定 `--sms-addr` 时，SPEARlet 启动时将立即尝试连接到该 SMS 地址；如果连接失败，将退出进程
- 未指定 `--sms-addr` 时，SPEARlet 启动不会主动连接 SMS
- 可配合 `--auto-register` 使用：连接成功后立即执行注册，否则也会退出

Examples / 使用示例：
```bash
# 不连接SMS（未指定地址）
make run-spearlet

# 连接并注册到SMS，如果连接失败则退出
cargo run --bin spearlet -- --sms-addr 127.0.0.1:50051 --auto-register
```

### Task Events Subscription / 任务事件订阅

- SPEARlet 通过 gRPC 订阅 SMS 的任务事件流，仅处理当前节点的事件。
- 订阅器在 `storage.data_dir` 下持久化事件游标，文件名：`task_events_cursor_{node_uuid}.json`。
- 支持自动重连与退避：`sms_connect_retry_ms` 控制重试间隔，`sms_connect_timeout_ms` 控制连接超时，`reconnect_total_timeout_ms` 控制断线后的总超时。
- 对 `Create` 事件会拉取任务详情并准备执行分发（当前为占位逻辑）。

Usage / 使用示例：
```rust
use std::sync::Arc;
use spear_next::spearlet::{config::SpearletConfig, task_events::TaskEventSubscriber};

let config = Arc::new(SpearletConfig::default());
let subscriber = TaskEventSubscriber::new(config.clone());
subscriber.start().await; // 后台运行
```

Docs / 文档：`ai-docs/task-events-subscriber-en.md`、`ai-docs/task-events-subscriber-zh.md`

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
├── Cargo.toml                   # Project configuration / 项目配置
├── build.rs                     # Build script for protobuf / protobuf构建脚本
├── config/
│   ├── sms/config.toml          # SMS config / SMS配置
│   └── spearlet/config.toml     # SPEARlet config / SPEARlet配置
├── proto/
│   └── sms/sms.proto            # Protobuf definitions / Protobuf定义
├── src/
│   ├── lib.rs                   # Library root / 库根文件
│   ├── config/                  # Shared config types / 共享配置类型
│   │   ├── base.rs              # ServerConfig, LogConfig / 基础配置结构
│   │   └── mod.rs               # Logging init / 日志初始化
│   ├── apps/
│   │   ├── sms/main.rs          # SMS main / SMS主入口
│   │   └── spearlet/main.rs     # SPEARlet main / SPEARlet主入口
│   ├── sms/                     # SMS modules / SMS模块
│   │   ├── grpc_server.rs       # gRPC server / gRPC服务器
│   │   └── http_gateway.rs      # HTTP gateway / HTTP网关
│   └── spearlet/                # SPEARlet modules / SPEARlet模块
│       ├── grpc_server.rs       # gRPC server / gRPC服务器
│       └── http_gateway.rs      # HTTP gateway / HTTP网关
└── README.md                    # This file / 本文件
```

## License

This project is licensed under the same license as the main Spear project.

本项目采用与主Spear项目相同的许可证。
