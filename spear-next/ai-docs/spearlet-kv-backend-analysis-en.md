# SPEARlet KV Backend Configuration Analysis

## Overview

This document analyzes the KV backend configuration and usage patterns in SPEARlet, clarifying the difference between default configuration values and actual runtime behavior.

## Configuration vs Runtime Behavior

### 1. Default Configuration Values

In `src/spearlet/config.rs`, the `StorageConfig::default()` implementation sets:

```rust
impl Default for StorageConfig {
    fn default() -> Self {
        Self {
            backend: "rocksdb".to_string(),  // Default is RocksDB
            data_dir: "/tmp/spearlet".to_string(),
            max_cache_size_mb: 100,
            compression_enabled: false,
            max_object_size: 1024 * 1024, // 1MB
        }
    }
}
```

### 2. Main Application Runtime

In `src/bin/spearlet/main.rs`, the application:

1. **Loads configuration** from CLI args and config files
2. **Creates KV store** using `create_kv_store_from_config()` based on the configured backend
3. **Creates ObjectService** with the configured storage backend

```rust
let kv_store = create_kv_store_from_config(&kv_config).await?;
let object_service = Arc::new(ObjectServiceImpl::new(kv_store.into(), config.storage.max_object_size));
```

### 3. GrpcServer Implementation Issue

**Important Discovery**: In `src/spearlet/grpc_server.rs`, there's an inconsistency:

```rust
impl GrpcServer {
    pub fn new(config: Arc<SpearletConfig>) -> Self {
        // This ignores the config.storage.backend setting!
        let object_service = Arc::new(ObjectServiceImpl::new_with_memory(config.storage.max_object_size));
        // ...
    }
}
```

**This is a potential bug** - the GrpcServer always uses memory backend regardless of configuration.

## Usage Patterns

### 1. Test Environment

All test files explicitly use memory backend:

- **Test configurations**: Set `backend: "memory".to_string()`
- **Direct creation**: Use `ObjectServiceImpl::new_with_memory()`

**Rationale**: Tests need fast, isolated storage without filesystem dependencies.

### 2. Production Environment

- **Default behavior**: Uses RocksDB for persistent storage
- **Configurable**: Can be changed via configuration files or CLI arguments

## Memory Backend Usage Scenarios

### When Memory Backend is Used:

1. **All unit and integration tests**
2. **GrpcServer instances** (due to the implementation issue)
3. **Explicit configuration** setting `backend: "memory"`

### When RocksDB Backend is Used:

1. **Main application startup** with default or explicit RocksDB configuration
2. **Production deployments** requiring data persistence

## Recommendations

### 1. Fix GrpcServer Implementation

The `GrpcServer::new()` method should respect the storage configuration:

```rust
pub fn new(config: Arc<SpearletConfig>) -> Self {
    // Create KV store based on configuration
    let kv_store = create_kv_store_from_config(&kv_config).await?;
    let object_service = Arc::new(ObjectServiceImpl::new(kv_store, config.storage.max_object_size));
    // ...
}
```

### 2. Configuration Clarity

Consider adding configuration validation to ensure the specified backend is supported.

### 3. Documentation Updates

Update configuration documentation to clearly explain:
- Default backend behavior
- When memory vs persistent storage is used
- Performance and persistence trade-offs

## Summary

- **Configuration default**: RocksDB
- **Test environment**: Memory (explicitly configured)
- **GrpcServer**: Memory (implementation issue)
- **Main application**: Follows configuration (typically RocksDB in production)

The statement "spearlet defaults to memory backend" is partially correct for certain components (tests, GrpcServer) but not for the main application runtime.