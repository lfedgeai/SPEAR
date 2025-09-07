# KV存储工厂模式文档

## 概述

KV存储工厂模式提供了一种灵活且可配置的方式来在运行时创建不同类型的键值存储。这种模式允许您根据配置、环境变量或运行时条件在不同的存储后端（内存、sled等）之间切换。

## 核心组件

### 1. KvStoreConfig 配置结构体

`KvStoreConfig` 结构体保存创建KV存储的配置信息：

```rust
use spear_next::storage::KvStoreConfig;

// 创建内存存储配置
let memory_config = KvStoreConfig::memory();

// 创建sled存储配置（需要"sled"特性）
#[cfg(feature = "sled")]
let sled_config = KvStoreConfig::sled("/path/to/database");

// 添加自定义参数
let config_with_params = KvStoreConfig::memory()
    .with_param("cache_size", "1000")
    .with_param("timeout", "30");
```

### 2. KvStoreFactory Trait 工厂特征

`KvStoreFactory` trait定义了创建KV存储的接口：

```rust
use spear_next::storage::{KvStoreFactory, DefaultKvStoreFactory};

let factory = DefaultKvStoreFactory::new();

// 检查支持的后端
let backends = factory.supported_backends();
println!("支持的后端: {:?}", backends);

// 验证配置
let config = KvStoreConfig::memory();
factory.validate_config(&config)?;

// 创建存储
let store = factory.create(&config).await?;
```

### 3. 全局工厂函数

为常见用例提供的便利函数：

```rust
use spear_next::storage::{
    create_kv_store_from_config, 
    create_kv_store_from_env,
    get_kv_store_factory
};

// 从配置创建
let config = KvStoreConfig::memory();
let store = create_kv_store_from_config(&config).await?;

// 从环境变量创建
let store = create_kv_store_from_env().await?;

// 获取全局工厂实例
let factory = get_kv_store_factory();
```

## 配置方法

### 1. 编程式配置

```rust
use spear_next::storage::KvStoreConfig;

// 内存存储
let memory_config = KvStoreConfig::memory()
    .with_param("cache_size", "5000")
    .with_param("debug", "true");

// Sled存储（需要"sled"特性）
#[cfg(feature = "sled")]
let sled_config = KvStoreConfig::sled("/var/lib/app/data")
    .with_param("cache_capacity", "100000")
    .with_param("flush_every_ms", "5000");
```

### 2. 环境变量配置

设置环境变量来配置KV存储：

```bash
# 后端选择
export KV_STORE_BACKEND=memory

# 通用参数（转换为小写）
export KV_STORE_CACHE_SIZE=5000
export KV_STORE_TIMEOUT=60
export KV_STORE_DEBUG=true

# 对于sled后端
export KV_STORE_BACKEND=sled
export KV_STORE_PATH=/path/to/database
```

然后创建存储：

```rust
use spear_next::storage::create_kv_store_from_env;

let store = create_kv_store_from_env().await?;
```

### 3. 配置文件（JSON/TOML）

您可以使用serde序列化/反序列化`KvStoreConfig`：

```rust
use spear_next::storage::KvStoreConfig;
use serde_json;

// 序列化为JSON
let config = KvStoreConfig::memory().with_param("cache_size", "1000");
let json = serde_json::to_string(&config)?;

// 从JSON反序列化
let config: KvStoreConfig = serde_json::from_str(&json)?;
let store = create_kv_store_from_config(&config).await?;
```

## 使用模式

### 1. 基于应用模式的选择

```rust
use spear_next::storage::{KvStoreConfig, create_kv_store_from_config};

async fn create_store_for_mode(mode: &str) -> Result<Box<dyn KvStore>, Box<dyn std::error::Error>> {
    let config = match mode {
        "test" => KvStoreConfig::memory(),
        "dev" => KvStoreConfig::memory().with_param("debug", "true"),
        #[cfg(feature = "sled")]
        "prod" => KvStoreConfig::sled("/var/lib/app/data")
            .with_param("cache_capacity", "100000"),
        _ => return Err("不支持的模式".into()),
    };
    
    Ok(create_kv_store_from_config(&config).await?)
}
```

### 2. 运行时后端切换

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

### 3. 自定义工厂实现

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
        println!("创建KV存储，后端: {}", config.backend);
        self.inner.create(config).await
    }
    
    fn supported_backends(&self) -> Vec<String> {
        self.inner.supported_backends()
    }
    
    fn validate_config(&self, config: &KvStoreConfig) -> Result<(), SmsError> {
        println!("验证配置，后端: {}", config.backend);
        self.inner.validate_config(config)
    }
}
```

## 错误处理

工厂模式包含全面的错误处理：

```rust
use spear_next::storage::{KvStoreConfig, create_kv_store_from_config};

async fn safe_create_store(config: &KvStoreConfig) -> Result<Box<dyn KvStore>, Box<dyn std::error::Error>> {
    // 首先验证配置
    let factory = get_kv_store_factory();
    factory.validate_config(config)?;
    
    // 创建存储
    let store = factory.create(config).await?;
    
    // 测试基本功能
    let test_key = "__health_check__".to_string();
    let test_value = "ok".as_bytes().to_vec();
    
    store.put(&test_key, &test_value).await?;
    let retrieved = store.get(&test_key).await?;
    store.delete(&test_key).await?;
    
    if retrieved != Some(test_value) {
        return Err("存储健康检查失败".into());
    }
    
    Ok(store)
}
```

## 环境变量参考

| 变量 | 描述 | 示例 |
|------|------|------|
| `KV_STORE_BACKEND` | 后端类型（memory, sled） | `memory` |
| `KV_STORE_CACHE_SIZE` | 缓存大小参数 | `5000` |
| `KV_STORE_TIMEOUT` | 超时参数 | `60` |
| `KV_STORE_PATH` | 数据库路径（用于sled） | `/var/lib/app/data` |
| `KV_STORE_DEBUG` | 调试模式 | `true` |
| `SPEAR_KV_BACKEND` | 旧版后端变量 | `memory` |
| `SPEAR_KV_SLED_PATH` | 旧版sled路径变量 | `/path/to/db` |

## 最佳实践

1. **配置验证**：在创建存储之前始终验证配置
2. **错误处理**：为存储创建失败实现适当的错误处理
3. **健康检查**：创建存储后测试基本功能
4. **环境分离**：为不同环境使用不同配置
5. **参数文档**：为您的应用程序记录自定义参数
6. **工厂定制**：为特殊需求实现自定义工厂

## 测试

工厂模式包含全面的测试：

```bash
# 运行所有工厂相关测试
cargo test test_kv_store_config test_factory_validation test_global_factory test_config_from_env --lib --features sled

# 运行特定测试
cargo test test_new_kv_store_factory --lib --features sled
```

## 示例

查看 `examples/kv-factory-examples.rs` 获取演示所有工厂模式功能的完整工作示例。