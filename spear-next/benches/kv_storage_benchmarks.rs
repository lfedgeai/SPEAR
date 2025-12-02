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
use std::sync::Arc;

use spear_next::storage::{KvStore, KvStoreConfig, DefaultKvStoreFactory, KvStoreFactory};
use spear_next::storage::kv::RangeOptions;

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
        let store: Arc<dyn KvStore> = Arc::from(rt.block_on(factory.create(&config.config)).unwrap());
        
        // Benchmark SET operations
        // 基准测试SET操作
        group.bench_with_input(
            BenchmarkId::new("set", config.name),
            &config.name,
            |b, _| {
                use std::sync::atomic::{AtomicUsize, Ordering};
                let counter = Arc::new(AtomicUsize::new(0));
                let store = Arc::clone(&store);
                b.to_async(&rt).iter(|| {
                    let store = Arc::clone(&store);
                    let counter = Arc::clone(&counter);
                    async move {
                        let id = counter.fetch_add(1, Ordering::Relaxed);
                        let key = format!("bench_key_{}", id);
                        let value = format!("bench_value_{}", id);
                        let vb = value.into_bytes();
                        black_box(store.put(&key, &vb).await.unwrap());
                    }
                });
            },
        );
        
        // Setup data for GET benchmarks
        // 为GET基准测试设置数据
        rt.block_on(async {
            for i in 0..1000 {
                let key = format!("get_bench_key_{}", i);
                    let value = format!("get_bench_value_{}", i);
                    let vb = value.as_bytes().to_vec();
                    store.put(&key, &vb).await.unwrap();
            }
        });
        
        // Benchmark GET operations
        // 基准测试GET操作
        group.bench_with_input(
            BenchmarkId::new("get", config.name),
            &config.name,
            |b, _| {
                use std::sync::atomic::{AtomicUsize, Ordering};
                let counter = Arc::new(AtomicUsize::new(0));
                let store = Arc::clone(&store);
                b.to_async(&rt).iter(|| {
                    let store = Arc::clone(&store);
                    let counter = Arc::clone(&counter);
                    async move {
                        let id = counter.fetch_add(1, Ordering::Relaxed);
                        let key = format!("get_bench_key_{}", id % 1000);
                        black_box(store.get(&key).await.unwrap());
                    }
                });
            },
        );
        
        // Benchmark DELETE operations
        // 基准测试DELETE操作
        group.bench_with_input(
            BenchmarkId::new("delete", config.name),
            &config.name,
            |b, _| {
                use std::sync::atomic::{AtomicUsize, Ordering};
                let counter = Arc::new(AtomicUsize::new(0));
                let store = Arc::clone(&store);
                b.to_async(&rt).iter(|| {
                    let store = Arc::clone(&store);
                    let counter = Arc::clone(&counter);
                    async move {
                        let id = counter.fetch_add(1, Ordering::Relaxed);
                        let key = format!("delete_bench_key_{}", id);
                        let value = format!("delete_bench_value_{}", id);
                        let vb = value.into_bytes();
                        store.put(&key, &vb).await.unwrap();
                        black_box(store.delete(&key).await.unwrap());
                    }
                });
            },
        );
        
        // Benchmark EXISTS operations
        // 基准测试EXISTS操作
        group.bench_with_input(
            BenchmarkId::new("exists", config.name),
            &config.name,
            |b, _| {
                use std::sync::atomic::{AtomicUsize, Ordering};
                let counter = Arc::new(AtomicUsize::new(0));
                let store = Arc::clone(&store);
                b.to_async(&rt).iter(|| {
                    let store = Arc::clone(&store);
                    let counter = Arc::clone(&counter);
                    async move {
                        let id = counter.fetch_add(1, Ordering::Relaxed);
                        let key = format!("get_bench_key_{}", id % 1000);
                        black_box(store.exists(&key).await.unwrap());
                    }
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
            let store: Arc<dyn KvStore> = Arc::from(rt.block_on(factory.create(&config.config)).unwrap());
            let test_data = generate_test_data(batch_size, 32, 128);
            
            // Benchmark batch SET operations
            // 基准测试批量SET操作
            group.bench_with_input(
                BenchmarkId::new(format!("batch_set_{}", config.name), batch_size),
                &batch_size,
                |b, _| {
                    use std::sync::atomic::{AtomicUsize, Ordering};
                    let batch_counter = Arc::new(AtomicUsize::new(0));
                    let store = Arc::clone(&store);
                    let test_data = test_data.clone();
                    b.to_async(&rt).iter(|| {
                        let store = Arc::clone(&store);
                        let test_data = test_data.clone();
                        let batch_counter = Arc::clone(&batch_counter);
                        async move {
                            let id = batch_counter.fetch_add(1, Ordering::Relaxed);
                            for (key, value) in &test_data {
                                let unique_key = format!("{}_{}", key, id);
                                let vb = value.as_bytes().to_vec();
                                black_box(store.put(&unique_key, &vb).await.unwrap());
                            }
                        }
                    });
                },
            );
            
            // Setup data for batch GET operations
            // 为批量GET操作设置数据
            rt.block_on(async {
                for (key, value) in &test_data {
                    let vb = value.as_bytes().to_vec();
                    store.put(key, &vb).await.unwrap();
                }
            });
            
            // Benchmark batch GET operations
            // 基准测试批量GET操作
            group.bench_with_input(
                BenchmarkId::new(format!("batch_get_{}", config.name), batch_size),
                &batch_size,
                |b, _| {
                    let store = Arc::clone(&store);
                    let test_data = test_data.clone();
                    b.to_async(&rt).iter(|| {
                        let store = Arc::clone(&store);
                        let test_data = test_data.clone();
                        async move {
                            for (key, _) in &test_data {
                                black_box(store.get(key).await.unwrap());
                            }
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
        let store: Arc<dyn KvStore> = Arc::from(rt.block_on(factory.create(&config.config)).unwrap());
        
        // Setup test data with different prefixes
        // 使用不同前缀设置测试数据
        rt.block_on(async {
            for prefix in ["user", "session", "cache", "config"] {
                for i in 0..250 {
                    let key = format!("{}_{:04}", prefix, i);
                    let value = format!("value_{}_{}", prefix, i);
                    let vb = value.as_bytes().to_vec();
                    store.put(&key, &vb).await.unwrap();
                }
            }
        });
        
        // Benchmark keys_with_prefix
        // 基准测试keys_with_prefix
        group.bench_with_input(
            BenchmarkId::new("keys_with_prefix", config.name),
            &config.name,
            |b, _| {
                use std::sync::atomic::{AtomicUsize, Ordering};
                let prefix_counter = Arc::new(AtomicUsize::new(0));
                let store = Arc::clone(&store);
                b.to_async(&rt).iter(|| {
                    let store = Arc::clone(&store);
                    let prefix_counter = Arc::clone(&prefix_counter);
                    async move {
                        let prefixes = ["user", "session", "cache", "config"];
                        let id = prefix_counter.fetch_add(1, Ordering::Relaxed);
                        let prefix = prefixes[id % prefixes.len()];
                        black_box(store.keys_with_prefix(prefix).await.unwrap());
                    }
                });
            },
        );
        
        // Benchmark scan_prefix
        // 基准测试scan_prefix
        group.bench_with_input(
            BenchmarkId::new("scan_prefix", config.name),
            &config.name,
            |b, _| {
                use std::sync::atomic::{AtomicUsize, Ordering};
                let prefix_counter = Arc::new(AtomicUsize::new(0));
                let store = Arc::clone(&store);
                b.to_async(&rt).iter(|| {
                    let store = Arc::clone(&store);
                    let prefix_counter = Arc::clone(&prefix_counter);
                    async move {
                        let prefixes = ["user", "session", "cache", "config"];
                        let id = prefix_counter.fetch_add(1, Ordering::Relaxed);
                        let prefix = prefixes[id % prefixes.len()];
                        black_box(store.scan_prefix(prefix).await.unwrap());
                    }
                });
            },
        );
        
        // Benchmark range operations
        // 基准测试范围操作
        group.bench_with_input(
            BenchmarkId::new("range", config.name),
            &config.name,
            |b, _| {
                use std::sync::atomic::{AtomicUsize, Ordering};
                let range_counter = Arc::new(AtomicUsize::new(0));
                let store = Arc::clone(&store);
                b.to_async(&rt).iter(|| {
                    let store = Arc::clone(&store);
                    let range_counter = Arc::clone(&range_counter);
                    async move {
                        let ranges = [
                            ("user_0000", "user_0100"),
                            ("session_0000", "session_0100"),
                            ("cache_0000", "cache_0100"),
                            ("config_0000", "config_0100"),
                        ];
                        let id = range_counter.fetch_add(1, Ordering::Relaxed);
                        let (start, end) = ranges[id % ranges.len()];
                        let options = RangeOptions::new().start_key(start).end_key(end);
                        black_box(store.range(&options).await.unwrap());
                    }
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
            let store: Arc<dyn KvStore> = Arc::from(rt.block_on(factory.create(&config.config)).unwrap());
            let large_value = "x".repeat(value_size);
            let large_value_bytes = large_value.clone().into_bytes();
            
            // Benchmark large value SET
            // 基准测试大值SET
            group.bench_with_input(
                BenchmarkId::new(format!("large_set_{}_{}", config.name, value_size), value_size),
                &value_size,
                |b, _| {
                    use std::sync::atomic::{AtomicUsize, Ordering};
                    let counter = Arc::new(AtomicUsize::new(0));
                    let store = Arc::clone(&store);
                    let lv = large_value_bytes.clone();
                    b.to_async(&rt).iter(|| {
                        let store = Arc::clone(&store);
                        let lv = lv.clone();
                        let counter = Arc::clone(&counter);
                        async move {
                            let id = counter.fetch_add(1, Ordering::Relaxed);
                            let key = format!("large_key_{}", id);
                            black_box(store.put(&key, &lv).await.unwrap());
                        }
                    });
                },
            );
            
            // Setup data for large value GET
            // 为大值GET设置数据
            rt.block_on(async {
                for i in 0..10 {
                    let key = format!("large_get_key_{}", i);
                    store.put(&key, &large_value_bytes).await.unwrap();
                }
            });
            
            // Benchmark large value GET
            // 基准测试大值GET
            group.bench_with_input(
                BenchmarkId::new(format!("large_get_{}_{}", config.name, value_size), value_size),
                &value_size,
                |b, _| {
                    use std::sync::atomic::{AtomicUsize, Ordering};
                    let counter = Arc::new(AtomicUsize::new(0));
                    let store = Arc::clone(&store);
                    b.to_async(&rt).iter(|| {
                        let store = Arc::clone(&store);
                        let counter = Arc::clone(&counter);
                        async move {
                            let id = counter.fetch_add(1, Ordering::Relaxed);
                            let key = format!("large_get_key_{}", id % 10);
                            black_box(store.get(&key).await.unwrap());
                        }
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
        let store: Arc<dyn KvStore> = Arc::from(rt.block_on(factory.create(&config.config)).unwrap());
        
        // Benchmark concurrent SET operations
        // 基准测试并发SET操作
        group.bench_with_input(
            BenchmarkId::new("concurrent_set", config.name),
            &config.name,
            |b, _| {
                let mut counter = std::sync::atomic::AtomicUsize::new(0);
                b.to_async(&rt).iter(|| async {
                    let tasks: Vec<_> = (0..10).map(|task_id| {
                        let store = Arc::clone(&store);
                        let counter = &counter;
                        async move {
                            let id = counter.fetch_add(1, std::sync::atomic::Ordering::Relaxed);
                            let key = format!("concurrent_key_{}_{}", task_id, id);
                            let value = format!("concurrent_value_{}_{}", task_id, id);
                            let vb = value.into_bytes();
                            store.put(&key, &vb).await.unwrap();
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
                    let vb = value.as_bytes().to_vec();
                    store.put(&key, &vb).await.unwrap();
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
                        let store = Arc::clone(&store);
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
