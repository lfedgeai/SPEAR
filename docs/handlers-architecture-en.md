# Handlers Architecture / 处理器架构

## Overview / 概述

The handlers module provides a service layer architecture for SPEAR Metadata Server, organizing business logic into dedicated handler components. This architecture separates concerns and provides a clean interface for managing different aspects of the system.

处理器模块为SPEAR元数据服务器提供服务层架构，将业务逻辑组织到专用的处理器组件中。这种架构分离关注点，为管理系统的不同方面提供清洁的接口。

## Architecture Components / 架构组件

### Node Handler / 节点处理器

The `NodeHandler` manages all node-related operations in the cluster:

`NodeHandler` 管理集群中所有与节点相关的操作：

- **Node Registration**: Register new nodes in the cluster / 节点注册：在集群中注册新节点
- **Node Management**: Update, remove, and query node information / 节点管理：更新、移除和查询节点信息
- **Health Monitoring**: Track node heartbeats and health status / 健康监控：跟踪节点心跳和健康状态
- **Resource Integration**: Manage node resource information / 资源集成：管理节点资源信息
- **Cluster Statistics**: Provide cluster-wide statistics / 集群统计：提供集群范围的统计信息

### Resource Handler / 资源处理器

The `ResourceHandler` manages resource monitoring and statistics:

`ResourceHandler` 管理资源监控和统计：

- **Resource Monitoring**: Track CPU, memory, disk, and network usage / 资源监控：跟踪CPU、内存、磁盘和网络使用情况
- **Performance Metrics**: Collect and analyze performance data / 性能指标：收集和分析性能数据
- **Load Detection**: Identify high-load nodes / 负载检测：识别高负载节点
- **Resource Cleanup**: Remove stale resource information / 资源清理：移除过期的资源信息
- **Aggregated Statistics**: Provide cluster-wide resource statistics / 聚合统计：提供集群范围的资源统计

## Module Structure / 模块结构

```
src/handlers/
├── mod.rs          # Module definition and exports / 模块定义和导出
├── node.rs         # Node management handler / 节点管理处理器
└── resource.rs     # Resource monitoring handler / 资源监控处理器
```

## Usage Examples / 使用示例

### Node Handler Usage / 节点处理器使用

```rust
use spear_next::handlers::node::{NodeHandler, NodeInfo};

// Create a new node handler / 创建新的节点处理器
let mut node_handler = NodeHandler::new();

// Register a new node / 注册新节点
let node = NodeInfo::new("127.0.0.1".to_string(), 8080);
node_handler.register_node(node).await?;

// List all nodes / 列出所有节点
let nodes = node_handler.list_nodes().await?;

// Update node heartbeat / 更新节点心跳
node_handler.update_heartbeat(&node_uuid).await?;

// Get cluster statistics / 获取集群统计
let stats = node_handler.get_cluster_stats().await?;
```

### Resource Handler Usage / 资源处理器使用

```rust
use spear_next::handlers::resource::{ResourceHandler, NodeResourceInfo};

// Create a new resource handler / 创建新的资源处理器
let mut resource_handler = ResourceHandler::new();

// Update resource information / 更新资源信息
let mut resource = NodeResourceInfo::new(node_uuid);
resource.cpu_usage_percent = 75.0;
resource.memory_usage_percent = 60.0;
resource_handler.update_resource(resource).await?;

// Get resource information / 获取资源信息
let resource = resource_handler.get_resource(&node_uuid).await?;

// List high-load nodes / 列出高负载节点
let high_load_nodes = resource_handler.list_high_load_nodes().await?;

// Get cluster resource statistics / 获取集群资源统计
let avg_cpu = resource_handler.get_average_cpu_usage().await?;
let total_memory = resource_handler.get_total_memory_bytes().await?;
```

## Integration with KV Store / 与KV存储的集成

Both handlers integrate seamlessly with the KV abstraction layer:

两个处理器都与KV抽象层无缝集成：

```rust
use spear_next::storage::MemoryKvStore;
use std::sync::Arc;

// Create handlers with custom KV store / 使用自定义KV存储创建处理器
let kv_store = Arc::new(MemoryKvStore::new());
let node_handler = NodeHandler::with_kv_store(kv_store.clone());
let resource_handler = ResourceHandler::with_kv_store(kv_store);
```

## Benefits of Handler Architecture / 处理器架构的优势

### Separation of Concerns / 关注点分离

Each handler focuses on a specific domain, making the codebase more maintainable and testable.

每个处理器专注于特定领域，使代码库更易维护和测试。

### Scalability / 可扩展性

New handlers can be easily added for additional functionality without affecting existing code.

可以轻松添加新的处理器以实现额外功能，而不影响现有代码。

### Testability / 可测试性

Handlers can be tested independently with mock dependencies.

处理器可以使用模拟依赖项独立测试。

### Flexibility / 灵活性

Different storage backends can be used for different handlers based on requirements.

可以根据需求为不同的处理器使用不同的存储后端。

## Migration from Previous Architecture / 从先前架构的迁移

The handlers architecture replaces the previous `common::node` and `common::resource` modules:

处理器架构替换了之前的 `common::node` 和 `common::resource` 模块：

- `NodeRegistry` → `NodeHandler`
- `NodeResourceRegistry` → `ResourceHandler`
- Improved API design with better error handling / 改进的API设计，具有更好的错误处理
- Enhanced testing capabilities / 增强的测试能力
- Better separation of node and resource management / 更好地分离节点和资源管理

## Future Extensions / 未来扩展

The handler architecture is designed to accommodate future API additions:

处理器架构旨在适应未来的API添加：

- **Authentication Handler**: User authentication and authorization / 认证处理器：用户认证和授权
- **Configuration Handler**: Dynamic configuration management / 配置处理器：动态配置管理
- **Metrics Handler**: Advanced metrics collection and analysis / 指标处理器：高级指标收集和分析
- **Event Handler**: Event processing and notification / 事件处理器：事件处理和通知

Each new handler follows the same pattern and integrates with the existing KV abstraction layer.

每个新处理器都遵循相同的模式，并与现有的KV抽象层集成。