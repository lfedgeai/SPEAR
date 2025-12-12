# SPEARlet KV 后端配置分析

## 概述

本文档分析了 SPEARlet 中 KV 后端的配置和使用模式，澄清了默认配置值与实际运行时行为之间的差异。

## 配置 vs 运行时行为

### 1. 默认配置值

在 `src/spearlet/config.rs` 中，`StorageConfig::default()` 实现设置：

```rust
impl Default for StorageConfig {
    fn default() -> Self {
        Self {
            backend: "rocksdb".to_string(),  // 默认是 RocksDB
            data_dir: "/tmp/spearlet".to_string(),
            max_cache_size_mb: 100,
            compression_enabled: false,
            max_object_size: 1024 * 1024, // 1MB
        }
    }
}
```

### 2. 主应用程序运行时

在 `src/bin/spearlet/main.rs` 中，应用程序：

1. **加载配置** 从 CLI 参数和配置文件
2. **创建 KV 存储** 使用 `create_kv_store_from_config()` 基于配置的后端
3. **创建 ObjectService** 使用配置的存储后端

```rust
let kv_store = create_kv_store_from_config(&kv_config).await?;
let object_service = Arc::new(ObjectServiceImpl::new(kv_store.into(), config.storage.max_object_size));
```

### 3. GrpcServer 实现问题

**重要发现**：在 `src/spearlet/grpc_server.rs` 中存在不一致性：

```rust
impl GrpcServer {
    pub fn new(config: Arc<SpearletConfig>) -> Self {
        // 这里忽略了 config.storage.backend 设置！
        let object_service = Arc::new(ObjectServiceImpl::new_with_memory(config.storage.max_object_size));
        // ...
    }
}
```

**这是一个潜在的 bug** - GrpcServer 总是使用内存后端，无论配置如何。

## 使用模式

### 1. 测试环境

所有测试文件都明确使用内存后端：

- **测试配置**：设置 `backend: "memory".to_string()`
- **直接创建**：使用 `ObjectServiceImpl::new_with_memory()`

**原因**：测试需要快速、隔离的存储，不依赖文件系统。

### 2. 生产环境

- **默认行为**：使用 RocksDB 进行持久化存储
- **可配置**：可通过配置文件或 CLI 参数更改

## 内存后端使用场景

### 何时使用内存后端：

1. **所有单元和集成测试**
2. **GrpcServer 实例**（由于实现问题）
3. **显式配置** 设置 `backend: "memory"`

### 何时使用 RocksDB 后端：

1. **主应用程序启动** 使用默认或显式的 RocksDB 配置
2. **生产部署** 需要数据持久化

## 建议

### 1. 修复 GrpcServer 实现

`GrpcServer::new()` 方法应该遵循存储配置：

```rust
pub fn new(config: Arc<SpearletConfig>) -> Self {
    // 基于配置创建 KV 存储
    let kv_store = create_kv_store_from_config(&kv_config).await?;
    let object_service = Arc::new(ObjectServiceImpl::new(kv_store, config.storage.max_object_size));
    // ...
}
```

### 2. 配置清晰度

考虑添加配置验证以确保指定的后端受支持。

### 3. 文档更新

更新配置文档以清楚说明：
- 默认后端行为
- 何时使用内存 vs 持久化存储
- 性能和持久化权衡

## 总结

- **配置默认值**：RocksDB
- **测试环境**：内存（显式配置）
- **GrpcServer**：内存（实现问题）
- **主应用程序**：遵循配置（生产环境通常是 RocksDB）

"spearlet 默认使用内存后端"这个说法对某些组件（测试、GrpcServer）是部分正确的，但对主应用程序运行时不正确。