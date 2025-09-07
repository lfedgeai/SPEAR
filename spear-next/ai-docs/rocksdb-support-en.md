# RocksDB Support Documentation

## Overview

This document explains how to use RocksDB as a KV storage backend in the Spear-Next project. RocksDB is a high-performance embedded key-value database, particularly suitable for high-load production environments.

## Enabling RocksDB Feature

### 1. Enable at Compile Time

```bash
# Enable rocksdb feature during compilation
cargo build --features rocksdb

# Run tests (including RocksDB tests)
cargo test --features rocksdb

# Release build
cargo build --release --features rocksdb
```

### 2. Cargo.toml Configuration

```toml
[dependencies]
rocksdb = { version = "0.22", optional = true }

[features]
default = []
rocksdb = ["dep:rocksdb"]
```

## Usage

### 1. Basic Configuration

```rust
use spear_next::storage::kv::{KvStoreConfig, create_kv_store_from_config};

#[cfg(feature = "rocksdb")]
async fn setup_rocksdb_storage() -> Result<(), Box<dyn std::error::Error>> {
    // Create RocksDB configuration
    let config = KvStoreConfig::rocksdb("/path/to/rocksdb/data");
    
    // Create storage instance
    let kv_store = create_kv_store_from_config(&config).await?;
    
    // Use storage
    kv_store.put(&"key".to_string(), &b"value".to_vec()).await?;
    let value = kv_store.get(&"key".to_string()).await?;
    
    Ok(())
}
```

### 2. Environment Variable Configuration

```bash
# Set environment variables
export KV_BACKEND=rocksdb
export KV_PATH=/var/lib/spear/rocksdb

# Create config from environment
```

```rust
use spear_next::storage::kv::{KvStoreConfig, create_kv_store_from_env};

async fn setup_from_env() -> Result<(), Box<dyn std::error::Error>> {
    let kv_store = create_kv_store_from_env().await?;
    // Use storage
    Ok(())
}
```

### 3. Integration with NodeRegistry

```rust
use spear_next::common::{NodeRegistry, NodeResourceRegistry};
use spear_next::storage::kv::KvStoreConfig;
use std::sync::Arc;

#[cfg(feature = "rocksdb")]
async fn setup_with_rocksdb() -> Result<(), Box<dyn std::error::Error>> {
    // Create shared KV storage
    let config = KvStoreConfig::rocksdb("/path/to/rocksdb");
    let kv_store = Arc::new(create_kv_store_from_config(&config).await?);
    
    // Create registries
    let node_registry = NodeRegistry::new_with_kv_store(kv_store.clone());
    let resource_registry = NodeResourceRegistry::new_with_kv_store(kv_store.clone());
    
    Ok(())
}
```

## Performance Characteristics

### 1. High Throughput
- RocksDB is optimized for high write loads
- Supports batch operations for improved performance
- LSM-tree structure provides excellent write performance

### 2. Data Compression
- Built-in multiple compression algorithms (LZ4, Snappy, ZSTD)
- Automatic compression to save storage space
- Configurable compression strategies

### 3. Durability Guarantees
- WAL (Write-Ahead Log) ensures data durability
- Configurable sync strategies
- Crash recovery mechanisms

## Configuration Options

### 1. Basic Configuration

```rust
// Use default configuration
let config = KvStoreConfig::rocksdb("/path/to/db");

// Custom configuration parameters
let config = KvStoreConfig::rocksdb("/path/to/db")
    .with_param("max_open_files", "1000")
    .with_param("write_buffer_size", "67108864"); // 64MB
```

### 2. Advanced Configuration

```rust
use rocksdb::{DB, Options};

// Note: Direct RocksDB API usage requires implementation at application layer
```

## Troubleshooting

### 1. Compilation Issues

**Problem**: RocksDB compilation failure
```
error: failed to run custom build command for `rocksdb-sys`
```

**Solution**:
```bash
# macOS
brew install rocksdb

# Ubuntu/Debian
sudo apt-get install librocksdb-dev

# Or use static linking
export ROCKSDB_STATIC=1
cargo build --features rocksdb
```

### 2. Runtime Issues

**Problem**: Database open failure
```
Error: Failed to open RocksDB: IO error: lock /path/to/db/LOCK: Resource temporarily unavailable
```

**Solution**:
- Ensure no other process is using the same database directory
- Check file permissions
- Ensure directory exists and is writable

### 3. Performance Issues

**Problem**: Slow write performance

**Solution**:
- Increase write_buffer_size
- Enable batch writes
- Adjust compression strategy
- Use SSD storage

## Best Practices

### 1. Directory Management
```bash
# Use different data directories for different environments
/var/lib/spear/rocksdb/production
/var/lib/spear/rocksdb/staging
/tmp/spear/rocksdb/development
```

### 2. Backup Strategy
```bash
# Regular database backup
cp -r /var/lib/spear/rocksdb /backup/spear-$(date +%Y%m%d)
```

### 3. Monitoring
```rust
// Regular database statistics check
let count = kv_store.count().await?;
println!("Total keys: {}", count);
```

## Testing

### 1. Unit Tests
```bash
# Run RocksDB-related tests
cargo test --features rocksdb test_rocksdb
```

### 2. Integration Tests
```bash
# Run full integration tests
cargo test --features rocksdb
```

### 3. Performance Tests
```bash
# Run performance benchmarks
cargo bench --features rocksdb
```

## Migration Guide

### Migrating from Memory Storage
```rust
// 1. Export existing data
let memory_store = MemoryKvStore::new();
let all_data = memory_store.all().await?;

// 2. Create RocksDB storage
let config = KvStoreConfig::rocksdb("/path/to/rocksdb");
let rocksdb_store = create_kv_store_from_config(&config).await?;

// 3. Import data
for pair in all_data {
    rocksdb_store.put(&pair.key, &pair.value).await?;
}
```

### Migrating from Sled Storage
```rust
// Similar migration process
// Can use the same KvStore trait methods
```

## Version Compatibility

| Spear-Next Version | RocksDB Version | Compatibility |
|-------------------|-----------------|---------------|
| 0.1.0+            | 0.22.x          | ✅ Full Support |

## References

- [RocksDB Official Documentation](https://rocksdb.org/)
- [RocksDB Rust Bindings](https://docs.rs/rocksdb/)
- [LSM-tree Principles](https://en.wikipedia.org/wiki/Log-structured_merge-tree)