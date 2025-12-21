# KV Abstraction Layer

## Overview

The KV (Key-Value) abstraction layer provides a unified interface for different storage backends in the spear-next project. This abstraction allows the system to seamlessly switch between different storage implementations while maintaining consistent API semantics.

## Architecture

### Core Components

1. **KvStore Trait**: The main abstraction interface that defines all storage operations
2. **MemoryKvStore**: In-memory implementation for testing and development
3. **SledKvStore**: Persistent storage implementation using Sled database
4. **Serialization Module**: Helper functions for data serialization and key generation

### Key Features

- **Async/Await Support**: All operations are asynchronous for better performance
- **Type Safety**: Strong typing with `KvKey` (String) and `KvValue` (Vec<u8>)
- **Range Queries**: Support for prefix-based and range-based queries
- **Batch Operations**: Efficient batch put and delete operations
- **Serialization Helpers**: Built-in JSON serialization for complex data types

## API Reference

### KvStore Trait

```rust
#[async_trait]
pub trait KvStore: Send + Sync + Debug {
    // Basic CRUD operations
    async fn get(&self, key: &KvKey) -> Result<Option<KvValue>, SmsError>;
    async fn put(&self, key: &KvKey, value: &KvValue) -> Result<(), SmsError>;
    async fn delete(&self, key: &KvKey) -> Result<bool, SmsError>;
    async fn exists(&self, key: &KvKey) -> Result<bool, SmsError>;
    
    // Query operations
    async fn keys_with_prefix(&self, prefix: &str) -> Result<Vec<KvKey>, SmsError>;
    async fn range(&self, options: &RangeOptions) -> Result<Vec<KvPair>, SmsError>;
    async fn all(&self) -> Result<Vec<KvPair>, SmsError>;
    
    // Utility operations
    async fn count(&self) -> Result<usize, SmsError>;
    async fn clear(&self) -> Result<(), SmsError>;
    
    // Batch operations
    async fn batch_put(&self, pairs: &[KvPair]) -> Result<(), SmsError>;
    async fn batch_delete(&self, keys: &[KvKey]) -> Result<usize, SmsError>;
}
```

### Range Query Options

```rust
#[derive(Debug, Clone, Default)]
pub struct RangeOptions {
    pub start_key: Option<KvKey>,    // Inclusive start key
    pub end_key: Option<KvKey>,      // Exclusive end key
    pub limit: Option<usize>,        // Maximum number of results
    pub reverse: bool,               // Reverse order
}
```

## Usage Examples

### Basic Operations

```rust
use spear_next::storage::{MemoryKvStore, KvStore};

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    let store = MemoryKvStore::new();
    
    // Put a value
    store.put(&"key1".to_string(), &b"value1".to_vec()).await?;
    
    // Get a value
    if let Some(value) = store.get(&"key1".to_string()).await? {
        println!("Value: {:?}", String::from_utf8(value)?);
    }
    
    // Check existence
    let exists = store.exists(&"key1".to_string()).await?;
    println!("Key exists: {}", exists);
    
    // Delete a key
    let deleted = store.delete(&"key1".to_string()).await?;
    println!("Key deleted: {}", deleted);
    
    Ok(())
}
```

### Working with Serialization

```rust
use spear_next::storage::{MemoryKvStore, KvStore, serialization};
use spear_next::common::node::NodeInfo;

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    let store = MemoryKvStore::new();
    let node = NodeInfo::new("127.0.0.1".to_string(), 8080);
    
    // Serialize and store a node
    let key = serialization::node_key(&node.uuid);
    let value = serialization::serialize(&node)?;
    store.put(&key, &value).await?;
    
    // Retrieve and deserialize
    if let Some(data) = store.get(&key).await? {
        let retrieved_node: NodeInfo = serialization::deserialize(&data)?;
        println!("Retrieved node: {:?}", retrieved_node);
    }
    
    Ok(())
}
```

### Range Queries

```rust
use spear_next::storage::{MemoryKvStore, KvStore, RangeOptions};

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    let store = MemoryKvStore::new();
    
    // Insert some test data
    store.put(&"node:001".to_string(), &b"data1".to_vec()).await?;
    store.put(&"node:002".to_string(), &b"data2".to_vec()).await?;
    store.put(&"resource:001".to_string(), &b"resource1".to_vec()).await?;
    
    // Query all keys with "node:" prefix
    let node_keys = store.keys_with_prefix("node:").await?;
    println!("Node keys: {:?}", node_keys);
    
    // Range query with options
    let options = RangeOptions::new()
        .start_key("node:")
        .end_key("node:999")
        .limit(10);
    
    let results = store.range(&options).await?;
    for pair in results {
        println!("Key: {}, Value: {:?}", pair.key, String::from_utf8(pair.value)?);
    }
    
    Ok(())
}
```

### Using Sled Backend

```rust
use spear_next::storage::{SledKvStore, KvStore, create_kv_store, KvStoreType};

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    // Create Sled store directly
    let store = SledKvStore::new("/tmp/test_db")?;
    
    // Or use the factory function
    let store = create_kv_store(KvStoreType::Sled { 
        path: "/tmp/test_db".to_string() 
    })?;
    
    // Use the same API as MemoryKvStore
    store.put(&"persistent_key".to_string(), &b"persistent_value".to_vec()).await?;
    
    Ok(())
}
```

## Integration with NodeRegistry

The KV abstraction layer is integrated into the `NodeRegistry` through the `KvNodeRegistry` implementation:

```rust
use spear_next::storage::{MemoryKvStore, KvStore};
use spear_next::common::node::{KvNodeRegistry, NodeInfo};

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    let kv_store: Box<dyn KvStore> = Box::new(MemoryKvStore::new());
    let mut registry = KvNodeRegistry::new(kv_store);
    
    let node = NodeInfo::new("127.0.0.1".to_string(), 8080);
    let uuid = registry.register_node(node).await?;
    
    if let Some(retrieved_node) = registry.get_node(&uuid).await? {
        println!("Retrieved node: {:?}", retrieved_node);
    }
    
    Ok(())
}
```

## Performance Considerations

### MemoryKvStore
- **Pros**: Extremely fast, no I/O overhead
- **Cons**: Data is lost on restart, limited by available RAM
- **Use Case**: Testing, development, temporary caching

### SledKvStore
- **Pros**: Persistent storage, good performance, ACID transactions
- **Cons**: Disk I/O overhead, requires file system access
- **Use Case**: Production environments, data persistence requirements

## Error Handling

All KV operations return `Result<T, SmsError>` where `SmsError` provides detailed error information:

```rust
use spear_next::common::error::SmsError;

match store.get(&key).await {
    Ok(Some(value)) => println!("Found value: {:?}", value),
    Ok(None) => println!("Key not found"),
    Err(SmsError::SerializationError { message }) => {
        eprintln!("Serialization error: {}", message);
    },
    Err(SmsError::StorageError { message }) => {
        eprintln!("Storage error: {}", message);
    },
    Err(e) => eprintln!("Other error: {:?}", e),
}
```

## Testing

The KV abstraction layer includes a comprehensive test suite covering basic functionality, integration tests, performance tests, and edge case testing:

### Basic Unit Tests
```bash
# Run all basic KV tests
cargo test storage::kv

# Tests with Sled feature
cargo test storage::kv --features sled

# Run NodeRegistry integration tests
cargo test common::node --features sled
```

### Integration Tests
```bash
# Run KV storage integration tests (cross-backend compatibility, performance comparison, concurrent operations, etc.)
cargo test --test kv_storage_integration_tests --features sled

# Run KV storage edge case tests (error handling, large data processing, memory pressure testing, etc.)
cargo test --test kv_storage_edge_cases --features sled
```

### Test Coverage

#### Integration Tests (kv_storage_integration_tests.rs)
- **Cross-backend Compatibility**: Verify consistency between Memory and Sled backends
- **Performance Comparison**: Compare read/write performance across different backends
- **Large Data Handling**: Test storage and retrieval of large key-value pairs
- **Range Operations**: Verify prefix scanning and range query functionality
- **Concurrent Operations**: Test data consistency in multi-threaded environments
- **Resource Cleanup**: Verify resource management and cleanup mechanisms
- **Error Handling**: Test handling of various error scenarios
- **Factory Configuration Validation**: Test storage factory configuration validation

#### Edge Case Tests (kv_storage_edge_cases.rs)
- **Empty and Whitespace Keys**: Handle edge cases with special key values
- **Problematic Values**: Test complex values with special characters, Unicode, JSON, etc.
- **Large Key-Value Tests**: Test extremely large keys and values (up to 100MB)
- **Concurrent Same-Key Access**: Test concurrent read/write operations on the same key
- **Rapid Creation/Deletion**: Test high-frequency key creation and deletion operations
- **Scan Operation Boundaries**: Test various boundary conditions for scan operations
- **Memory Pressure Testing**: Test memory limits and pressure situations

### Test Features
- **Multi-backend Support**: All tests run on both Memory and Sled backends
- **Async Testing**: Uses tokio for asynchronous operation testing
- **Timeout Protection**: Prevents long-running tests from blocking
- **Resource Cleanup**: Automatic cleanup of test data and temporary files
- **Performance Benchmarking**: Includes basic performance measurement and comparison

## Future Enhancements

- **Redis Backend**: Add Redis support for distributed caching
- **Compression**: Optional value compression for large data
- **Encryption**: At-rest encryption for sensitive data
- **Metrics**: Built-in performance and usage metrics
- **Backup/Restore**: Automated backup and restore functionality