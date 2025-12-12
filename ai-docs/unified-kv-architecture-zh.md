# 统一KV存储架构 / Unified KV Storage Architecture

## 概述 / Overview

本文档描述了spear-next项目中统一KV存储架构的重构，该重构将原本分离的NodeRegistry和NodeResourceRegistry统一到基于KV存储的架构中。

This document describes the refactoring of the unified KV storage architecture in the spear-next project, which unifies the previously separate NodeRegistry and NodeResourceRegistry into a KV storage-based architecture.

## 架构变更 / Architecture Changes

### 重构前 / Before Refactoring

```
NodeRegistry (Memory-based)
├── nodes: HashMap<Uuid, NodeInfo>
└── Direct memory operations

KvNodeRegistry (KV-based)  
├── kv_store: Box<dyn KvStore>
└── Serialized operations

NodeResourceRegistry (Memory-based)
├── resources: HashMap<Uuid, NodeResourceInfo>
└── Direct memory operations
```

### 重构后 / After Refactoring

```
NodeRegistry (Unified KV-based)
├── kv_store: Box<dyn KvStore>
├── Namespace: "node:"
└── Configurable backend (Memory/Sled)

NodeResourceRegistry (Unified KV-based)
├── kv_store: Box<dyn KvStore>  
├── Namespace: "resource:"
└── Configurable backend (Memory/Sled)
```

## 核心改进 / Core Improvements

### 1. 统一存储接口 / Unified Storage Interface

所有注册表现在都使用相同的KV存储抽象：
All registries now use the same KV storage abstraction:

- **单一KV存储实例**: 所有数据操作通过统一的KV存储接口进行
- **配置驱动**: 通过`KvStoreConfig`配置存储后端类型（内存、Sled等）
- **类型安全**: 使用强类型的序列化/反序列化机制
- **共享存储**: `NodeRegistry`和`NodeResourceRegistry`共享同一个`Arc<dyn KvStore>`实例

```rust
// 统一的构造函数 / Unified constructors
let config = KvStoreConfig::memory();
let kv_store = create_kv_store_from_config(&config)?;
let shared_kv_store = Arc::new(kv_store);
let registry = NodeRegistry::new_with_kv_store(shared_kv_store);

// NodeRegistry和NodeResourceRegistry现在共享同一个KV存储
// NodeRegistry and NodeResourceRegistry now share the same KV storage

impl NodeRegistry {
    pub fn new() -> Self {
        Self::new_with_memory()
    }
    
    pub fn new_with_memory() -> Self {
        Self::new_with_kv_store(Box::new(MemoryKvStore::new()))
    }
    
    pub fn new_with_kv_store(kv_store: Box<dyn KvStore>) -> Self {
        Self { kv_store }
    }
}

impl NodeResourceRegistry {
    pub fn new() -> Self {
        Self::with_kv_store(Box::new(MemoryKvStore::new()))
    }
    
    pub fn with_kv_store(kv_store: Box<dyn KvStore>) -> Self {
        Self { kv_store }
    }
}
```

### 2. 命名空间分离 / Namespace Separation

使用前缀来区分不同类型的数据：
Use prefixes to distinguish different types of data:

```rust
// 节点命名空间 / Node namespace
pub mod namespace {
    pub const NODE_PREFIX: &str = "node:";
}

// 资源命名空间 / Resource namespace  
pub mod namespace {
    pub const RESOURCE_PREFIX: &str = "resource:";
}

// 键生成 / Key generation
pub mod keys {
    pub fn node_key(uuid: &Uuid) -> String {
        format!("{}:{}", namespace::NODE_PREFIX, uuid)
    }
    
    pub fn resource_key(uuid: &Uuid) -> String {
        format!("{}:{}", namespace::RESOURCE_PREFIX, uuid)
    }
}
```

### 3. 异步操作统一 / Unified Async Operations

所有操作现在都是异步的，提供更好的性能：
All operations are now asynchronous for better performance:

```rust
// 节点操作 / Node operations
impl NodeRegistry {
    pub async fn register_node(&mut self, node: NodeInfo) -> Result<Uuid, SmsError> {
        let key = keys::node_key(&node.uuid);
        let value = serialization::serialize(&node)?;
        self.kv_store.put(&key, &value).await?;
        Ok(node.uuid)
    }
    
    pub async fn get_node(&self, uuid: &Uuid) -> Result<Option<NodeInfo>, SmsError> {
        let key = keys::node_key(uuid);
        match self.kv_store.get(&key).await? {
            Some(data) => Ok(Some(serialization::deserialize(&data)?)),
            None => Ok(None),
        }
    }
}

// 资源操作 / Resource operations
impl NodeResourceRegistry {
    pub async fn update_resource(&mut self, mut resource: NodeResourceInfo) -> Result<(), SmsError> {
        resource.update_timestamp();
        self.store_resource_direct(resource).await
    }
    
    pub async fn get_resource(&self, node_uuid: &Uuid) -> Result<Option<NodeResourceInfo>, SmsError> {
        let key = keys::resource_key(node_uuid);
        match self.kv_store.get(&key).await? {
            Some(data) => Ok(Some(serialization::deserialize(&data)?)),
            None => Ok(None),
        }
    }
}
```

## 服务层集成 / Service Layer Integration

SPEAR Metadata Server Service现在使用统一的架构：
SPEAR Metadata Server Service now uses the unified architecture:

```rust
pub struct SmsServiceImpl {
    registry: Arc<RwLock<NodeRegistry>>,  // 统一的NodeRegistry / Unified NodeRegistry
    resource_registry: Arc<RwLock<NodeResourceRegistry>>,
}

impl SmsServiceImpl {
    pub fn new() -> Self {
        Self {
            registry: Arc::new(RwLock::new(NodeRegistry::new())),
            resource_registry: Arc::new(RwLock::new(NodeResourceRegistry::new())),
        }
    }
    
    pub fn with_kv_config(kv_store: Box<dyn KvStore>) -> Self {
        Self {
            registry: Arc::new(RwLock::new(NodeRegistry::new_with_kv_store(kv_store.clone()))),
            resource_registry: Arc::new(RwLock::new(NodeResourceRegistry::with_kv_store(kv_store))),
        }
    }
}
```

## 测试改进 / Testing Improvements

### 测试修复 / Test Fixes

1. **时间戳覆盖问题** / **Timestamp Override Issues**
   - 添加了`store_resource_direct`方法用于测试
   - Added `store_resource_direct` method for testing
   
2. **异步调用修复** / **Async Call Fixes**
   - 所有KV操作现在正确使用`.await`
   - All KV operations now properly use `.await`

3. **构造函数修复** / **Constructor Fixes**
   - 使用正确的构造函数方法
   - Use correct constructor methods

### 测试结果 / Test Results

```bash
test result: ok. 120 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out
```

所有120个测试现在都通过，确保重构的正确性。
All 120 tests now pass, ensuring the correctness of the refactoring.

## 配置示例 / Configuration Examples

### 内存存储 / Memory Storage

```rust
// 默认内存存储 / Default memory storage
let service = SmsServiceImpl::new();

// 显式内存存储 / Explicit memory storage  
let kv_store = Box::new(MemoryKvStore::new());
let service = SmsServiceImpl::with_kv_config(kv_store);
```

### 持久化存储 / Persistent Storage

#### 支持的存储后端 / Supported Storage Backends

目前支持以下存储后端：

1. **Memory Store (内存存储)**
   - 基于 HashMap 的内存存储
   - 适用于测试和开发环境
   - 数据不持久化

2. **Sled Store (Sled存储)**
   - 基于 Sled 嵌入式数据库
   - 提供持久化存储
   - 适用于生产环境

3. **RocksDB Store (RocksDB存储)**
   - 基于 RocksDB 高性能键值数据库
   - 提供高性能持久化存储
   - 适用于高负载生产环境
   - 需要启用 `rocksdb` feature

```rust
// Sled数据库存储 / Sled database storage
let config = KvStoreConfig::sled("/path/to/sled/db");
let kv_store = create_kv_store_from_config(&config).await?;

// RocksDB数据库存储 / RocksDB database storage (需要启用rocksdb feature)
#[cfg(feature = "rocksdb")]
{
    let config = KvStoreConfig::rocksdb("/path/to/rocksdb");
    let kv_store = create_kv_store_from_config(&config).await?;
}
```

### 环境变量配置 / Environment Variable Configuration

```bash
# 使用内存存储 / Use memory storage
export KV_BACKEND=memory

# 使用Sled存储 / Use Sled storage
export KV_BACKEND=sled
export KV_PATH=/path/to/sled/db

# 使用RocksDB存储 / Use RocksDB storage (需要启用rocksdb feature)
export KV_BACKEND=rocksdb
export KV_PATH=/path/to/rocksdb
```

## 性能优势 / Performance Benefits

1. **统一接口** / **Unified Interface**: 减少代码重复和维护成本
2. **可配置后端** / **Configurable Backend**: 根据需求选择存储类型
3. **异步操作** / **Async Operations**: 更好的并发性能
4. **命名空间隔离** / **Namespace Isolation**: 避免键冲突
5. **共享存储机制** / **Shared Storage Mechanism**: 使用Arc智能指针实现线程安全的存储共享

## 迁移指南 / Migration Guide

### 代码更新 / Code Updates

1. **移除KvNodeRegistry引用** / **Remove KvNodeRegistry References**
   ```rust
   // 旧代码 / Old code
   use crate::common::node::KvNodeRegistry;
   
   // 新代码 / New code  
   use crate::common::node::NodeRegistry;
   ```

2. **更新构造函数调用** / **Update Constructor Calls**
   ```rust
   // 旧代码 / Old code
   let registry = KvNodeRegistry::new(kv_store);
   
   // 新代码 / New code
   let registry = NodeRegistry::new_with_kv_store(kv_store);
   ```

3. **添加异步支持** / **Add Async Support**
   ```rust
   // 旧代码 / Old code
   let node = registry.get_node(&uuid);
   
   // 新代码 / New code
   let node = registry.get_node(&uuid).await?;
   ```

## 未来扩展 / Future Extensions

1. **分布式存储** / **Distributed Storage**: 支持Redis等分布式后端
2. **数据分片** / **Data Sharding**: 大规模部署的数据分片
3. **缓存层** / **Caching Layer**: 多级缓存优化
4. **监控指标** / **Monitoring Metrics**: 存储操作的性能监控

## 总结 / Summary

统一KV存储架构重构成功地：
The unified KV storage architecture refactoring successfully:

- ✅ 统一了存储接口 / Unified storage interfaces
- ✅ 提供了可配置的后端 / Provided configurable backends  
- ✅ 实现了命名空间隔离 / Implemented namespace isolation
- ✅ 保持了向后兼容性 / Maintained backward compatibility
- ✅ 通过了所有测试 / Passed all tests

这为项目的未来扩展和维护奠定了坚实的基础。
This lays a solid foundation for future expansion and maintenance of the project.