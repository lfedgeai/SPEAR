# Sled Build Error Fix Guide

## Problem Description

When building the project with `cargo build --features sled`, the following compilation errors occur:

```
error[E0425]: cannot find function `create_kv_store` in this scope
  --> src/services/resource.rs:148:25
   |
148|         let kv_store = create_kv_store(KvStoreType::Sled { path: db_path.to_string() })?;
   |                        ^^^^^^^^^^^^^^^ not found in this scope
   |
help: consider importing this function:
   |
1  + use crate::create_kv_store;
   |

error[E0412]: cannot find type `KvStoreType` in this scope
  --> src/services/resource.rs:148:40
   |
148|         let kv_store = create_kv_store(KvStoreType::Sled { path: db_path.to_string() })?;
   |                                        ^^^^^^^^^^^ not found in this scope
   |
help: consider importing this type:
   |
1  + use crate::KvStoreType;
   |
```

## Root Cause Analysis

The error occurs in `src/services/resource.rs` at line 148, within the `new_with_sled` function:

```rust
#[cfg(feature = "sled")]
pub fn new_with_sled(db_path: &str) -> Result<Self, SmsError> {
    let kv_store = create_kv_store(KvStoreType::Sled { path: db_path.to_string() })?;
    Ok(Self::with_kv_store(Arc::new(kv_store)))
}
```

The issue is missing import statements for:
- `create_kv_store` function
- `KvStoreType` enum

Both are defined in the `src/storage/kv.rs` module but are not imported in `resource.rs`.

## Fix Solution

### Step 1: Add Missing Imports

In `src/services/resource.rs` file, modify the import statement:

**Before:**
```rust
use crate::storage::{KvStore, serialization, MemoryKvStore};
```

**After:**
```rust
use crate::storage::{KvStore, serialization, MemoryKvStore, create_kv_store, KvStoreType};
```

### Step 2: Verify the Fix

1. **Rebuild the project:**
   ```bash
   cargo build --features sled
   ```

2. **Verify successful build:**
   ```bash
   ✓ Compiling spear-next v0.1.0 (/path/to/spear-next)
   ✓ Finished `dev` profile [unoptimized + debuginfo] target(s) in 8.22s
   ```

3. **Test Sled configuration run:**
   ```bash
   cargo run --features sled --bin sms -- --config config-sled.toml
   ```

4. **Verify service startup logs:**
   ```
   INFO sms: KV store backend: sled
   INFO sms: KV store path: ./data/node-service.db
   INFO sms: HTTP gateway listening on 0.0.0.0:8080
   INFO sms: Swagger UI available at: http://0.0.0.0:8080/swagger-ui/
   ```

5. **Test API functionality:**
   ```bash
   curl -s http://localhost:8080/api/v1/nodes | jq .
   ```

## Technical Details

### Related Module Structure

```
src/
├── storage/
│   └── kv.rs              # Defines create_kv_store and KvStoreType
└── services/
    └── resource.rs        # Resource service using KV storage
```

### KvStoreType Enum Definition

```rust
#[derive(Debug, Clone)]
pub enum KvStoreType {
    Memory,
    #[cfg(feature = "sled")]
    Sled { path: String },
    #[cfg(feature = "rocksdb")]
    RocksDb { path: String },
}
```

### create_kv_store Function Signature

```rust
pub fn create_kv_store(store_type: KvStoreType) -> Result<Box<dyn KvStore>, SmsError>
```

## Configuration File Example

`config-sled.toml` configuration file content:

```toml
[sms]
grpc_addr = "0.0.0.0:50051"
http_addr = "0.0.0.0:8080"
cleanup_interval = 300
heartbeat_timeout = 120
enable_swagger = true

[sms.kv_store]
backend = "sled"
path = "./data/node-service.db"
cache_capacity = "10000"
flush_every_ms = "5000"
compression = "true"
```

## Best Practices

1. **Feature Dependency Check:** Ensure corresponding Cargo features are enabled when using specific backends
2. **Import Completeness:** Ensure all necessary imports are added when using cross-module functions
3. **Build Verification:** Test builds under different feature combinations to ensure code compatibility
4. **Configuration Validation:** Test service startup and API functionality with actual configuration files

## Related Documentation

- [KV Storage Architecture Documentation](./unified-kv-architecture-en.md)
- [KV Factory Pattern Documentation](./kv-factory-pattern-en.md)
- [API Usage Guide](./api-usage-guide-en.md)
- [Configuration Guide](./config-guide-en.md)

## Troubleshooting

If you still encounter build issues:

1. **Clean build cache:**
   ```bash
   cargo clean
   cargo build --features sled
   ```

2. **Check dependency versions:**
   ```bash
   cargo tree --features sled
   ```

3. **Verify feature enablement:**
   ```bash
   cargo build --features sled --verbose
   ```

4. **Check Sled dependency:**
   Ensure `Cargo.toml` contains the sled dependency:
   ```toml
   [dependencies]
   sled = { version = "0.34", optional = true }
   
   [features]
   sled = ["dep:sled"]
   ```