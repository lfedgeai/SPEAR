//! KV Storage Integration Tests
//! KV存储集成测试
//!
//! This module contains comprehensive integration tests for the KV storage layer,
//! including performance tests, error handling, concurrent operations, and cross-backend compatibility.
//! 此模块包含KV存储层的综合集成测试，包括性能测试、错误处理、并发操作和跨后端兼容性。

use std::collections::HashMap;
use std::sync::Arc;
use std::time::{Duration, Instant};
use tokio::sync::Barrier;
use tokio::time::timeout;
use uuid::Uuid;

use spear_next::storage::{KvStoreConfig, DefaultKvStoreFactory, KvStoreFactory, RangeOptions};


/// Test utilities for KV storage integration tests
/// KV存储集成测试的测试工具
mod test_utils {
    use super::*;
    use tempfile::TempDir;

    /// Create test configurations for all supported backends
    /// 为所有支持的后端创建测试配置
    pub fn create_test_configs() -> Vec<(&'static str, KvStoreConfig, Option<TempDir>)> {
        let configs = vec![
            ("memory", KvStoreConfig::memory(), None),
        ];

        // Add Sled configuration with temporary directory
        // 添加带临时目录的Sled配置
        #[cfg(feature = "sled")]
        {
            let temp_dir = TempDir::new().unwrap();
            let sled_config = KvStoreConfig::sled(temp_dir.path().to_str().unwrap());
            configs.push(("sled", sled_config, Some(temp_dir)));
        }

        // Add RocksDB configuration with temporary directory
        // 添加带临时目录的RocksDB配置
        #[cfg(feature = "rocksdb")]
        {
            let temp_dir = TempDir::new().unwrap();
            let rocksdb_config = KvStoreConfig::rocksdb(temp_dir.path().to_str().unwrap());
            configs.push(("rocksdb", rocksdb_config, Some(temp_dir)));
        }

        configs
    }

    /// Generate test data for performance tests
    /// 为性能测试生成测试数据
    pub fn generate_test_data(count: usize) -> Vec<(String, String)> {
        (0..count)
            .map(|i| (format!("key_{:06}", i), format!("value_{:06}_{}", i, Uuid::new_v4())))
            .collect()
    }

    /// Generate large test data for stress tests
    /// 为压力测试生成大量测试数据
    pub fn generate_large_test_data(count: usize, value_size: usize) -> Vec<(String, String)> {
        let large_value = "x".repeat(value_size);
        (0..count)
            .map(|i| (format!("large_key_{:06}", i), format!("{}_{}_{}", large_value, i, Uuid::new_v4())))
            .collect()
    }
}

/// Test cross-backend compatibility
/// 测试跨后端兼容性
#[tokio::test]
async fn test_cross_backend_compatibility() {
    let factory = DefaultKvStoreFactory::new();
    let configs = test_utils::create_test_configs();

    for (backend_name, config, _temp_dir) in configs {
        println!("Testing backend: {}", backend_name);
        
        let store = factory.create(&config).await.unwrap();
        
        // Test basic operations
        // 测试基本操作
        let key = "compatibility_test_key";
        let value = "compatibility_test_value";
        
        store.put(&key.to_string(), &value.as_bytes().to_vec()).await.unwrap();
        let retrieved = store.get(&key.to_string()).await.unwrap();
        assert_eq!(retrieved, Some(value.as_bytes().to_vec()));
        
        let exists = store.exists(&key.to_string()).await.unwrap();
        assert!(exists);
        
        store.delete(&key.to_string()).await.unwrap();
        let after_delete = store.get(&key.to_string()).await.unwrap();
        assert_eq!(after_delete, None);
        
        println!("Backend {} passed compatibility test", backend_name);
    }
}

/// Test performance comparison across backends
/// 测试跨后端性能比较
#[tokio::test]
async fn test_performance_comparison() {
    let factory = DefaultKvStoreFactory::new();
    let configs = test_utils::create_test_configs();
    let test_data = test_utils::generate_test_data(1000);
    
    let mut results = HashMap::new();
    
    for (backend_name, config, _temp_dir) in configs {
        println!("Performance testing backend: {}", backend_name);
        
        let store = factory.create(&config).await.unwrap();
        
        // Test write performance
        // 测试写入性能
        let start = Instant::now();
        for (key, value) in &test_data {
            store.put(key, &value.as_bytes().to_vec()).await.unwrap();
        }
        let write_duration = start.elapsed();
        
        // Test read performance
        // 测试读取性能
        let start = Instant::now();
        for (key, _) in &test_data {
            let _ = store.get(key).await.unwrap();
        }
        let read_duration = start.elapsed();
        
        // Test scan performance
        // 测试扫描性能
        let start = Instant::now();
        let keys = store.keys_with_prefix("key_").await.unwrap();
        let scan_duration = start.elapsed();
        
        results.insert(backend_name, (write_duration, read_duration, scan_duration, keys.len()));
        
        println!(
            "Backend {}: Write: {:?}, Read: {:?}, Scan: {:?}, Keys: {}",
            backend_name, write_duration, read_duration, scan_duration, keys.len()
        );
    }
    
    // Verify all backends returned the same number of keys
    // 验证所有后端返回相同数量的键
    let key_counts: Vec<usize> = results.values().map(|(_, _, _, count)| *count).collect();
    assert!(key_counts.iter().all(|&count| count == test_data.len()));
}

/// Test concurrent operations
/// 测试并发操作
#[tokio::test]
async fn test_concurrent_operations() {
    let factory = DefaultKvStoreFactory::new();
    let configs = test_utils::create_test_configs();
    
    for (backend_name, config, _temp_dir) in configs {
        println!("Testing concurrent operations for backend: {}", backend_name);
        
        let store = Arc::new(factory.create(&config).await.unwrap());
        let num_tasks = 10;
        let operations_per_task = 100;
        let barrier = Arc::new(Barrier::new(num_tasks));
        
        let mut handles = vec![];
        
        for task_id in 0..num_tasks {
            let store_clone = Arc::clone(&store);
            let barrier_clone = Arc::clone(&barrier);
            
            let handle = tokio::spawn(async move {
                // Wait for all tasks to be ready
                // 等待所有任务准备就绪
                barrier_clone.wait().await;
                
                for op_id in 0..operations_per_task {
                    let key = format!("concurrent_{}_{}", task_id, op_id);
                    let value = format!("value_{}_{}", task_id, op_id);
                    
                    // Perform set operation
                    // 执行设置操作
                    store_clone.put(&key, &value.as_bytes().to_vec()).await.unwrap();
                    
                    // Verify get operation
                    // 验证获取操作
                    let retrieved = store_clone.get(&key).await.unwrap();
                    assert_eq!(retrieved, Some(value.as_bytes().to_vec()));
                    
                    // Test exists operation
                    // 测试存在性操作
                    let exists = store_clone.exists(&key).await.unwrap();
                    assert!(exists);
                }
            });
            
            handles.push(handle);
        }
        
        // Wait for all tasks to complete
        // 等待所有任务完成
        for handle in handles {
            handle.await.unwrap();
        }
        
        // Verify total number of keys
        // 验证键的总数
        let all_keys = store.keys_with_prefix("concurrent_").await.unwrap();
        assert_eq!(all_keys.len(), num_tasks * operations_per_task);
        
        println!("Backend {} passed concurrent operations test", backend_name);
    }
}

/// Test error handling and edge cases
/// 测试错误处理和边界情况
#[tokio::test]
async fn test_error_handling_and_edge_cases() {
    let factory = DefaultKvStoreFactory::new();
    let configs = test_utils::create_test_configs();
    
    for (backend_name, config, _temp_dir) in configs {
        println!("Testing error handling for backend: {}", backend_name);
        
        let store = factory.create(&config).await.unwrap();
        
        // Test empty key handling
        // 测试空键处理
        let result = store.put(&"".to_string(), &"value".as_bytes().to_vec()).await;
        // Should succeed for most backends
        // 对大多数后端应该成功
        assert!(result.is_ok());
        
        // Test empty value handling
        // 测试空值处理
        let result = store.put(&"key".to_string(), &"".as_bytes().to_vec()).await;
        assert!(result.is_ok());
        
        let retrieved = store.get(&"key".to_string()).await.unwrap();
        assert_eq!(retrieved, Some("".as_bytes().to_vec()));
        
        // Test very long key
        // 测试很长的键
        let long_key = "x".repeat(1000);
        let result = store.put(&long_key, &"value".as_bytes().to_vec()).await;
        assert!(result.is_ok());
        
        // Test very long value
        // 测试很长的值
        let long_value = "y".repeat(10000);
        let result = store.put(&"long_value_key".to_string(), &long_value.as_bytes().to_vec()).await;
        assert!(result.is_ok());
        
        let retrieved = store.get(&"long_value_key".to_string()).await.unwrap();
        assert_eq!(retrieved, Some(long_value.as_bytes().to_vec()));
        
        // Test special characters in keys and values
        // 测试键和值中的特殊字符
        let special_key = "key/with\\special:chars@#$%^&*()";
        let special_value = "value\nwith\ttabs\rand\0nulls";
        let result = store.put(&special_key.to_string(), &special_value.as_bytes().to_vec()).await;
        assert!(result.is_ok());
        
        let retrieved = store.get(&special_key.to_string()).await.unwrap();
        assert_eq!(retrieved, Some(special_value.as_bytes().to_vec()));
        
        // Test Unicode characters
        // 测试Unicode字符
        let unicode_key = "键_🔑_key";
        let unicode_value = "值_💎_value_中文";
        let result = store.put(&unicode_key.to_string(), &unicode_value.as_bytes().to_vec()).await;
        assert!(result.is_ok());
        
        let retrieved = store.get(&unicode_key.to_string()).await.unwrap();
        assert_eq!(retrieved, Some(unicode_value.as_bytes().to_vec()));
        
        println!("Backend {} passed error handling test", backend_name);
    }
}

/// Test large data handling and stress scenarios
/// 测试大数据处理和压力场景
#[tokio::test]
async fn test_large_data_handling() {
    let factory = DefaultKvStoreFactory::new();
    let configs = test_utils::create_test_configs();
    
    for (backend_name, config, _temp_dir) in configs {
        println!("Testing large data handling for backend: {}", backend_name);
        
        let store = factory.create(&config).await.unwrap();
        
        // Test with large values (1MB each)
        // 测试大值（每个1MB）
        let large_data = test_utils::generate_large_test_data(10, 1024 * 1024);
        
        for (key, value) in &large_data {
            let result = timeout(Duration::from_secs(30), store.put(key, &value.as_bytes().to_vec())).await;
            assert!(result.is_ok(), "Timeout or error setting large value for backend {}", backend_name);
            assert!(result.unwrap().is_ok());
        }
        
        // Verify large values can be retrieved
        // 验证可以检索大值
        for (key, expected_value) in &large_data {
            let result = timeout(Duration::from_secs(30), store.get(key)).await;
            assert!(result.is_ok(), "Timeout getting large value for backend {}", backend_name);
            
            let retrieved = result.unwrap().unwrap();
            assert_eq!(retrieved, Some(expected_value.as_bytes().to_vec()));
        }
        
        // Test batch operations with large data
        // 测试大数据的批量操作
        let _keys: Vec<String> = large_data.iter().map(|(k, _)| k.clone()).collect();
        let scan_result = timeout(Duration::from_secs(30), store.keys_with_prefix("large_key_")).await;
        assert!(scan_result.is_ok(), "Timeout scanning large data for backend {}", backend_name);
        
        let scanned_keys = scan_result.unwrap().unwrap();
        assert_eq!(scanned_keys.len(), large_data.len());
        
        println!("Backend {} passed large data handling test", backend_name);
    }
}

/// Test range operations across different backends
/// 测试跨不同后端的范围操作
#[tokio::test]
async fn test_range_operations_comprehensive() {
    let factory = DefaultKvStoreFactory::new();
    let configs = test_utils::create_test_configs();
    
    for (backend_name, config, _temp_dir) in configs {
        println!("Testing range operations for backend: {}", backend_name);
        
        let store = factory.create(&config).await.unwrap();
        
        // Setup test data with predictable ordering
        // 设置具有可预测排序的测试数据
        let test_data = vec![
            ("a001", "value_a001"),
            ("a002", "value_a002"),
            ("a010", "value_a010"),
            ("b001", "value_b001"),
            ("b002", "value_b002"),
            ("c001", "value_c001"),
        ];
        
        for (key, value) in &test_data {
            store.put(&key.to_string(), &value.as_bytes().to_vec()).await.unwrap();
        }
        
        // Test prefix scanning
        // 测试前缀扫描
        let a_keys = store.keys_with_prefix("a").await.unwrap();
        assert_eq!(a_keys.len(), 3);
        assert!(a_keys.contains(&"a001".to_string()));
        assert!(a_keys.contains(&"a002".to_string()));
        assert!(a_keys.contains(&"a010".to_string()));
        
        let b_keys = store.keys_with_prefix("b").await.unwrap();
        assert_eq!(b_keys.len(), 2);
        
        // Test scan_prefix with key-value pairs
        // 测试带键值对的scan_prefix
        let a_pairs = store.scan_prefix("a").await.unwrap();
        assert_eq!(a_pairs.len(), 3);
        
        for pair in &a_pairs {
            assert!(pair.key.starts_with("a"));
            let value_str = String::from_utf8(pair.value.clone()).unwrap();
            assert!(value_str.starts_with("value_a"));
        }
        
        // Test range operations
        // 测试范围操作
        let range_options = RangeOptions::new()
            .start_key("a001")
            .end_key("b001");
        let range_pairs = store.range(&range_options).await.unwrap();
        // Should include a001, a002, a010 but not b001 (exclusive end)
        // 应该包括a001, a002, a010但不包括b001（排他性结束）
        assert!(range_pairs.len() >= 3);
        
        println!("Backend {} passed range operations test", backend_name);
    }
}

/// Test factory configuration and validation
/// 测试工厂配置和验证
#[tokio::test]
async fn test_factory_configuration_validation() {
    let factory = DefaultKvStoreFactory::new();
    
    // Test memory configuration
    // 测试内存配置
    let memory_config = KvStoreConfig::memory();
    let memory_store = factory.create(&memory_config).await;
    assert!(memory_store.is_ok());
    
    // Test invalid path configurations (should still work but may create directories)
    // 测试无效路径配置（应该仍然工作但可能创建目录）
    #[cfg(feature = "sled")]
    {
        let sled_config = KvStoreConfig::sled("/tmp/test_sled_factory");
        let sled_store = factory.create(&sled_config).await;
        assert!(sled_store.is_ok());
    }
    
    #[cfg(feature = "rocksdb")]
    {
        let rocksdb_config = KvStoreConfig::rocksdb("/tmp/test_rocksdb_factory");
        let rocksdb_store = factory.create(&rocksdb_config).await;
        assert!(rocksdb_store.is_ok());
    }
}

/// Test cleanup and resource management
/// 测试清理和资源管理
#[tokio::test]
async fn test_cleanup_and_resource_management() {
    let factory = DefaultKvStoreFactory::new();
    let configs = test_utils::create_test_configs();
    
    for (backend_name, config, _temp_dir) in configs {
        println!("Testing cleanup for backend: {}", backend_name);
        
        let store = factory.create(&config).await.unwrap();
        
        // Add some test data
        // 添加一些测试数据
        for i in 0..100 {
            let key = format!("cleanup_test_{}", i);
            let value = format!("value_{}", i);
            store.put(&key, &value.as_bytes().to_vec()).await.unwrap();
        }
        
        // Verify data exists
        // 验证数据存在
        let keys = store.keys_with_prefix("cleanup_test_").await.unwrap();
        assert_eq!(keys.len(), 100);
        
        // Delete all test data
        // 删除所有测试数据
        for key in &keys {
            store.delete(key).await.unwrap();
        }
        
        // Verify data is deleted
        // 验证数据已删除
        let remaining_keys = store.keys_with_prefix("cleanup_test_").await.unwrap();
        assert_eq!(remaining_keys.len(), 0);
        
        println!("Backend {} passed cleanup test", backend_name);
    }
}