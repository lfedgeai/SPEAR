# KV Store Factory Pattern Documentation

## Overview

The KV Store Factory Pattern provides a flexible and configurable way to create different types of key-value stores at runtime. This pattern allows you to switch between different storage backends (memory, sled, etc.) based on configuration, environment variables, or runtime conditions.

## Key Components

### 1. KvStoreConfig

The `KvStoreConfig` struct holds configuration information for creating KV stores:

```rust
use spear_next::storage::KvStoreConfig;

// Create memory store configuration
let memory_config = KvStoreConfig::memory();

// Create sled store configuration (requires "sled" feature)
#[cfg(feature = "sled")]
let sled_config = KvStoreConfig::sled("/path/to/database");

// Add custom parameters
let config_with_params = KvStoreConfig::memory()
    .with_param("cache_size", "1000")
    .with_param("timeout", "30");
```

### 2. KvStoreFactory Trait

The `KvStoreFactory` trait defines the interface for creating KV stores:

```rust
use spear_next::storage::{KvStoreFactory, DefaultKvStoreFactory};

let factory = DefaultKvStoreFactory::new();

// Check supported backends
let backends = factory.supported_backends();
println!("Supported backends: {:?}", backends);

// Validate configuration
let config = KvStoreConfig::memory();
factory.validate_config(&config)?;

// Create store
let store = factory.create(&config).await?;
```

### 3. Global Factory Functions

Convenient functions for common use cases:

```rust
use spear_next::storage::{
    create_kv_store_from_config, 
    create_kv_store_from_env,
    get_kv_store_factory
};

// Create from configuration
let config = KvStoreConfig::memory();
let store = create_kv_store_from_config(&config).await?;

// Create from environment variables
let store = create_kv_store_from_env().await?;

// Get global factory instance
let factory = get_kv_store_factory();
```

## Configuration Methods

### 1. Programmatic Configuration

```rust
use spear_next::storage::KvStoreConfig;

// Memory store
let memory_config = KvStoreConfig::memory()
    .with_param("cache_size", "5000")
    .with_param("debug", "true");

// Sled store (requires "sled" feature)
#[cfg(feature = "sled")]
let sled_config = KvStoreConfig::sled("/var/lib/app/data")
    .with_param("cache_capacity", "100000")
    .with_param("flush_every_ms", "5000");
```

### 2. Environment Variable Configuration

Set environment variables to configure the KV store:

```bash
# Backend selection
export KV_STORE_BACKEND=memory

# Generic parameters (converted to lowercase)
export KV_STORE_CACHE_SIZE=5000
export KV_STORE_TIMEOUT=60
export KV_STORE_DEBUG=true

# For sled backend
export KV_STORE_BACKEND=sled
export KV_STORE_PATH=/path/to/database
```

Then create the store:

```rust
use spear_next::storage::create_kv_store_from_env;

let store = create_kv_store_from_env().await?;
```

### 3. Configuration File (JSON/TOML)

You can serialize/deserialize `KvStoreConfig` using serde:

```rust
use spear_next::storage::KvStoreConfig;
use serde_json;

// Serialize to JSON
let config = KvStoreConfig::memory().with_param("cache_size", "1000");
let json = serde_json::to_string(&config)?;

// Deserialize from JSON
let config: KvStoreConfig = serde_json::from_str(&json)?;
let store = create_kv_store_from_config(&config).await?;
```

## Usage Patterns

### 1. Application Mode-based Selection

```rust
use spear_next::storage::{KvStoreConfig, create_kv_store_from_config};

async fn create_store_for_mode(mode: &str) -> Result<Box<dyn KvStore>, Box<dyn std::error::Error>> {
    let config = match mode {
        "test" => KvStoreConfig::memory(),
        "dev" => KvStoreConfig::memory().with_param("debug", "true"),
        #[cfg(feature = "sled")]
        "prod" => KvStoreConfig::sled("/var/lib/app/data")
            .with_param("cache_capacity", "100000"),
        _ => return Err("Unsupported mode".into()),
    };
    
    Ok(create_kv_store_from_config(&config).await?)
}
```

### 2. Runtime Backend Switching

```rust
use spear_next::storage::{KvStoreConfig, create_kv_store_from_config};

async fn create_store_based_on_conditions() -> Result<Box<dyn KvStore>, Box<dyn std::error::Error>> {
    let config = if std::env::var("TESTING").is_ok() {
        KvStoreConfig::memory()
    } else if let Ok(db_path) = std::env::var("DATABASE_PATH") {
        #[cfg(feature = "sled")]
        { KvStoreConfig::sled(db_path) }
        #[cfg(not(feature = "sled"))]
        { KvStoreConfig::memory() }
    } else {
        KvStoreConfig::memory()
    };
    
    Ok(create_kv_store_from_config(&config).await?)
}
```

### 3. Custom Factory Implementation

```rust
use spear_next::storage::{KvStoreFactory, KvStoreConfig, KvStore};
use async_trait::async_trait;

#[derive(Debug)]
struct LoggingKvStoreFactory {
    inner: DefaultKvStoreFactory,
}

#[async_trait]
impl KvStoreFactory for LoggingKvStoreFactory {
    async fn create(&self, config: &KvStoreConfig) -> Result<Box<dyn KvStore>, SmsError> {
        println!("Creating KV store with backend: {}", config.backend);
        self.inner.create(config).await
    }
    
    fn supported_backends(&self) -> Vec<String> {
        self.inner.supported_backends()
    }
    
    fn validate_config(&self, config: &KvStoreConfig) -> Result<(), SmsError> {
        println!("Validating config for backend: {}", config.backend);
        self.inner.validate_config(config)
    }
}
```

## Error Handling

The factory pattern includes comprehensive error handling:

```rust
use spear_next::storage::{KvStoreConfig, create_kv_store_from_config};

async fn safe_create_store(config: &KvStoreConfig) -> Result<Box<dyn KvStore>, Box<dyn std::error::Error>> {
    // Validate configuration first
    let factory = get_kv_store_factory();
    factory.validate_config(config)?;
    
    // Create store
    let store = factory.create(config).await?;
    
    // Test basic functionality
    let test_key = "__health_check__".to_string();
    let test_value = "ok".as_bytes().to_vec();
    
    store.put(&test_key, &test_value).await?;
    let retrieved = store.get(&test_key).await?;
    store.delete(&test_key).await?;
    
    if retrieved != Some(test_value) {
        return Err("Store health check failed".into());
    }
    
    Ok(store)
}
```

## Environment Variables Reference

| Variable | Description | Example |
|----------|-------------|---------|
| `KV_STORE_BACKEND` | Backend type (memory, sled) | `memory` |
| `KV_STORE_CACHE_SIZE` | Cache size parameter | `5000` |
| `KV_STORE_TIMEOUT` | Timeout parameter | `60` |
| `KV_STORE_PATH` | Database path (for sled) | `/var/lib/app/data` |
| `KV_STORE_DEBUG` | Debug mode | `true` |
| `SPEAR_KV_BACKEND` | Legacy backend variable | `memory` |
| `SPEAR_KV_SLED_PATH` | Legacy sled path variable | `/path/to/db` |

## Best Practices

1. **Configuration Validation**: Always validate configuration before creating stores
2. **Error Handling**: Implement proper error handling for store creation failures
3. **Health Checks**: Test basic functionality after creating stores
4. **Environment Separation**: Use different configurations for different environments
5. **Parameter Documentation**: Document custom parameters for your application
6. **Factory Customization**: Implement custom factories for specialized requirements

## Testing

The factory pattern includes comprehensive tests:

```bash
# Run all factory-related tests
cargo test test_kv_store_config test_factory_validation test_global_factory test_config_from_env --lib --features sled

# Run specific test
cargo test test_new_kv_store_factory --lib --features sled
```

## Examples

See `examples/kv-factory-examples.rs` for complete working examples demonstrating all factory pattern features.