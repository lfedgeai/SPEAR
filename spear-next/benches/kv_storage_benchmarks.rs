//! KV Storage Performance Benchmarks
//! KV存储性能基准测试
//!
//! This module contains performance benchmarks for different KV storage backends
//! to help users choose the most appropriate backend for their use case.
//! 此模块包含不同KV存储后端的性能基准测试，帮助用户为其用例选择最合适的后端。

use criterion::{black_box, criterion_group, criterion_main, Criterion, BenchmarkId};
use std::time::Duration;
use tokio::runtime::Runtime;
use tempfile::TempDir;
use uuid::Uuid;

use spear_next::storage::{KvStore, KvStoreConfig, DefaultKvStoreFactory, KvStoreFactory};

/// Benchmark configuration
/// 基准测试配置
struct BenchmarkConfig {
    name: &'static str,
    config: KvStoreConfig,
    _temp_dir: Option<TempDir>, // Keep temp dir alive
}

impl BenchmarkConfig {
    /// Create benchmark configurations for all available backends
    /// 为所有可用后端创建基准测试配置
    fn create_all() -> Vec<Self> {
        let mut configs = vec![
            BenchmarkConfig {
                name: "memory",
                config: KvStoreConfig::memory(),
                _temp_dir: None,
            }
        ];

        #[cfg(feature = "sled")]
        {
            let temp_dir = TempDir::new().unwrap();
            let sled_config = KvStoreConfig::sled(temp_dir.path().to_str().unwrap());
            configs.push(BenchmarkConfig {
                name: "sled",
                config: sled_config,
                _temp_dir: Some(temp_dir),
            });
        }

        #[cfg(feature = "rocksdb")]
        {
            let temp_dir = TempDir::new().unwrap();
            let rocksdb_config = KvStoreConfig::rocksdb(temp_dir.path().to_str().unwrap());
            configs.push(BenchmarkConfig {
                name: "rocksdb",
                config: rocksdb_config,
                _temp_dir: Some(temp_dir),
            });
        }

        configs
    }
}

/// Generate test data for benchmarks
/// 为基准测试生成测试数据
fn generate_test_data(count: usize, key_size: usize, value_size: usize) -> Vec<(String, String)> {
    (0..count)
        .map(|i| {
            let key = format!("{:0width$}_{}", i, Uuid::new_v4().to_string(), width = key_size.saturating_sub(37));
            let value = format!("{}{}", "x".repeat(value_size.saturating_sub(10)), Uuid::new_v4().to_string());
            (key, value)
        })
        .collect()
}

/// Benchmark single key operations
/// 基准测试单键操作
fn bench_single_operations(c: &mut Criterion) {
    let rt = Runtime::new().unwrap();
    let configs = BenchmarkConfig::create_all();
    
    let mut group = c.benchmark_group("single_operations");
    group.measurement_time(Duration::from_secs(10));
    
    for config in &configs {
        let factory = DefaultKvStoreFactory::new();
        let store = rt.block_on(factory.create_kv_store(&config.config)).unwrap();
        
        // Benchmark SET operations
        // 基准测试SET操作
        group.bench_with_input(
            BenchmarkId::new("set", config.name),
            &config.name,
            |b, _| {
                let mut counter = 0;
                b.to_async(&rt).iter(|| async {
                    let key = format!("bench_key_{}", counter);
                    let value = format!("bench_value_{}", counter);
                    counter += 1;
                    black_box(store.set(&key, &value).await.unwrap());
                });
            },
        );
        
        // Setup data for GET benchmarks
        // 为GET基准测试设置数据
        rt.block_on(async {
            for i in 0..1000 {
                let key = format!("get_bench_key_{}", i);
                let value = format!("get_bench_value_{}", i);
                store.set(&key, &value).await.unwrap();
            }
        });
        
        // Benchmark GET operations
        // 基准测试GET操作
        group.bench_with_input(
            BenchmarkId::new("get", config.name),
            &config.name,
            |b, _| {
                let mut counter = 0;
                b.to_async(&rt).iter(|| async {
                    let key = format!("get_bench_key_{}", counter % 1000);
                    counter += 1;
                    black_box(store.get(&key).await.unwrap());
                });
            },
        );
        
        // Benchmark DELETE operations
        // 基准测试DELETE操作
        group.bench_with_input(
            BenchmarkId::new("delete", config.name),
            &config.name,
            |b, _| {
                let mut counter = 0;
                b.to_async(&rt).iter(|| async {
                    // Setup key for deletion
                    // 为删除设置键
                    let key = format!("delete_bench_key_{}", counter);
                    let value = format!("delete_bench_value_{}", counter);
                    store.set(&key, &value).await.unwrap();
                    
                    counter += 1;
                    black_box(store.delete(&key).await.unwrap());
                });
            },
        );
        
        // Benchmark EXISTS operations
        // 基准测试EXISTS操作
        group.bench_with_input(
            BenchmarkId::new("exists", config.name),
            &config.name,
            |b, _| {
                let mut counter = 0;
                b.to_async(&rt).iter(|| async {
                    let key = format!("get_bench_key_{}", counter % 1000);
                    counter += 1;
                    black_box(store.exists(&key).await.unwrap());
                });
            },
        );
    }
    
    group.finish();
}

/// Benchmark batch operations
/// 基准测试批量操作
fn bench_batch_operations(c: &mut Criterion) {
    let rt = Runtime::new().unwrap();
    let configs = BenchmarkConfig::create_all();
    
    let mut group = c.benchmark_group("batch_operations");
    group.measurement_time(Duration::from_secs(15));
    
    let batch_sizes = vec![10, 100, 1000];
    
    for config in &configs {
        let factory = DefaultKvStoreFactory::new();
        
        for &batch_size in &batch_sizes {
            let store = rt.block_on(factory.create_kv_store(&config.config)).unwrap();
            let test_data = generate_test_data(batch_size, 32, 128);
            
            // Benchmark batch SET operations
            // 基准测试批量SET操作
            group.bench_with_input(
                BenchmarkId::new(format!("batch_set_{}", config.name), batch_size),
                &batch_size,
                |b, _| {
                    let mut batch_counter = 0;
                    b.to_async(&rt).iter(|| async {
                        for (key, value) in &test_data {
                            let unique_key = format!("{}_{}", key, batch_counter);
                            black_box(store.set(&unique_key, value).await.unwrap());
                        }
                        batch_counter += 1;
                    });
                },
            );
            
            // Setup data for batch GET operations
            // 为批量GET操作设置数据
            rt.block_on(async {
                for (key, value) in &test_data {
                    store.set(key, value).await.unwrap();
                }
            });
            
            // Benchmark batch GET operations
            // 基准测试批量GET操作
            group.bench_with_input(
                BenchmarkId::new(format!("batch_get_{}", config.name), batch_size),
                &batch_size,
                |b, _| {
                    b.to_async(&rt).iter(|| async {
                        for (key, _) in &test_data {
                            black_box(store.get(key).await.unwrap());
                        }
                    });
                },
            );
        }
    }
    
    group.finish();
}

/// Benchmark scan operations
/// 基准测试扫描操作
fn bench_scan_operations(c: &mut Criterion) {
    let rt = Runtime::new().unwrap();
    let configs = BenchmarkConfig::create_all();
    
    let mut group = c.benchmark_group("scan_operations");
    group.measurement_time(Duration::from_secs(15));
    
    for config in &configs {
        let factory = DefaultKvStoreFactory::new();
        let store = rt.block_on(factory.create_kv_store(&config.config)).unwrap();
        
        // Setup test data with different prefixes
        // 使用不同前缀设置测试数据
        rt.block_on(async {
            for prefix in ["user", "session", "cache", "config"] {
                for i in 0..250 {
                    let key = format!("{}_{:04}", prefix, i);
                    let value = format!("value_{}_{}", prefix, i);
                    store.set(&key, &value).await.unwrap();
                }
            }
        });
        
        // Benchmark keys_with_prefix
        // 基准测试keys_with_prefix
        group.bench_with_input(
            BenchmarkId::new("keys_with_prefix", config.name),
            &config.name,
            |b, _| {
                let prefixes = ["user", "session", "cache", "config"];
                let mut prefix_counter = 0;
                b.to_async(&rt).iter(|| async {
                    let prefix = prefixes[prefix_counter % prefixes.len()];
                    prefix_counter += 1;
                    black_box(store.keys_with_prefix(prefix).await.unwrap());
                });
            },
        );
        
        // Benchmark scan_prefix
        // 基准测试scan_prefix
        group.bench_with_input(
            BenchmarkId::new("scan_prefix", config.name),
            &config.name,
            |b, _| {
                let prefixes = ["user", "session", "cache", "config"];
                let mut prefix_counter = 0;
                b.to_async(&rt).iter(|| async {
                    let prefix = prefixes[prefix_counter % prefixes.len()];
                    prefix_counter += 1;
                    black_box(store.scan_prefix(prefix).await.unwrap());
                });
            },
        );
        
        // Benchmark range operations
        // 基准测试范围操作
        group.bench_with_input(
            BenchmarkId::new("range", config.name),
            &config.name,
            |b, _| {
                let ranges = [
                    ("user_0000", "user_0100"),
                    ("session_0000", "session_0100"),
                    ("cache_0000", "cache_0100"),
                    ("config_0000", "config_0100"),
                ];
                let mut range_counter = 0;
                b.to_async(&rt).iter(|| async {
                    let (start, end) = ranges[range_counter % ranges.len()];
                    range_counter += 1;
                    black_box(store.range(start, end).await.unwrap());
                });
            },
        );
    }
    
    group.finish();
}

/// Benchmark large value operations
/// 基准测试大值操作
fn bench_large_value_operations(c: &mut Criterion) {
    let rt = Runtime::new().unwrap();
    let configs = BenchmarkConfig::create_all();
    
    let mut group = c.benchmark_group("large_value_operations");
    group.measurement_time(Duration::from_secs(20));
    group.sample_size(10); // Reduce sample size for large operations
    
    let value_sizes = vec![1024, 10240, 102400]; // 1KB, 10KB, 100KB
    
    for config in &configs {
        let factory = DefaultKvStoreFactory::new();
        
        for &value_size in &value_sizes {
            let store = rt.block_on(factory.create_kv_store(&config.config)).unwrap();
            let large_value = "x".repeat(value_size);
            
            // Benchmark large value SET
            // 基准测试大值SET
            group.bench_with_input(
                BenchmarkId::new(format!("large_set_{}_{}", config.name, value_size), value_size),
                &value_size,
                |b, _| {
                    let mut counter = 0;
                    b.to_async(&rt).iter(|| async {
                        let key = format!("large_key_{}", counter);
                        counter += 1;
                        black_box(store.set(&key, &large_value).await.unwrap());
                    });
                },
            );
            
            // Setup data for large value GET
            // 为大值GET设置数据
            rt.block_on(async {
                for i in 0..10 {
                    let key = format!("large_get_key_{}", i);
                    store.set(&key, &large_value).await.unwrap();
                }
            });
            
            // Benchmark large value GET
            // 基准测试大值GET
            group.bench_with_input(
                BenchmarkId::new(format!("large_get_{}_{}", config.name, value_size), value_size),
                &value_size,
                |b, _| {
                    let mut counter = 0;
                    b.to_async(&rt).iter(|| async {
                        let key = format!("large_get_key_{}", counter % 10);
                        counter += 1;
                        black_box(store.get(&key).await.unwrap());
                    });
                },
            );
        }
    }
    
    group.finish();
}

/// Benchmark concurrent operations
/// 基准测试并发操作
fn bench_concurrent_operations(c: &mut Criterion) {
    let rt = Runtime::new().unwrap();
    let configs = BenchmarkConfig::create_all();
    
    let mut group = c.benchmark_group("concurrent_operations");
    group.measurement_time(Duration::from_secs(20));
    group.sample_size(10);
    
    for config in &configs {
        let factory = DefaultKvStoreFactory::new();
        let store = rt.block_on(factory.create_kv_store(&config.config)).unwrap();
        
        // Benchmark concurrent SET operations
        // 基准测试并发SET操作
        group.bench_with_input(
            BenchmarkId::new("concurrent_set", config.name),
            &config.name,
            |b, _| {
                let mut counter = std::sync::atomic::AtomicUsize::new(0);
                b.to_async(&rt).iter(|| async {
                    let tasks: Vec<_> = (0..10).map(|task_id| {
                        let store = store.clone();
                        let counter = &counter;
                        async move {
                            let id = counter.fetch_add(1, std::sync::atomic::Ordering::Relaxed);
                            let key = format!("concurrent_key_{}_{}", task_id, id);
                            let value = format!("concurrent_value_{}_{}", task_id, id);
                            store.set(&key, &value).await.unwrap();
                        }
                    }).collect();
                    
                    black_box(futures::future::join_all(tasks).await);
                });
            },
        );
        
        // Setup data for concurrent GET operations
        // 为并发GET操作设置数据
        rt.block_on(async {
            for i in 0..100 {
                let key = format!("concurrent_get_key_{}", i);
                let value = format!("concurrent_get_value_{}", i);
                store.set(&key, &value).await.unwrap();
            }
        });
        
        // Benchmark concurrent GET operations
        // 基准测试并发GET操作
        group.bench_with_input(
            BenchmarkId::new("concurrent_get", config.name),
            &config.name,
            |b, _| {
                let mut counter = std::sync::atomic::AtomicUsize::new(0);
                b.to_async(&rt).iter(|| async {
                    let tasks: Vec<_> = (0..10).map(|_| {
                        let store = store.clone();
                        let counter = &counter;
                        async move {
                            let id = counter.fetch_add(1, std::sync::atomic::Ordering::Relaxed);
                            let key = format!("concurrent_get_key_{}", id % 100);
                            store.get(&key).await.unwrap()
                        }
                    }).collect();
                    
                    black_box(futures::future::join_all(tasks).await);
                });
            },
        );
    }
    
    group.finish();
}

criterion_group!(
    benches,
    bench_single_operations,
    bench_batch_operations,
    bench_scan_operations,
    bench_large_value_operations,
    bench_concurrent_operations
);
criterion_main!(benches);