# KV抽象层

## 概述

KV（键值）抽象层为spear-next项目中的不同存储后端提供了统一的接口。这种抽象允许系统在保持一致API语义的同时，无缝切换不同的存储实现。

## 架构

### 核心组件

1. **KvStore Trait**: 定义所有存储操作的主要抽象接口
2. **MemoryKvStore**: 用于测试和开发的内存实现
3. **SledKvStore**: 使用Sled数据库的持久化存储实现
4. **Serialization模块**: 数据序列化和键生成的辅助函数

### 主要特性

- **异步/等待支持**: 所有操作都是异步的，以获得更好的性能
- **类型安全**: 使用`KvKey`（String）和`KvValue`（Vec<u8>）的强类型
- **范围查询**: 支持基于前缀和基于范围的查询
- **批量操作**: 高效的批量存储和删除操作
- **序列化辅助**: 复杂数据类型的内置JSON序列化

## API参考

### KvStore Trait

```rust
#[async_trait]
pub trait KvStore: Send + Sync + Debug {
    // 基本CRUD操作
    async fn get(&self, key: &KvKey) -> Result<Option<KvValue>, SmsError>;
    async fn put(&self, key: &KvKey, value: &KvValue) -> Result<(), SmsError>;
    async fn delete(&self, key: &KvKey) -> Result<bool, SmsError>;
    async fn exists(&self, key: &KvKey) -> Result<bool, SmsError>;
    
    // 查询操作
    async fn keys_with_prefix(&self, prefix: &str) -> Result<Vec<KvKey>, SmsError>;
    async fn range(&self, options: &RangeOptions) -> Result<Vec<KvPair>, SmsError>;
    async fn all(&self) -> Result<Vec<KvPair>, SmsError>;
    
    // 实用操作
    async fn count(&self) -> Result<usize, SmsError>;
    async fn clear(&self) -> Result<(), SmsError>;
    
    // 批量操作
    async fn batch_put(&self, pairs: &[KvPair]) -> Result<(), SmsError>;
    async fn batch_delete(&self, keys: &[KvKey]) -> Result<usize, SmsError>;
}
```

### 范围查询选项

```rust
#[derive(Debug, Clone, Default)]
pub struct RangeOptions {
    pub start_key: Option<KvKey>,    // 包含的起始键
    pub end_key: Option<KvKey>,      // 不包含的结束键
    pub limit: Option<usize>,        // 最大结果数量
    pub reverse: bool,               // 逆序
}
```

## 使用示例

### 基本操作

```rust
use spear_next::storage::{MemoryKvStore, KvStore};

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    let store = MemoryKvStore::new();
    
    // 存储值
    store.put(&"key1".to_string(), &b"value1".to_vec()).await?;
    
    // 获取值
    if let Some(value) = store.get(&"key1".to_string()).await? {
        println!("值: {:?}", String::from_utf8(value)?);
    }
    
    // 检查存在性
    let exists = store.exists(&"key1".to_string()).await?;
    println!("键存在: {}", exists);
    
    // 删除键
    let deleted = store.delete(&"key1".to_string()).await?;
    println!("键已删除: {}", deleted);
    
    Ok(())
}
```

### 使用序列化

```rust
use spear_next::storage::{MemoryKvStore, KvStore, serialization};
use spear_next::common::node::NodeInfo;

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    let store = MemoryKvStore::new();
    let node = NodeInfo::new("127.0.0.1".to_string(), 8080);
    
    // 序列化并存储节点
    let key = serialization::node_key(&node.uuid);
    let value = serialization::serialize(&node)?;
    store.put(&key, &value).await?;
    
    // 检索并反序列化
    if let Some(data) = store.get(&key).await? {
        let retrieved_node: NodeInfo = serialization::deserialize(&data)?;
        println!("检索到的节点: {:?}", retrieved_node);
    }
    
    Ok(())
}
```

### 范围查询

```rust
use spear_next::storage::{MemoryKvStore, KvStore, RangeOptions};

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    let store = MemoryKvStore::new();
    
    // 插入一些测试数据
    store.put(&"node:001".to_string(), &b"data1".to_vec()).await?;
    store.put(&"node:002".to_string(), &b"data2".to_vec()).await?;
    store.put(&"resource:001".to_string(), &b"resource1".to_vec()).await?;
    
    // 查询所有带有"node:"前缀的键
    let node_keys = store.keys_with_prefix("node:").await?;
    println!("节点键: {:?}", node_keys);
    
    // 带选项的范围查询
    let options = RangeOptions::new()
        .start_key("node:")
        .end_key("node:999")
        .limit(10);
    
    let results = store.range(&options).await?;
    for pair in results {
        println!("键: {}, 值: {:?}", pair.key, String::from_utf8(pair.value)?);
    }
    
    Ok(())
}
```

### 使用Sled后端

```rust
use spear_next::storage::{SledKvStore, KvStore, create_kv_store, KvStoreType};

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    // 直接创建Sled存储
    let store = SledKvStore::new("/tmp/test_db")?;
    
    // 或使用工厂函数
    let store = create_kv_store(KvStoreType::Sled { 
        path: "/tmp/test_db".to_string() 
    })?;
    
    // 使用与MemoryKvStore相同的API
    store.put(&"persistent_key".to_string(), &b"persistent_value".to_vec()).await?;
    
    Ok(())
}
```

## 与NodeRegistry的集成

KV抽象层通过`KvNodeRegistry`实现集成到`NodeRegistry`中：

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
        println!("检索到的节点: {:?}", retrieved_node);
    }
    
    Ok(())
}
```

## 性能考虑

### MemoryKvStore
- **优点**: 极快速度，无I/O开销
- **缺点**: 重启时数据丢失，受可用RAM限制
- **使用场景**: 测试、开发、临时缓存

### SledKvStore
- **优点**: 持久化存储，良好性能，ACID事务
- **缺点**: 磁盘I/O开销，需要文件系统访问
- **使用场景**: 生产环境，数据持久化需求

## 错误处理

所有KV操作返回`Result<T, SmsError>`，其中`SmsError`提供详细的错误信息：

```rust
use spear_next::common::error::SmsError;

match store.get(&key).await {
    Ok(Some(value)) => println!("找到值: {:?}", value),
    Ok(None) => println!("未找到键"),
    Err(SmsError::SerializationError { message }) => {
        eprintln!("序列化错误: {}", message);
    },
    Err(SmsError::StorageError { message }) => {
        eprintln!("存储错误: {}", message);
    },
    Err(e) => eprintln!("其他错误: {:?}", e),
}
```

## 测试

KV抽象层包含全面的测试套件，涵盖基本功能、集成测试、性能测试和边界条件测试：

### 基本单元测试
```bash
# 运行所有KV基础测试
cargo test common::kv

# 运行带有Sled功能的测试
cargo test storage::kv::tests --features sled

# 运行NodeRegistry集成测试
cargo test common::node --features sled
```

### 集成测试
```bash
# 运行KV存储集成测试（包含跨后端兼容性、性能比较、并发操作等）
cargo test --test kv_storage_integration_tests --features sled

# 运行KV存储边界条件测试（包含错误处理、大数据处理、内存压力测试等）
cargo test --test kv_storage_edge_cases --features sled
```

### 测试覆盖范围

#### 集成测试 (kv_storage_integration_tests.rs)
- **跨后端兼容性测试**: 验证Memory和Sled后端的一致性
- **性能比较测试**: 比较不同后端的读写性能
- **大数据处理测试**: 测试大键值对的存储和检索
- **范围操作测试**: 验证前缀扫描和范围查询功能
- **并发操作测试**: 测试多线程环境下的数据一致性
- **资源清理测试**: 验证资源管理和清理机制
- **错误处理测试**: 测试各种错误场景的处理
- **工厂配置验证**: 测试存储工厂的配置验证

#### 边界条件测试 (kv_storage_edge_cases.rs)
- **空键和空白键测试**: 处理特殊键值的边界情况
- **问题值测试**: 测试特殊字符、Unicode、JSON等复杂值
- **大键值测试**: 测试极大的键和值（最大100MB）
- **并发同键访问**: 测试同一键的并发读写操作
- **快速创建删除**: 测试高频率的键创建和删除操作
- **扫描操作边界**: 测试扫描操作的各种边界条件
- **内存压力测试**: 测试内存限制和压力情况

### 测试特性
- **多后端支持**: 所有测试都在Memory和Sled后端上运行
- **异步测试**: 使用tokio进行异步操作测试
- **超时保护**: 防止长时间运行的测试阻塞
- **资源清理**: 自动清理测试数据和临时文件
- **性能基准**: 包含基本的性能测量和比较

## 未来增强

- **Redis后端**: 添加Redis支持以进行分布式缓存
- **压缩**: 大数据的可选值压缩
- **加密**: 敏感数据的静态加密
- **指标**: 内置性能和使用指标
- **备份/恢复**: 自动备份和恢复功能