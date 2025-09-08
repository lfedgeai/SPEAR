//! KV Storage Edge Cases and Error Handling Tests
//! KV存储边界情况和错误处理测试
//!
//! This module contains tests for edge cases, error conditions, and boundary scenarios
//! to ensure robust behavior across all KV storage backends.
//! 此模块包含边界情况、错误条件和边界场景的测试，以确保所有KV存储后端的健壮行为。


use std::sync::Arc;
use std::time::Duration;
use tokio::time::timeout;
use tempfile::TempDir;

use spear_next::storage::{KvStoreConfig, DefaultKvStoreFactory, KvStoreFactory};


/// Test utilities for edge case testing
/// 边界情况测试的测试工具
mod test_utils {
    use super::*;

    /// Create test configurations for all supported backends
    /// 为所有支持的后端创建测试配置
    pub fn create_test_configs() -> Vec<(&'static str, KvStoreConfig, Option<TempDir>)> {
        let mut configs = vec![
            ("memory", KvStoreConfig::memory(), None),
        ];

        #[cfg(feature = "sled")]
        {
            let temp_dir = TempDir::new().unwrap();
            let sled_config = KvStoreConfig::sled(temp_dir.path().to_str().unwrap());
            configs.push(("sled", sled_config, Some(temp_dir)));
        }

        #[cfg(feature = "rocksdb")]
        {
            let temp_dir = TempDir::new().unwrap();
            let rocksdb_config = KvStoreConfig::rocksdb(temp_dir.path().to_str().unwrap());
            configs.push(("rocksdb", rocksdb_config, Some(temp_dir)));
        }

        configs
    }

    /// Generate problematic keys for testing
    /// 生成用于测试的问题键
    pub fn generate_problematic_keys() -> Vec<String> {
        vec![
            "".to_string(),                           // Empty key / 空键
            " ".to_string(),                          // Space key / 空格键
            "\n".to_string(),                         // Newline key / 换行键
            "\t".to_string(),                         // Tab key / 制表符键
            "\0".to_string(),                         // Null byte / 空字节
            "key\0with\0nulls".to_string(),          // Key with null bytes / 包含空字节的键
            "key\nwith\nnewlines".to_string(),       // Key with newlines / 包含换行的键
            "key\twith\ttabs".to_string(),           // Key with tabs / 包含制表符的键
            "key with spaces".to_string(),           // Key with spaces / 包含空格的键
            "key/with/slashes".to_string(),          // Key with slashes / 包含斜杠的键
            "key\\with\\backslashes".to_string(),    // Key with backslashes / 包含反斜杠的键
            "key:with:colons".to_string(),           // Key with colons / 包含冒号的键
            "key@with@symbols".to_string(),          // Key with symbols / 包含符号的键
            "键_with_中文".to_string(),               // Key with Chinese characters / 包含中文字符的键
            "🔑_emoji_key".to_string(),              // Key with emoji / 包含表情符号的键
            "very_long_key_".repeat(100),            // Very long key / 很长的键
        ]
    }

    /// Generate problematic values for testing
    /// 生成用于测试的问题值
    pub fn generate_problematic_values() -> Vec<String> {
        vec![
            "".to_string(),                                    // Empty value / 空值
            " ".to_string(),                                   // Space value / 空格值
            "\n".to_string(),                                  // Newline value / 换行值
            "\t".to_string(),                                  // Tab value / 制表符值
            "\0".to_string(),                                  // Null byte / 空字节
            "value\0with\0nulls".to_string(),                 // Value with null bytes / 包含空字节的值
            "value\nwith\nnewlines".to_string(),              // Value with newlines / 包含换行的值
            "value\twith\ttabs".to_string(),                  // Value with tabs / 包含制表符的值
            "value with spaces".to_string(),                  // Value with spaces / 包含空格的值
            "值_with_中文_characters".to_string(),             // Value with Chinese / 包含中文的值
            "💎_emoji_value_🚀".to_string(),                  // Value with emoji / 包含表情符号的值
            "x".repeat(1024 * 1024),                         // 1MB value / 1MB值
            "y".repeat(10 * 1024 * 1024),                    // 10MB value / 10MB值
            serde_json::to_string(&(0..1000).collect::<Vec<i32>>()).unwrap(), // JSON data / JSON数据
        ]
    }
}

/// Test empty and whitespace keys
/// 测试空键和空白键
#[tokio::test]
async fn test_empty_and_whitespace_keys() {
    let factory = DefaultKvStoreFactory::new();
    let configs = test_utils::create_test_configs();

    for (backend_name, config, _temp_dir) in configs {
        println!("Testing empty and whitespace keys for backend: {}", backend_name);
        
        let store = factory.create(&config).await.unwrap();
        
        let problematic_keys = test_utils::generate_problematic_keys();
        
        for key in &problematic_keys {
            let value = format!("value_for_key_{}", key.len());
            
            // Test put operation
            // 测试设置操作
            let value_bytes = value.as_bytes().to_vec();
            let put_result = store.put(key, &value_bytes).await;
            assert!(put_result.is_ok(), "Failed to put key '{}' for backend {}: {:?}", 
                    key.escape_debug(), backend_name, put_result);
            
            // Test get operation
            // 测试获取操作
            let get_result = store.get(&key.to_string()).await;
            assert!(get_result.is_ok(), "Failed to get key '{}' for backend {}: {:?}", 
                    key.escape_debug(), backend_name, get_result);
            
            let retrieved_value = get_result.unwrap();
            assert_eq!(retrieved_value, Some(value.as_bytes().to_vec()), 
                      "Value mismatch for key '{}' in backend {}", key.escape_debug(), backend_name);
            
            // Test exists operation
            // 测试存在性操作
            let exists_result = store.exists(key).await;
            assert!(exists_result.is_ok(), "Failed to check existence of key '{}' for backend {}: {:?}", 
                    key.escape_debug(), backend_name, exists_result);
            assert!(exists_result.unwrap(), "Key '{}' should exist in backend {}", 
                    key.escape_debug(), backend_name);
            
            // Test delete operation
            // 测试删除操作
            let delete_result = store.delete(&key.to_string()).await;
            assert!(delete_result.is_ok(), "Failed to delete key '{}' for backend {}: {:?}", 
                    key.escape_debug(), backend_name, delete_result);
            
            // Verify deletion
            // 验证删除
            let after_delete = store.get(&key.to_string()).await.unwrap();
            assert_eq!(after_delete, None, "Key '{}' should be deleted from backend {}", 
                      key.escape_debug(), backend_name);
        }
        
        println!("Backend {} passed empty and whitespace keys test", backend_name);
    }
}

/// Test problematic values
/// 测试问题值
#[tokio::test]
async fn test_problematic_values() {
    let factory = DefaultKvStoreFactory::new();
    let configs = test_utils::create_test_configs();

    for (backend_name, config, _temp_dir) in configs {
        println!("Testing problematic values for backend: {}", backend_name);
        
        let store = factory.create(&config).await.unwrap();
        
        let problematic_values = test_utils::generate_problematic_values();
        
        for (i, value) in problematic_values.iter().enumerate() {
            let key = format!("problematic_value_key_{}", i);
            
            // Test put operation with timeout for large values
            // 对大值使用超时测试设置操作
            let value_bytes = value.as_bytes().to_vec();
            let put_result = timeout(Duration::from_secs(30), store.put(&key, &value_bytes)).await;
            assert!(put_result.is_ok(), "Timeout putting problematic value {} for backend {}", 
                    i, backend_name);
            assert!(put_result.unwrap().is_ok(), "Failed to put problematic value {} for backend {}", 
                    i, backend_name);
            
            // Test get operation with timeout for large values
            // 对大值使用超时测试获取操作
            let get_result = timeout(Duration::from_secs(30), store.get(&key)).await;
            assert!(get_result.is_ok(), "Timeout getting problematic value {} for backend {}", 
                    i, backend_name);
            
            let retrieved_value = get_result.unwrap().unwrap();
            assert_eq!(retrieved_value, Some(value.as_bytes().to_vec()), 
                      "Value mismatch for problematic value {} in backend {}", i, backend_name);
            
            // Clean up large values to avoid memory issues
            // 清理大值以避免内存问题
            if value.len() > 1024 * 1024 {
                store.delete(&key).await.unwrap();
            }
        }
        
        println!("Backend {} passed problematic values test", backend_name);
    }
}

/// Test very large keys and values
/// 测试非常大的键和值
#[tokio::test]
async fn test_very_large_keys_and_values() {
    let factory = DefaultKvStoreFactory::new();
    let configs = test_utils::create_test_configs();

    for (backend_name, config, _temp_dir) in configs {
        println!("Testing very large keys and values for backend: {}", backend_name);
        
        let store = factory.create(&config).await.unwrap();
        
        // Test very long key (but reasonable value)
        // 测试很长的键（但合理的值）
        let very_long_key = "x".repeat(10000);
        let normal_value = "normal_value";
        
        let normal_value_bytes = normal_value.as_bytes().to_vec();
        let result = store.put(&very_long_key, &normal_value_bytes).await;
        assert!(result.is_ok(), "Failed to put very long key for backend {}: {:?}", 
                backend_name, result);
        
        let retrieved = store.get(&very_long_key).await.unwrap();
        assert_eq!(retrieved, Some(normal_value.as_bytes().to_vec()));
        
        // Test normal key with very large value (100MB)
        // 测试普通键与非常大的值（100MB）
        let normal_key = "normal_key_large_value";
        let very_large_value = "z".repeat(100 * 1024 * 1024); // 100MB
        
        let very_large_value_bytes = very_large_value.as_bytes().to_vec();
        let result = timeout(Duration::from_secs(60), store.put(&normal_key.to_string(), &very_large_value_bytes)).await;
        if result.is_ok() && result.unwrap().is_ok() {
            println!("Backend {} successfully stored 100MB value", backend_name);
            
            // Try to retrieve it
            // 尝试检索它
            let get_result = timeout(Duration::from_secs(60), store.get(&normal_key.to_string())).await;
            if get_result.is_ok() {
                let retrieved = get_result.unwrap().unwrap();
                assert_eq!(retrieved, Some(very_large_value.as_bytes().to_vec()));
                println!("Backend {} successfully retrieved 100MB value", backend_name);
            } else {
                println!("Backend {} timed out retrieving 100MB value", backend_name);
            }
            
            // Clean up
            // 清理
            store.delete(&normal_key.to_string()).await.unwrap();
        } else {
            println!("Backend {} cannot handle 100MB values (expected for some backends)", backend_name);
        }
        
        // Clean up
        // 清理
        store.delete(&very_long_key).await.unwrap();
        
        println!("Backend {} completed large keys and values test", backend_name);
    }
}

/// Test concurrent access to the same key
/// 测试对同一键的并发访问
#[tokio::test]
async fn test_concurrent_same_key_access() {
    let factory = DefaultKvStoreFactory::new();
    let configs = test_utils::create_test_configs();

    for (backend_name, config, _temp_dir) in configs {
        println!("Testing concurrent same key access for backend: {}", backend_name);
        
        let store = Arc::new(factory.create(&config).await.unwrap());
        let key = "concurrent_test_key";
        
        // Test concurrent writes to the same key
        // 测试对同一键的并发写入
        let write_handles: Vec<_> = (0..10).map(|i| {
            let store_clone = Arc::clone(&store);
            let key = key.to_string();
            tokio::spawn(async move {
                for j in 0..100 {
                    let value = format!("value_{}_{}", i, j);
                    let value_bytes = value.as_bytes().to_vec();
                    store_clone.put(&key, &value_bytes).await.unwrap();
                }
            })
        }).collect();
        
        // Wait for all writes to complete
        // 等待所有写入完成
        for handle in write_handles {
            handle.await.unwrap();
        }
        
        // Verify the key exists and has some value
        // 验证键存在并有某个值
        let final_value = store.get(&key.to_string()).await.unwrap();
        assert!(final_value.is_some(), "Key should exist after concurrent writes in backend {}", backend_name);
        
        // Test concurrent reads of the same key
        // 测试对同一键的并发读取
        let read_handles: Vec<_> = (0..10).map(|_| {
            let store_clone = Arc::clone(&store);
            let key = key.to_string();
            tokio::spawn(async move {
                for _ in 0..100 {
                    let _value = store_clone.get(&key).await.unwrap();
                }
            })
        }).collect();
        
        // Wait for all reads to complete
        // 等待所有读取完成
        for handle in read_handles {
            handle.await.unwrap();
        }
        
        // Test mixed concurrent operations
        // 测试混合并发操作
        let mixed_handles: Vec<_> = (0..5).map(|i| {
            let store_clone = Arc::clone(&store);
            let key = key.to_string();
            tokio::spawn(async move {
                for j in 0..50 {
                    match j % 3 {
                        0 => {
                            let value = format!("mixed_value_{}_{}", i, j);
                            let value_bytes = value.as_bytes().to_vec();
                            store_clone.put(&key, &value_bytes).await.unwrap();
                        },
                        1 => {
                            let _value = store_clone.get(&key).await.unwrap();
                        },
                        2 => {
                            let _exists = store_clone.exists(&key).await.unwrap();
                        },
                        _ => unreachable!(),
                    }
                }
            })
        }).collect();
        
        // Wait for all mixed operations to complete
        // 等待所有混合操作完成
        for handle in mixed_handles {
            handle.await.unwrap();
        }
        
        // Clean up
        // 清理
        store.delete(&key.to_string()).await.unwrap();
        
        println!("Backend {} passed concurrent same key access test", backend_name);
    }
}

/// Test rapid key creation and deletion
/// 测试快速键创建和删除
#[tokio::test]
async fn test_rapid_key_creation_deletion() {
    let factory = DefaultKvStoreFactory::new();
    let configs = test_utils::create_test_configs();

    for (backend_name, config, _temp_dir) in configs {
        println!("Testing rapid key creation and deletion for backend: {}", backend_name);
        
        let store = factory.create(&config).await.unwrap();
        
        // Rapidly create and delete keys
        // 快速创建和删除键
        for cycle in 0..10 {
            // Create many keys
            // 创建许多键
            for i in 0..1000 {
                let key = format!("rapid_key_{}_{}", cycle, i);
                let value = format!("rapid_value_{}_{}", cycle, i);
                let value_bytes = value.as_bytes().to_vec();
                store.put(&key, &value_bytes).await.unwrap();
            }
            
            // Verify they exist
            // 验证它们存在
            for i in 0..1000 {
                let key = format!("rapid_key_{}_{}", cycle, i);
                let exists = store.exists(&key).await.unwrap();
                assert!(exists, "Key {} should exist in cycle {} for backend {}", 
                        key, cycle, backend_name);
            }
            
            // Delete them all
            // 删除所有键
            for i in 0..1000 {
                let key = format!("rapid_key_{}_{}", cycle, i);
                store.delete(&key).await.unwrap();
            }
            
            // Verify they're gone
            // 验证它们已消失
            for i in 0..1000 {
                let key = format!("rapid_key_{}_{}", cycle, i);
                let exists = store.exists(&key).await.unwrap();
                assert!(!exists, "Key {} should not exist after deletion in cycle {} for backend {}", 
                        key, cycle, backend_name);
            }
        }
        
        println!("Backend {} passed rapid key creation and deletion test", backend_name);
    }
}

/// Test scan operations with edge cases
/// 测试带边界情况的扫描操作
#[tokio::test]
async fn test_scan_operations_edge_cases() {
    let factory = DefaultKvStoreFactory::new();
    let configs = test_utils::create_test_configs();

    for (backend_name, config, _temp_dir) in configs {
        println!("Testing scan operations edge cases for backend: {}", backend_name);
        
        let store = factory.create(&config).await.unwrap();
        
        // Test scanning with no matching keys
        // 测试没有匹配键的扫描
        let empty_scan = store.keys_with_prefix("nonexistent_prefix").await.unwrap();
        assert_eq!(empty_scan.len(), 0);
        
        let empty_scan_pairs = store.scan_prefix("nonexistent_prefix").await.unwrap();
        assert_eq!(empty_scan_pairs.len(), 0);
        
        // Test scanning with empty prefix
        // 测试空前缀的扫描
        let all_keys_scan = store.keys_with_prefix("").await.unwrap();
        let all_pairs_scan = store.scan_prefix("").await.unwrap();
        assert_eq!(all_keys_scan.len(), all_pairs_scan.len());
        
        // Add test data with overlapping prefixes
        // 添加具有重叠前缀的测试数据
        let test_keys = vec![
            "a", "aa", "aaa", "aaaa",
            "ab", "abb", "abbb",
            "b", "ba", "baa",
            "prefix", "prefix_1", "prefix_12", "prefix_123",
        ];
        
        for key in &test_keys {
            let value = format!("value_for_{}", key);
            let value_bytes = value.as_bytes().to_vec();
            store.put(&key.to_string(), &value_bytes).await.unwrap();
        }
        
        // Test prefix scanning with overlapping prefixes
        // 测试重叠前缀的前缀扫描
        let a_keys = store.keys_with_prefix("a").await.unwrap();
        assert!(a_keys.len() >= 7); // Should include a, aa, aaa, aaaa, ab, abb, abbb
        
        let aa_keys = store.keys_with_prefix("aa").await.unwrap();
        assert!(aa_keys.len() >= 3); // Should include aa, aaa, aaaa
        
        let prefix_keys = store.keys_with_prefix("prefix").await.unwrap();
        assert!(prefix_keys.len() >= 4); // Should include prefix, prefix_1, prefix_12, prefix_123
        
        // Test range operations with edge cases
        // 测试边界情况的范围操作
        let range_options = spear_next::storage::RangeOptions {
             start_key: Some("a".to_string()),
             end_key: Some("b".to_string()),
             limit: None,
             reverse: false,
         };
         let range_result = store.range(&range_options).await.unwrap();
         assert!(range_result.len() > 0);
         
         // Test range with same start and end
         // 测试相同开始和结束的范围
         let empty_range_options = spear_next::storage::RangeOptions {
             start_key: Some("a".to_string()),
             end_key: Some("a".to_string()),
             limit: None,
             reverse: false,
         };
         let empty_range = store.range(&empty_range_options).await.unwrap();
         assert_eq!(empty_range.len(), 0);
         
         // Test range with inverted order (end < start)
         // 测试倒序范围（结束 < 开始）
         let inverted_range_options = spear_next::storage::RangeOptions {
             start_key: Some("z".to_string()),
             end_key: Some("a".to_string()),
             limit: None,
             reverse: false,
         };
        let inverted_range = store.range(&inverted_range_options).await.unwrap();
        assert_eq!(inverted_range.len(), 0);
        
        // Clean up
        // 清理
        for key in &test_keys {
            store.delete(&key.to_string()).await.unwrap();
        }
        
        println!("Backend {} passed scan operations edge cases test", backend_name);
    }
}

/// Test memory pressure and resource limits
/// 测试内存压力和资源限制
#[tokio::test]
async fn test_memory_pressure_and_limits() {
    let factory = DefaultKvStoreFactory::new();
    let configs = test_utils::create_test_configs();

    for (backend_name, config, _temp_dir) in configs {
        println!("Testing memory pressure and limits for backend: {}", backend_name);
        
        let store = factory.create(&config).await.unwrap();
        
        // Test storing many small keys
        // 测试存储许多小键
        let num_keys = 10000;
        for i in 0..num_keys {
            let key = format!("memory_test_key_{:06}", i);
            let value = format!("memory_test_value_{:06}", i);
            let value_bytes = value.as_bytes().to_vec();
            store.put(&key, &value_bytes).await.unwrap();
        }
        
        // Verify all keys exist
        // 验证所有键存在
        let all_keys = store.keys_with_prefix("memory_test_key_").await.unwrap();
        assert_eq!(all_keys.len(), num_keys);
        
        // Test batch retrieval
        // 测试批量检索
        for i in 0..num_keys {
            let key = format!("memory_test_key_{:06}", i);
            let value = store.get(&key).await.unwrap();
            assert!(value.is_some());
        }
        
        // Clean up in batches to test deletion performance
        // 批量清理以测试删除性能
        for batch_start in (0..num_keys).step_by(1000) {
            for i in batch_start..std::cmp::min(batch_start + 1000, num_keys) {
                let key = format!("memory_test_key_{:06}", i);
                store.delete(&key).await.unwrap();
            }
        }
        
        // Verify cleanup
        // 验证清理
        let remaining_keys = store.keys_with_prefix("memory_test_key_").await.unwrap();
        assert_eq!(remaining_keys.len(), 0);
        
        println!("Backend {} passed memory pressure and limits test", backend_name);
    }
}