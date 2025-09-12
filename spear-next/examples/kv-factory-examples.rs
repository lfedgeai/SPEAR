// KV Store Factory Pattern Examples / KV存储工厂模式示例
// This file demonstrates how to use the factory pattern for dynamic KV store selection
// 本文件演示如何使用工厂模式进行动态KV存储选择

use spear_next::storage::{
    KvStore, KvStoreConfig, KvStoreFactory, DefaultKvStoreFactory,
    create_kv_store_from_config, create_kv_store_from_env,
    get_kv_store_factory
};

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    // Example 1: Basic factory usage / 示例1：基本工厂使用
    println!("=== Example 1: Basic Factory Usage ===");
    basic_factory_usage().await?;

    // Example 2: Configuration-driven selection / 示例2：配置驱动选择
    println!("\n=== Example 2: Configuration-driven Selection ===");
    config_driven_selection().await?;

    // Example 3: Environment variable configuration / 示例3：环境变量配置
    println!("\n=== Example 3: Environment Variable Configuration ===");
    env_var_configuration().await?;

    // Example 4: Custom factory implementation / 示例4：自定义工厂实现
    println!("\n=== Example 4: Custom Factory Implementation ===");
    custom_factory_implementation().await?;

    // Example 5: Runtime backend switching / 示例5：运行时后端切换
    println!("\n=== Example 5: Runtime Backend Switching ===");
    runtime_backend_switching().await?;

    Ok(())
}

// Example 1: Basic factory usage / 示例1：基本工厂使用
async fn basic_factory_usage() -> Result<(), Box<dyn std::error::Error>> {
    // Create a factory instance / 创建工厂实例
    let factory = DefaultKvStoreFactory::new();
    
    // Create memory store / 创建内存存储
    let memory_config = KvStoreConfig::memory();
    let memory_store = factory.create(&memory_config).await?;
    
    // Test basic operations / 测试基本操作
    memory_store.put(&"key1".to_string(), &"value1".as_bytes().to_vec()).await?;
    let value = memory_store.get(&"key1".to_string()).await?;
    println!("Memory store - Retrieved: {:?}", String::from_utf8_lossy(&value.unwrap()));
    
    #[cfg(feature = "sled")]
    {
        // Create sled store / 创建sled存储
        let sled_config = KvStoreConfig::sled("/tmp/example_db");
        let sled_store = factory.create(&sled_config).await?;
        
        // Test basic operations / 测试基本操作
        sled_store.put(&"key2".to_string(), &"value2".as_bytes().to_vec()).await?;
        let value = sled_store.get(&"key2".to_string()).await?;
        println!("Sled store - Retrieved: {:?}", String::from_utf8_lossy(&value.unwrap()));
    }
    
    Ok(())
}

// Example 2: Configuration-driven selection / 示例2：配置驱动选择
async fn config_driven_selection() -> Result<(), Box<dyn std::error::Error>> {
    // Simulate loading configuration from file or database / 模拟从文件或数据库加载配置
    let configs = vec![
        KvStoreConfig::memory()
            .with_param("cache_size", "1000")
            .with_param("timeout", "30"),
        #[cfg(feature = "sled")]
        KvStoreConfig::sled("/tmp/config_driven_db")
            .with_param("cache_capacity", "10000")
            .with_param("flush_every_ms", "1000"),
    ];
    
    for (i, config) in configs.iter().enumerate() {
        println!("Testing configuration {}: backend = {}", i + 1, config.backend);
        
        // Create store from configuration / 从配置创建存储
        let store = create_kv_store_from_config(config).await?;
        
        // Test operations / 测试操作
        let key = format!("config_key_{}", i);
        let value = format!("config_value_{}", i);
        
        store.put(&key, &value.as_bytes().to_vec()).await?;
        let retrieved = store.get(&key).await?;
        println!("  Retrieved: {:?}", String::from_utf8_lossy(&retrieved.unwrap()));
        
        // Show configuration parameters / 显示配置参数
        for (param_key, param_value) in &config.params {
            println!("  Parameter: {} = {}", param_key, param_value);
        }
    }
    
    Ok(())
}

// Example 3: Environment variable configuration / 示例3：环境变量配置
async fn env_var_configuration() -> Result<(), Box<dyn std::error::Error>> {
    // Set environment variables / 设置环境变量
    std::env::set_var("KV_STORE_BACKEND", "memory");
    std::env::set_var("KV_STORE_CACHE_SIZE", "5000");
    std::env::set_var("KV_STORE_TIMEOUT", "60");
    
    // Create store from environment / 从环境变量创建存储
    let store = create_kv_store_from_env().await?;
    
    // Test operations / 测试操作
    store.put(&"env_key".to_string(), &"env_value".as_bytes().to_vec()).await?;
    let value = store.get(&"env_key".to_string()).await?;
    println!("Environment-configured store - Retrieved: {:?}", 
             String::from_utf8_lossy(&value.unwrap()));
    
    // Clean up / 清理
    std::env::remove_var("KV_STORE_BACKEND");
    std::env::remove_var("KV_STORE_CACHE_SIZE");
    std::env::remove_var("KV_STORE_TIMEOUT");
    
    Ok(())
}

// Example 4: Custom factory implementation / 示例4：自定义工厂实现
async fn custom_factory_implementation() -> Result<(), Box<dyn std::error::Error>> {
    // Define a custom factory that adds logging / 定义一个添加日志的自定义工厂
    #[derive(Debug)]
    struct LoggingKvStoreFactory {
        inner: DefaultKvStoreFactory,
    }
    
    impl LoggingKvStoreFactory {
        fn new() -> Self {
            Self {
                inner: DefaultKvStoreFactory::new(),
            }
        }
    }
    
    #[async_trait::async_trait]
    impl KvStoreFactory for LoggingKvStoreFactory {
        async fn create(&self, config: &KvStoreConfig) -> Result<Box<dyn KvStore>, spear_next::sms::error::SmsError> {
            println!("Creating KV store with backend: {}", config.backend);
            for (key, value) in &config.params {
                println!("  Parameter: {} = {}", key, value);
            }
            self.inner.create(config).await
        }
        
        fn supported_backends(&self) -> Vec<String> {
            self.inner.supported_backends()
        }
        
        fn validate_config(&self, config: &KvStoreConfig) -> Result<(), spear_next::sms::error::SmsError> {
            println!("Validating config for backend: {}", config.backend);
            self.inner.validate_config(config)
        }
    }
    
    // Use the custom factory / 使用自定义工厂
    let custom_factory = LoggingKvStoreFactory::new();
    let config = KvStoreConfig::memory().with_param("custom", "true");
    let store = custom_factory.create(&config).await?;
    
    // Test operations / 测试操作
    store.put(&"custom_key".to_string(), &"custom_value".as_bytes().to_vec()).await?;
    let value = store.get(&"custom_key".to_string()).await?;
    println!("Custom factory store - Retrieved: {:?}", 
             String::from_utf8_lossy(&value.unwrap()));
    
    Ok(())
}

// Example 5: Runtime backend switching / 示例5：运行时后端切换
async fn runtime_backend_switching() -> Result<(), Box<dyn std::error::Error>> {
    // Simulate different runtime conditions / 模拟不同的运行时条件
    let conditions = vec![
        ("development", "memory"),
        ("testing", "memory"),
        #[cfg(feature = "sled")]
        ("production", "sled"),
    ];
    
    for (environment, backend) in conditions {
        println!("Environment: {} -> Backend: {}", environment, backend);
        
        let config = match backend {
            "memory" => KvStoreConfig::memory()
                .with_param("environment", environment),
            #[cfg(feature = "sled")]
            "sled" => KvStoreConfig::sled(&format!("/tmp/{}_db", environment))
                .with_param("environment", environment),
            _ => continue,
        };
        
        // Create store based on runtime condition / 根据运行时条件创建存储
        let store = create_kv_store_from_config(&config).await?;
        
        // Test operations / 测试操作
        let key = format!("{}_key", environment);
        let value = format!("{}_value", environment);
        
        store.put(&key, &value.as_bytes().to_vec()).await?;
        let retrieved = store.get(&key).await?;
        println!("  Retrieved: {:?}", String::from_utf8_lossy(&retrieved.unwrap()));
        
        // Show store count / 显示存储计数
        let count = store.count().await?;
        println!("  Store count: {}", count);
    }
    
    Ok(())
}

// Additional utility functions / 额外的实用函数

/// Create a store based on application mode / 根据应用模式创建存储
pub async fn create_store_for_mode(mode: &str) -> Result<Box<dyn KvStore>, Box<dyn std::error::Error>> {
    let config = match mode {
        "test" => KvStoreConfig::memory(),
        "dev" => KvStoreConfig::memory().with_param("debug", "true"),
        #[cfg(feature = "sled")]
        "prod" => KvStoreConfig::sled("/var/lib/app/data")
            .with_param("cache_capacity", "100000")
            .with_param("flush_every_ms", "5000"),
        _ => return Err("Unsupported mode".into()),
    };
    
    Ok(create_kv_store_from_config(&config).await?)
}

/// Validate and create store with error handling / 验证并创建存储，包含错误处理
pub async fn safe_create_store(config: &KvStoreConfig) -> Result<Box<dyn KvStore>, Box<dyn std::error::Error>> {
    // Validate configuration first / 首先验证配置
    let factory = get_kv_store_factory();
    factory.validate_config(config)?;
    
    // Create store / 创建存储
    let store = factory.create(config).await?;
    
    // Test basic functionality / 测试基本功能
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