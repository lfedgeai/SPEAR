//! KV Abstraction Layer Usage Examples
//! KV抽象层使用示例
//!
//! This file contains practical examples of how to use the KV abstraction layer
//! in different scenarios within the spear-next project.
//! 此文件包含在spear-next项目中不同场景下如何使用KV抽象层的实际示例。

use spear_next::storage::{KvStore, RangeOptions, KvPair};
use spear_next::sms::error::SmsError;
use uuid;
use chrono;

// Note: These examples are for documentation purposes and may not compile
// without proper imports and dependencies in a real project.
// 注意：这些示例仅用于文档目的，在真实项目中可能需要适当的导入和依赖才能编译。

/// Example 1: Basic KV Operations
/// 示例1：基本KV操作
async fn example_basic_operations() -> Result<(), Box<dyn std::error::Error>> {
    use spear_next::storage::{MemoryKvStore, KvStore};
    
    println!("=== Basic KV Operations Example ===");
    println!("=== 基本KV操作示例 ===");
    
    let store = MemoryKvStore::new();
    
    // Store some data / 存储一些数据
    store.put(&"user:123".to_string(), &b"John Doe".to_vec()).await?;
    store.put(&"user:456".to_string(), &b"Jane Smith".to_vec()).await?;
    store.put(&"config:timeout".to_string(), &b"30".to_vec()).await?;
    
    // Retrieve data / 检索数据
    if let Some(value) = store.get(&"user:123".to_string()).await? {
        println!("User 123: {}", String::from_utf8(value)?);
    }
    
    // Check if key exists / 检查键是否存在
    let exists = store.exists(&"user:999".to_string()).await?;
    println!("User 999 exists: {}", exists);
    
    // Get all keys with prefix / 获取具有前缀的所有键
    let user_keys = store.keys_with_prefix("user:").await?;
    println!("User keys: {:?}", user_keys);
    
    // Count total items / 计算总项目数
    let count = store.count().await?;
    println!("Total items: {}", count);
    
    Ok(())
}

/// Example 2: Working with Serialized Data
/// 示例2：处理序列化数据
async fn example_serialized_data() -> Result<(), Box<dyn std::error::Error>> {
    use spear_next::storage::{MemoryKvStore, KvStore, serialization};
    use spear_next::sms::services::node_service::NodeInfo;
    
    println!("=== Serialized Data Example ===");
    println!("=== 序列化数据示例 ===");
    
    let store = MemoryKvStore::new();
    
    // Create some nodes / 创建一些节点
    let node1 = NodeInfo {
        uuid: uuid::Uuid::new_v4().to_string(),
        name: "node-1".to_string(),
        address: "192.168.1.10".to_string(),
        port: 8080,
        capabilities: vec!["storage".to_string(), "primary".to_string()],
    };
    let node2 = NodeInfo {
        uuid: uuid::Uuid::new_v4().to_string(),
        name: "node-2".to_string(),
        address: "192.168.1.11".to_string(),
        port: 8081,
        capabilities: vec!["storage".to_string(), "secondary".to_string()],
    };
    
    // Serialize and store nodes / 序列化并存储节点
    let uuid1 = uuid::Uuid::parse_str(&node1.uuid).unwrap();
    let uuid2 = uuid::Uuid::parse_str(&node2.uuid).unwrap();
    let key1 = serialization::node_key(&uuid1);
    let key2 = serialization::node_key(&uuid2);
    
    let value1 = serialization::serialize(&node1)?;
    let value2 = serialization::serialize(&node2)?;
    
    store.put(&key1, &value1).await?;
    store.put(&key2, &value2).await?;
    
    // Retrieve and deserialize / 检索并反序列化
    if let Some(data) = store.get(&key1).await? {
        let retrieved_node: NodeInfo = serialization::deserialize(&data)?;
        println!("Retrieved node: {} at {}:{}", 
                retrieved_node.uuid, 
                retrieved_node.address, 
                retrieved_node.port);
    }
    
    // Get all node keys / 获取所有节点键
    let node_prefix = serialization::node_prefix();
    let all_node_keys = store.keys_with_prefix(node_prefix).await?;
    
    println!("Found {} nodes in storage", all_node_keys.len());
    
    // Retrieve all nodes / 检索所有节点
    for key in all_node_keys {
        if let Some(data) = store.get(&key).await? {
            let node: NodeInfo = serialization::deserialize(&data)?;
            println!("Node {}: {}:{} (capabilities: {:?})", 
                    node.uuid, 
                    node.address, 
                    node.port,
                    node.capabilities);
        }
    }
    
    Ok(())
}

/// Example 3: Range Queries and Filtering
/// 示例3：范围查询和过滤
async fn example_range_queries() -> Result<(), Box<dyn std::error::Error>> {
    use spear_next::storage::{MemoryKvStore, KvStore};
    
    println!("=== Range Queries Example ===");
    println!("=== 范围查询示例 ===");
    
    let store = MemoryKvStore::new();
    
    // Insert test data with different prefixes / 插入具有不同前缀的测试数据
    let test_data = vec![
        ("node:001", "Node 1 data"),
        ("node:002", "Node 2 data"),
        ("node:003", "Node 3 data"),
        ("resource:001", "Resource 1 data"),
        ("resource:002", "Resource 2 data"),
        ("config:timeout", "30"),
        ("config:retries", "3"),
        ("stats:cpu", "75.5"),
        ("stats:memory", "82.1"),
    ];
    
    for (key, value) in test_data {
        store.put(&key.to_string(), &value.as_bytes().to_vec()).await?;
    }
    
    // Query 1: Get all nodes / 查询1：获取所有节点
    println!("\n--- All Nodes ---");
    let node_keys = store.keys_with_prefix("node:").await?;
    for key in node_keys {
        if let Some(value) = store.get(&key).await? {
            println!("{}: {}", key, String::from_utf8(value)?);
        }
    }
    
    // Query 2: Range query with limit / 查询2：带限制的范围查询
    println!("\n--- First 2 Config Items ---");
    let config_options = RangeOptions::new()
        .start_key("config:")
        .end_key("config:~")  // Use ~ to get all config keys
        .limit(2);
    
    let config_results = store.range(&config_options).await?;
    for pair in config_results {
        println!("{}: {}", pair.key, String::from_utf8(pair.value)?);
    }
    
    // Query 3: Reverse order query / 查询3：逆序查询
    println!("\n--- Stats in Reverse Order ---");
    let stats_options = RangeOptions::new()
        .start_key("stats:")
        .end_key("stats:~")
        .reverse(true);
    
    let stats_results = store.range(&stats_options).await?;
    for pair in stats_results {
        println!("{}: {}", pair.key, String::from_utf8(pair.value)?);
    }
    
    Ok(())
}

/// Example 4: Batch Operations
/// 示例4：批量操作
async fn example_batch_operations() -> Result<(), Box<dyn std::error::Error>> {
    use spear_next::storage::{MemoryKvStore, KvStore};
    
    println!("=== Batch Operations Example ===");
    println!("=== 批量操作示例 ===");
    
    let store = MemoryKvStore::new();
    
    // Prepare batch data / 准备批量数据
    let batch_data = vec![
        KvPair {
            key: "batch:001".to_string(),
            value: b"Batch item 1".to_vec(),
        },
        KvPair {
            key: "batch:002".to_string(),
            value: b"Batch item 2".to_vec(),
        },
        KvPair {
            key: "batch:003".to_string(),
            value: b"Batch item 3".to_vec(),
        },
        KvPair {
            key: "batch:004".to_string(),
            value: b"Batch item 4".to_vec(),
        },
        KvPair {
            key: "batch:005".to_string(),
            value: b"Batch item 5".to_vec(),
        },
    ];
    
    // Batch insert / 批量插入
    println!("Inserting {} items in batch...", batch_data.len());
    store.batch_put(&batch_data).await?;
    
    let count_after_insert = store.count().await?;
    println!("Total items after batch insert: {}", count_after_insert);
    
    // Verify data / 验证数据
    let batch_keys = store.keys_with_prefix("batch:").await?;
    println!("Found {} batch items", batch_keys.len());
    
    // Batch delete some items / 批量删除一些项目
    let keys_to_delete = vec![
        "batch:002".to_string(),
        "batch:004".to_string(),
    ];
    
    println!("Deleting {} items in batch...", keys_to_delete.len());
    let deleted_count = store.batch_delete(&keys_to_delete).await?;
    println!("Successfully deleted {} items", deleted_count);
    
    let count_after_delete = store.count().await?;
    println!("Total items after batch delete: {}", count_after_delete);
    
    // Show remaining items / 显示剩余项目
    println!("\nRemaining batch items:");
    let remaining_keys = store.keys_with_prefix("batch:").await?;
    for key in remaining_keys {
        if let Some(value) = store.get(&key).await? {
            println!("{}: {}", key, String::from_utf8(value)?);
        }
    }
    
    Ok(())
}

/// Example 5: Using with KvNodeRegistry
/// 示例5：与KvNodeRegistry一起使用
async fn example_kv_node_registry() -> Result<(), Box<dyn std::error::Error>> {
    use spear_next::storage::{MemoryKvStore, KvStore};
    use spear_next::sms::services::node_service::NodeService;
    
    println!("=== KvNodeRegistry Example ===");
    println!("=== KvNodeRegistry示例 ===");
    
    // Create KV store and registry / 创建KV存储和注册表
    let _kv_store: Box<dyn KvStore> = Box::new(MemoryKvStore::new());
    let mut registry = NodeService::new();
    
    // Create some nodes using proto Node structure / 使用proto Node结构创建一些节点
    use spear_next::proto::sms::Node;
    use std::collections::HashMap;
    
    let node1 = Node {
        uuid: uuid::Uuid::new_v4().to_string(),
        ip_address: "10.0.1.10".to_string(),
        port: 8080,
        status: "online".to_string(),
        last_heartbeat: chrono::Utc::now().timestamp(),
        registered_at: chrono::Utc::now().timestamp(),
        metadata: HashMap::new(),
    };
    let node2 = Node {
        uuid: uuid::Uuid::new_v4().to_string(),
        ip_address: "10.0.1.11".to_string(),
        port: 8081,
        status: "online".to_string(),
        last_heartbeat: chrono::Utc::now().timestamp(),
        registered_at: chrono::Utc::now().timestamp(),
        metadata: HashMap::new(),
    };
    let node3 = Node {
        uuid: uuid::Uuid::new_v4().to_string(),
        ip_address: "10.0.1.12".to_string(),
        port: 8082,
        status: "online".to_string(),
        last_heartbeat: chrono::Utc::now().timestamp(),
        registered_at: chrono::Utc::now().timestamp(),
        metadata: HashMap::new(),
    };
    
    // Register nodes / 注册节点
    registry.register_node(node1.clone()).await?;
    registry.register_node(node2.clone()).await?;
    registry.register_node(node3.clone()).await?;
    
    println!("Registered nodes: {}, {}, {}", node1.uuid, node2.uuid, node3.uuid);
    
    // Update heartbeat / 更新心跳
    registry.update_heartbeat(&node1.uuid, chrono::Utc::now().timestamp()).await?;
    println!("Updated heartbeat for node {}", node1.uuid);
    
    // List all nodes / 列出所有节点
    let all_nodes = registry.list_nodes().await?;
    println!("Total nodes: {}", all_nodes.len());
    
    // Get node count / 获取节点数量
    let count = registry.node_count().await?;
    println!("Node count: {}", count);
    
    // Remove a node / 移除节点
    registry.remove_node(&node3.uuid).await?;
    println!("Node {} removed successfully", node3.uuid);
    
    Ok(())
}

/// Example 6: Persistent Storage with Sled
/// 示例6：使用Sled的持久化存储
#[cfg(feature = "sled")]
async fn example_sled_persistence() -> Result<(), Box<dyn std::error::Error>> {
    use spear_next::storage::{SledKvStore, KvStore};
    use tempfile::TempDir;
    
    println!("=== Sled Persistence Example ===");
    println!("=== Sled持久化示例 ===");
    
    // Create temporary directory for this example / 为此示例创建临时目录
    let temp_dir = TempDir::new()?;
    let db_path = temp_dir.path().join("example_db");
    
    // Method 1: Create Sled store directly / 方法1：直接创建Sled存储
    {
        let store = SledKvStore::new(&db_path)?;
        
        // Store some data / 存储一些数据
        store.put(&"persistent:key1".to_string(), &b"This data will persist".to_vec()).await?;
        store.put(&"persistent:key2".to_string(), &b"Even after restart".to_vec()).await?;
        
        println!("Stored data in Sled database");
        
        let count = store.count().await?;
        println!("Items in database: {}", count);
    } // Store is dropped here / 存储在这里被丢弃
    
    // Method 2: Use factory function / 方法2：使用工厂函数
    {
        let store = create_kv_store(KvStoreType::Sled { 
            path: db_path.to_string_lossy().to_string() 
        })?;
        
        // Data should still be there / 数据应该仍然存在
        let count = store.count().await?;
        println!("Items after reopening database: {}", count);
        
        // Retrieve the data / 检索数据
        if let Some(value) = store.get(&"persistent:key1".to_string()).await? {
            println!("Retrieved: {}", String::from_utf8(value)?);
        }
        
        // Add more data / 添加更多数据
        store.put(&"persistent:key3".to_string(), &b"Added after reopen".to_vec()).await?;
        
        let final_count = store.count().await?;
        println!("Final count: {}", final_count);
        
        // List all persistent keys / 列出所有持久化键
        let persistent_keys = store.keys_with_prefix("persistent:").await?;
        println!("Persistent keys: {:?}", persistent_keys);
    }
    
    Ok(())
}

/// Example 7: Error Handling Patterns
/// 示例7：错误处理模式
async fn example_error_handling() -> Result<(), Box<dyn std::error::Error>> {
    use spear_next::storage::{MemoryKvStore, KvStore, serialization};
    use spear_next::sms::error::SmsError;
    use spear_next::sms::services::node_service::NodeInfo;
    
    println!("=== Error Handling Example ===");
    println!("=== 错误处理示例 ===");
    
    let store = MemoryKvStore::new();
    
    // Example 1: Handling missing keys / 示例1：处理缺失的键
    match store.get(&"nonexistent:key".to_string()).await {
        Ok(Some(value)) => println!("Found value: {:?}", value),
        Ok(None) => println!("Key not found (this is normal)"),
        Err(e) => eprintln!("Error getting key: {:?}", e),
    }
    
    // Example 2: Handling serialization errors / 示例2：处理序列化错误
    let invalid_data = b"invalid json data";
    match serialization::deserialize::<NodeInfo>(invalid_data) {
        Ok(node) => println!("Deserialized node: {:?}", node),
        Err(SmsError::Serialization(message)) => {
            println!("Expected serialization error: {}", message);
        },
        Err(e) => eprintln!("Unexpected error: {:?}", e),
    }
    
    // Example 3: Graceful error recovery / 示例3：优雅的错误恢复
    let keys_to_try = vec!["key1", "key2", "key3"];
    let mut successful_retrievals = 0;
    
    for key in &keys_to_try {
        match store.get(&key.to_string()).await {
            Ok(Some(_)) => {
                successful_retrievals += 1;
                println!("Successfully retrieved {}", key);
            },
            Ok(None) => {
                println!("Key {} not found, skipping", key);
            },
            Err(e) => {
                eprintln!("Error retrieving {}: {:?}", key, e);
                // Continue with other keys / 继续处理其他键
            }
        }
    }
    
    println!("Successfully retrieved {} out of {} keys", 
             successful_retrievals, keys_to_try.len());
    
    Ok(())
}

// Helper function to demonstrate custom error handling
// 辅助函数演示自定义错误处理
async fn safe_get_and_deserialize<T>(
    store: &dyn KvStore, 
    key: &str
) -> Result<Option<T>, SmsError> 
where 
    T: for<'de> serde::Deserialize<'de>
{
    use spear_next::storage::serialization;
    
    match store.get(&key.to_string()).await? {
        Some(data) => {
            let deserialized = serialization::deserialize(&data)?;
            Ok(Some(deserialized))
        },
        None => Ok(None),
    }
}

/// Main function to run all examples
/// 运行所有示例的主函数
#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    println!("KV Abstraction Layer Examples");
    println!("KV抽象层示例");
    println!("=============================\n");
    
    // Run all examples / 运行所有示例
    example_basic_operations().await?;
    println!("\n");
    
    example_serialized_data().await?;
    println!("\n");
    
    example_range_queries().await?;
    println!("\n");
    
    example_batch_operations().await?;
    println!("\n");
    
    example_kv_node_registry().await?;
    println!("\n");
    
    #[cfg(feature = "sled")]
    {
        example_sled_persistence().await?;
        println!("\n");
    }
    
    example_error_handling().await?;
    
    println!("\nAll examples completed successfully!");
    println!("所有示例成功完成！");
    
    Ok(())
}