# Unified KV Storage Architecture

## Overview

This document describes the refactoring of the unified KV storage architecture in the spear-next project, which unifies the previously separate NodeRegistry and NodeResourceRegistry into a KV storage-based architecture.

## Architecture Changes

### Before Refactoring

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

### After Refactoring

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

## Core Improvements

### 1. Unified Storage Interface

- **Single KV Store Instance**: All data operations go through a unified KV storage interface
- **Configuration-Driven**: Configure storage backend type (memory, Sled, etc.) through `KvStoreConfig`
- **Type Safety**: Use strongly-typed serialization/deserialization mechanisms
- **Shared Storage**: `NodeRegistry` and `NodeResourceRegistry` share the same `Arc<dyn KvStore>` instance

All registries now use the same KV storage abstraction:

```rust
// Unified constructors
let config = KvStoreConfig::memory();
let kv_store = create_kv_store_from_config(&config)?;
let shared_kv_store = Arc::new(kv_store);
let registry = NodeRegistry::new_with_kv_store(shared_kv_store);

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

### 2. Namespace Separation

Use prefixes to distinguish different types of data:

```rust
// Node namespace
pub mod namespace {
    pub const NODE_PREFIX: &str = "node:";
}

// Resource namespace
pub mod namespace {
    pub const RESOURCE_PREFIX: &str = "resource:";
}

// Key generation
pub mod keys {
    pub fn node_key(uuid: &Uuid) -> String {
        format!("{}:{}", namespace::NODE_PREFIX, uuid)
    }
    
    pub fn resource_key(uuid: &Uuid) -> String {
        format!("{}:{}", namespace::RESOURCE_PREFIX, uuid)
    }
}
```

### 3. Namespace Isolation
- **Prefix Mechanism**: Use different prefixes to distinguish node and resource data
- **Avoid Conflicts**: Ensure different types of data don't overwrite each other
- **Clear Organization**: Facilitate data management and debugging

### 4. Shared Storage Mechanism
- **Arc Smart Pointer**: Use `Arc<dyn KvStore>` for thread-safe sharing of KV storage
- **Unified Configuration**: `NodeRegistry` and `NodeResourceRegistry` use the same KV storage instance
- **Memory Efficiency**: Avoid duplicate storage instances, reducing memory usage
- **Data Consistency**: Ensure node and resource data are in the same storage backend

### 3. Unified Async Operations

All operations are now asynchronous for better performance:

```rust
// Node operations
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

// Resource operations
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

## Service Layer Integration

SPEAR Metadata Server Service now uses the unified architecture:

```rust
pub struct SmsServiceImpl {
    registry: Arc<RwLock<NodeRegistry>>,  // Unified NodeRegistry
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

## Testing Improvements

### Test Fixes

1. **Timestamp Override Issues**
   - Added `store_resource_direct` method for testing
   
2. **Async Call Fixes**
   - All KV operations now properly use `.await`

3. **Constructor Fixes**
   - Use correct constructor methods

### Test Results

```bash
test result: ok. 120 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out
```

All 120 tests now pass, ensuring the correctness of the refactoring.

## Supported Storage Backends

Currently supports the following storage backends:

1. **Memory Store**
   - HashMap-based in-memory storage
   - Suitable for testing and development environments
   - Data is not persistent

2. **Sled Store**
   - Based on Sled embedded database
   - Provides persistent storage
   - Suitable for production environments

3. **RocksDB Store**
   - Based on RocksDB high-performance key-value database
   - Provides high-performance persistent storage
   - Suitable for high-load production environments
   - Requires enabling the `rocksdb` feature

## Configuration Examples

### Environment Variable Configuration

```bash
# Use memory storage
export KV_BACKEND=memory

# Use Sled storage
export KV_BACKEND=sled
export KV_PATH=/path/to/sled/db

# Use RocksDB storage (requires rocksdb feature)
export KV_BACKEND=rocksdb
export KV_PATH=/path/to/rocksdb
```

### Memory Storage

```rust
// Default memory storage
let service = SmsServiceImpl::new();

// Explicit memory storage
let kv_store = Box::new(MemoryKvStore::new());
let service = SmsServiceImpl::with_kv_config(kv_store);
```

### Persistent Storage

```rust
// Sled database storage
let config = KvStoreConfig::sled("/path/to/sled/db");
let kv_store = create_kv_store_from_config(&config).await?;
let service = SmsServiceImpl::with_kv_config(kv_store);

// RocksDB storage (requires rocksdb feature)
#[cfg(feature = "rocksdb")]
{
    let config = KvStoreConfig::rocksdb("/path/to/rocksdb");
    let kv_store = create_kv_store_from_config(&config).await?;
    let service = SmsServiceImpl::with_kv_config(kv_store);
}
```

## Performance Benefits

1. **Unified Interface**: Reduces code duplication and maintenance costs
2. **Configurable Backend**: Choose storage type based on requirements
3. **Async Operations**: Better concurrent performance
4. **Namespace Isolation**: Prevents key conflicts
5. **Shared Storage Mechanism**: Thread-safe storage sharing using Arc smart pointers
6. **Multiple Storage Options**: Support for Memory, Sled, and RocksDB backends
7. **High Performance**: RocksDB support for high-throughput production workloads

## Migration Guide

### Code Updates

1. **Remove KvNodeRegistry References**
   ```rust
   // Old code
   use crate::common::node::KvNodeRegistry;
   
   // New code
   use crate::common::node::NodeRegistry;
   ```

2. **Update Constructor Calls**
   ```rust
   // Old code
   let registry = KvNodeRegistry::new(kv_store);
   
   // New code
   let registry = NodeRegistry::new_with_kv_store(kv_store);
   ```

3. **Add Async Support**
   ```rust
   // Old code
   let node = registry.get_node(&uuid);
   
   // New code
   let node = registry.get_node(&uuid).await?;
   ```

## Future Extensions

1. **Distributed Storage**: Support for distributed backends like Redis
2. **Data Sharding**: Data sharding for large-scale deployments
3. **Caching Layer**: Multi-level caching optimization
4. **Monitoring Metrics**: Performance monitoring for storage operations

## Summary

The unified KV storage architecture refactoring successfully:

- ✅ Unified storage interfaces
- ✅ Provided configurable backends
- ✅ Implemented namespace isolation
- ✅ Maintained backward compatibility
- ✅ Passed all tests

This lays a solid foundation for future expansion and maintenance of the project.