# 处理器架构 / Handlers Architecture

## 概述 / Overview

处理器模块为SPEAR元数据服务器提供服务层架构，将业务逻辑组织到专用的处理器组件中。这种架构分离关注点，为管理系统的不同方面提供清洁的接口。

The handlers module provides a service layer architecture for SPEAR Metadata Server, organizing business logic into dedicated handler components. This architecture separates concerns and provides a clean interface for managing different aspects of the system.

## 架构组件 / Architecture Components

### 节点处理器 / Node Handler

`NodeHandler` 管理集群中所有与节点相关的操作：

The `NodeHandler` manages all node-related operations in the cluster:

- **节点注册**：在集群中注册新节点 / **Node Registration**: Register new nodes in the cluster
- **节点管理**：更新、移除和查询节点信息 / **Node Management**: Update, remove, and query node information
- **健康监控**：跟踪节点心跳和健康状态 / **Health Monitoring**: Track node heartbeats and health status
- **资源集成**：管理节点资源信息 / **Resource Integration**: Manage node resource information
- **集群统计**：提供集群范围的统计信息 / **Cluster Statistics**: Provide cluster-wide statistics

### 资源处理器 / Resource Handler

`ResourceHandler` 管理资源监控和统计：

The `ResourceHandler` manages resource monitoring and statistics:

- **资源监控**：跟踪CPU、内存、磁盘和网络使用情况 / **Resource Monitoring**: Track CPU, memory, disk, and network usage
- **性能指标**：收集和分析性能数据 / **Performance Metrics**: Collect and analyze performance data
- **负载检测**：识别高负载节点 / **Load Detection**: Identify high-load nodes
- **资源清理**：移除过期的资源信息 / **Resource Cleanup**: Remove stale resource information
- **聚合统计**：提供集群范围的资源统计 / **Aggregated Statistics**: Provide cluster-wide resource statistics

## 模块结构 / Module Structure

```
src/handlers/
├── mod.rs          # 模块定义和导出 / Module definition and exports
├── node.rs         # 节点管理处理器 / Node management handler
└── resource.rs     # 资源监控处理器 / Resource monitoring handler
```

## 使用示例 / Usage Examples

### 节点处理器使用 / Node Handler Usage

```rust
use spear_next::handlers::node::{NodeHandler, NodeInfo};

// 创建新的节点处理器 / Create a new node handler
let mut node_handler = NodeHandler::new();

// 注册新节点 / Register a new node
let node = NodeInfo::new("127.0.0.1".to_string(), 8080);
node_handler.register_node(node).await?;

// 列出所有节点 / List all nodes
let nodes = node_handler.list_nodes().await?;

// 更新节点心跳 / Update node heartbeat
node_handler.update_heartbeat(&node_uuid).await?;

// 获取集群统计 / Get cluster statistics
let stats = node_handler.get_cluster_stats().await?;
```

### 资源处理器使用 / Resource Handler Usage

```rust
use spear_next::handlers::resource::{ResourceHandler, NodeResourceInfo};

// 创建新的资源处理器 / Create a new resource handler
let mut resource_handler = ResourceHandler::new();

// 更新资源信息 / Update resource information
let mut resource = NodeResourceInfo::new(node_uuid);
resource.cpu_usage_percent = 75.0;
resource.memory_usage_percent = 60.0;
resource_handler.update_resource(resource).await?;

// 获取资源信息 / Get resource information
let resource = resource_handler.get_resource(&node_uuid).await?;

// 列出高负载节点 / List high-load nodes
let high_load_nodes = resource_handler.list_high_load_nodes().await?;

// 获取集群资源统计 / Get cluster resource statistics
let avg_cpu = resource_handler.get_average_cpu_usage().await?;
let total_memory = resource_handler.get_total_memory_bytes().await?;
```

## 与KV存储的集成 / Integration with KV Store

两个处理器都与KV抽象层无缝集成：

Both handlers integrate seamlessly with the KV abstraction layer:

```rust
use spear_next::storage::MemoryKvStore;
use std::sync::Arc;

// 使用自定义KV存储创建处理器 / Create handlers with custom KV store
let kv_store = Arc::new(MemoryKvStore::new());
let node_handler = NodeHandler::with_kv_store(kv_store.clone());
let resource_handler = ResourceHandler::with_kv_store(kv_store);
```

## 处理器架构的优势 / Benefits of Handler Architecture

### 关注点分离 / Separation of Concerns

每个处理器专注于特定领域，使代码库更易维护和测试。

Each handler focuses on a specific domain, making the codebase more maintainable and testable.

### 可扩展性 / Scalability

可以轻松添加新的处理器以实现额外功能，而不影响现有代码。

New handlers can be easily added for additional functionality without affecting existing code.

### 可测试性 / Testability

处理器可以使用模拟依赖项独立测试。

Handlers can be tested independently with mock dependencies.

### 灵活性 / Flexibility

可以根据需求为不同的处理器使用不同的存储后端。

Different storage backends can be used for different handlers based on requirements.

## 从先前架构的迁移 / Migration from Previous Architecture

处理器架构替换了之前的 `common::node` 和 `common::resource` 模块：

The handlers architecture replaces the previous `common::node` and `common::resource` modules:

- `NodeRegistry` → `NodeHandler`
- `NodeResourceRegistry` → `ResourceHandler`
- 改进的API设计，具有更好的错误处理 / Improved API design with better error handling
- 增强的测试能力 / Enhanced testing capabilities
- 更好地分离节点和资源管理 / Better separation of node and resource management

## 未来扩展 / Future Extensions

处理器架构旨在适应未来的API添加：

The handler architecture is designed to accommodate future API additions:

- **认证处理器**：用户认证和授权 / **Authentication Handler**: User authentication and authorization
- **配置处理器**：动态配置管理 / **Configuration Handler**: Dynamic configuration management
- **指标处理器**：高级指标收集和分析 / **Metrics Handler**: Advanced metrics collection and analysis
- **事件处理器**：事件处理和通知 / **Event Handler**: Event processing and notification

每个新处理器都遵循相同的模式，并与现有的KV抽象层集成。

Each new handler follows the same pattern and integrates with the existing KV abstraction layer.

## 设计原则 / Design Principles

### 单一职责原则 / Single Responsibility Principle

每个处理器只负责一个特定的业务领域，确保代码的清晰性和可维护性。

Each handler is responsible for only one specific business domain, ensuring code clarity and maintainability.

### 依赖注入 / Dependency Injection

处理器通过构造函数接受KV存储依赖，支持不同的存储后端。

Handlers accept KV store dependencies through constructors, supporting different storage backends.

### 异步优先 / Async-First

所有处理器操作都是异步的，支持高并发和非阻塞操作。

All handler operations are asynchronous, supporting high concurrency and non-blocking operations.

### 错误处理 / Error Handling

统一的错误处理机制，使用 `SmsError` 类型提供清晰的错误信息。

Unified error handling mechanism using `SmsError` type to provide clear error information.

## 性能考虑 / Performance Considerations

### 内存效率 / Memory Efficiency

处理器使用引用计数和写时复制策略来最小化内存使用。

Handlers use reference counting and copy-on-write strategies to minimize memory usage.

### 并发安全 / Concurrency Safety

所有处理器操作都是线程安全的，支持多线程环境。

All handler operations are thread-safe, supporting multi-threaded environments.

### 缓存策略 / Caching Strategy

处理器可以实现缓存层来提高频繁访问数据的性能。

Handlers can implement caching layers to improve performance for frequently accessed data.