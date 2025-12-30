# RocksDB 支持说明 / RocksDB Support Documentation

## 概述 / Overview

本文档说明如何在 Spear-Next 项目中使用 RocksDB 作为 KV 存储后端。RocksDB 是一个高性能的嵌入式键值数据库，特别适合高负载的生产环境。

## 启用 RocksDB 功能 / Enabling RocksDB Feature

### 1. 编译时启用 / Enable at Compile Time

```bash
# 编译时启用 rocksdb feature
cargo build --features rocksdb

# 运行测试（包括 RocksDB 测试）
cargo test --features rocksdb

# 发布构建
cargo build --release --features rocksdb
```

### 2. Cargo.toml 配置 / Cargo.toml Configuration

```toml
[dependencies]
rocksdb = { version = "0.22", optional = true }

[features]
default = []
rocksdb = ["dep:rocksdb"]
```

## 使用方法 / Usage

### 1. 基本配置 / Basic Configuration

```rust
use spear_next::storage::kv::{KvStoreConfig, create_kv_store_from_config};

#[cfg(feature = "rocksdb")]
async fn setup_rocksdb_storage() -> Result<(), Box<dyn std::error::Error>> {
    // 创建 RocksDB 配置 / Create RocksDB configuration
    let config = KvStoreConfig::rocksdb("/path/to/rocksdb/data");
    
    // 创建存储实例 / Create storage instance
    let kv_store = create_kv_store_from_config(&config).await?;
    
    // 使用存储 / Use storage
    kv_store.put(&"key".to_string(), &b"value".to_vec()).await?;
    let value = kv_store.get(&"key".to_string()).await?;
    
    Ok(())
}
```

### 2. 环境变量配置 / Environment Variable Configuration

```bash
# 设置环境变量 / Set environment variables
export KV_BACKEND=rocksdb
export KV_PATH=/var/lib/spear/rocksdb

# 从环境变量创建配置 / Create config from environment
```

```rust
use spear_next::storage::kv::{KvStoreConfig, create_kv_store_from_env};

async fn setup_from_env() -> Result<(), Box<dyn std::error::Error>> {
    let kv_store = create_kv_store_from_env().await?;
    // 使用存储 / Use storage
    Ok(())
}
```

### 3. 与 NodeRegistry 集成 / Integration with NodeRegistry

```rust
use spear_next::common::{NodeRegistry, NodeResourceRegistry};
use spear_next::storage::kv::KvStoreConfig;
use std::sync::Arc;

#[cfg(feature = "rocksdb")]
async fn setup_with_rocksdb() -> Result<(), Box<dyn std::error::Error>> {
    // 创建共享的 KV 存储 / Create shared KV storage
    let config = KvStoreConfig::rocksdb("/path/to/rocksdb");
    let kv_store = Arc::new(create_kv_store_from_config(&config).await?);
    
    // 创建 NodeRegistry 和 NodeResourceRegistry / Create registries
    let node_registry = NodeRegistry::new_with_kv_store(kv_store.clone());
    let resource_registry = NodeResourceRegistry::new_with_kv_store(kv_store.clone());
    
    Ok(())
}
```

## 性能特性 / Performance Characteristics

### 1. 高吞吐量 / High Throughput
- RocksDB 针对高写入负载进行了优化
- 支持批量操作以提高性能
- LSM-tree 结构提供优秀的写入性能

### 2. 数据压缩 / Data Compression
- 内置多种压缩算法（LZ4、Snappy、ZSTD）
- 自动压缩以节省存储空间
- 可配置的压缩策略

### 3. 持久化保证 / Durability Guarantees
- WAL（Write-Ahead Log）确保数据持久性
- 可配置的同步策略
- 崩溃恢复机制

## 配置选项 / Configuration Options

### 1. 基本配置 / Basic Configuration

```rust
// 使用默认配置 / Use default configuration
let config = KvStoreConfig::rocksdb("/path/to/db");

// 自定义配置参数 / Custom configuration parameters
let config = KvStoreConfig::rocksdb("/path/to/db")
    .with_param("max_open_files", "1000")
    .with_param("write_buffer_size", "67108864"); // 64MB
```

### 2. 高级配置 / Advanced Configuration

```rust
use rocksdb::{DB, Options};

// 注意：直接使用 RocksDB API 需要在应用层实现
// Note: Direct RocksDB API usage requires implementation at application layer
```

## 故障排除 / Troubleshooting

### 1. 编译问题 / Compilation Issues

**问题**: RocksDB 编译失败
```
error: failed to run custom build command for `rocksdb-sys`
```

**解决方案**:
```bash
# macOS
brew install rocksdb

# Ubuntu/Debian
sudo apt-get install librocksdb-dev

# 或者使用静态链接 / Or use static linking
export ROCKSDB_STATIC=1
cargo build --features rocksdb
```

### 2. 运行时问题 / Runtime Issues

**问题**: 数据库打开失败
```
Error: Failed to open RocksDB: IO error: lock /path/to/db/LOCK: Resource temporarily unavailable
```

**解决方案**:
- 确保没有其他进程在使用同一个数据库目录
- 检查文件权限
- 确保目录存在且可写

### 3. 性能问题 / Performance Issues

**问题**: 写入性能较慢

**解决方案**:
- 增加 write_buffer_size
- 启用批量写入
- 调整压缩策略
- 使用 SSD 存储

## 最佳实践 / Best Practices

### 1. 目录管理 / Directory Management
```bash
# 为不同环境使用不同的数据目录
# Use different data directories for different environments
/var/lib/spear/rocksdb/production
/var/lib/spear/rocksdb/staging
/tmp/spear/rocksdb/development
```

### 2. 备份策略 / Backup Strategy
```bash
# 定期备份数据库 / Regular database backup
cp -r /var/lib/spear/rocksdb /backup/spear-$(date +%Y%m%d)
```

### 3. 监控 / Monitoring
```rust
// 定期检查数据库统计信息 / Regular database statistics check
let count = kv_store.count().await?;
println!("Total keys: {}", count);
```

## 测试 / Testing

### 1. 单元测试 / Unit Tests
```bash
# 运行 RocksDB 相关测试 / Run RocksDB-related tests
cargo test --features rocksdb test_rocksdb
```

### 2. 集成测试 / Integration Tests
```bash
# 运行完整的集成测试 / Run full integration tests
cargo test --features rocksdb
```

### 3. 性能测试 / Performance Tests
```bash
# 运行性能基准测试 / Run performance benchmarks
cargo bench --features rocksdb
```

## 迁移指南 / Migration Guide

### 从内存存储迁移 / Migrating from Memory Storage
```rust
// 1. 导出现有数据 / Export existing data
let memory_store = MemoryKvStore::new();
let all_data = memory_store.all().await?;

// 2. 创建 RocksDB 存储 / Create RocksDB storage
let config = KvStoreConfig::rocksdb("/path/to/rocksdb");
let rocksdb_store = create_kv_store_from_config(&config).await?;

// 3. 导入数据 / Import data
for pair in all_data {
    rocksdb_store.put(&pair.key, &pair.value).await?;
}
```

### 从 Sled 存储迁移 / Migrating from Sled Storage
```rust
// 类似的迁移过程 / Similar migration process
// 可以使用相同的 KvStore trait 方法 / Can use the same KvStore trait methods
```

## 版本兼容性 / Version Compatibility

| Spear-Next Version | RocksDB Version | 兼容性 / Compatibility |
|-------------------|-----------------|----------------------|
| 0.1.0+            | 0.22.x          | ✅ 完全支持 / Full Support |

## 参考资源 / References

- [RocksDB 官方文档](https://rocksdb.org/)
- [RocksDB Rust 绑定](https://docs.rs/rocksdb/)
- [LSM-tree 原理](https://en.wikipedia.org/wiki/Log-structured_merge-tree)