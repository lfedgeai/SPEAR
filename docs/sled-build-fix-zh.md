# Sled 构建错误修复指南

## 问题描述

在使用 `cargo build --features sled` 构建项目时，出现以下编译错误：

```
error[E0425]: cannot find function `create_kv_store` in this scope
  --> src/services/resource.rs:148:25
   |
148|         let kv_store = create_kv_store(KvStoreType::Sled { path: db_path.to_string() })?;
   |                        ^^^^^^^^^^^^^^^ not found in this scope
   |
help: consider importing this function:
   |
1  + use crate::create_kv_store;
   |

error[E0412]: cannot find type `KvStoreType` in this scope
  --> src/services/resource.rs:148:40
   |
148|         let kv_store = create_kv_store(KvStoreType::Sled { path: db_path.to_string() })?;
   |                                        ^^^^^^^^^^^ not found in this scope
   |
help: consider importing this type:
   |
1  + use crate::KvStoreType;
   |
```

## 根本原因分析

错误发生在 `src/services/resource.rs` 文件的第 148 行，在 `new_with_sled` 函数中：

```rust
#[cfg(feature = "sled")]
pub fn new_with_sled(db_path: &str) -> Result<Self, SmsError> {
    let kv_store = create_kv_store(KvStoreType::Sled { path: db_path.to_string() })?;
    Ok(Self::with_kv_store(Arc::new(kv_store)))
}
```

问题是缺少必要的导入语句：
- `create_kv_store` 函数
- `KvStoreType` 枚举

这两个都定义在 `src/storage/kv.rs` 模块中，但没有在 `resource.rs` 中导入。

## 修复方案

### 步骤 1: 添加缺失的导入

在 `src/services/resource.rs` 文件中，修改导入语句：

**修改前：**
```rust
use crate::storage::{KvStore, serialization, MemoryKvStore};
```

**修改后：**
```rust
use crate::storage::{KvStore, serialization, MemoryKvStore, create_kv_store, KvStoreType};
```

### 步骤 2: 验证修复

1. **重新构建项目：**
   ```bash
   cargo build --features sled
   ```

2. **验证构建成功：**
   ```bash
   ✓ Compiling spear-next v0.1.0 (/path/to/spear-next)
   ✓ Finished `dev` profile [unoptimized + debuginfo] target(s) in 8.22s
   ```

3. **测试 Sled 配置运行：**
   ```bash
   cargo run --features sled --bin sms -- --config config-sled.toml
   ```

4. **验证服务启动日志：**
   ```
   INFO sms: KV store backend: sled
   INFO sms: KV store path: ./data/node-service.db
   INFO sms: HTTP gateway listening on 0.0.0.0:8080
   INFO sms: Swagger UI available at: http://0.0.0.0:8080/swagger-ui/
   ```

5. **测试 API 功能：**
   ```bash
   curl -s http://localhost:8080/api/v1/nodes | jq .
   ```

## 技术细节

### 相关模块结构

```
src/
├── storage/
│   └── kv.rs              # 定义 create_kv_store 和 KvStoreType
└── services/
    └── resource.rs        # 使用 KV 存储的资源服务
```

### KvStoreType 枚举定义

```rust
#[derive(Debug, Clone)]
pub enum KvStoreType {
    Memory,
    #[cfg(feature = "sled")]
    Sled { path: String },
    #[cfg(feature = "rocksdb")]
    RocksDb { path: String },
}
```

### create_kv_store 函数签名

```rust
pub fn create_kv_store(store_type: KvStoreType) -> Result<Box<dyn KvStore>, SmsError>
```

## 配置文件示例

`config-sled.toml` 配置文件内容：

```toml
[sms]
grpc_addr = "0.0.0.0:50051"
http_addr = "0.0.0.0:8080"
cleanup_interval = 300
heartbeat_timeout = 120
enable_swagger = true

[sms.kv_store]
backend = "sled"
path = "./data/node-service.db"
cache_capacity = "10000"
flush_every_ms = "5000"
compression = "true"
```

## 最佳实践

1. **特性依赖检查：** 确保在使用特定后端时启用相应的 Cargo 特性
2. **导入完整性：** 在使用跨模块函数时，确保所有必要的导入都已添加
3. **编译验证：** 在不同特性组合下测试构建，确保代码的兼容性
4. **配置验证：** 使用实际配置文件测试服务启动和 API 功能

## 相关文档

- [KV 存储架构文档](./unified-kv-architecture-zh.md)
- [KV 工厂模式文档](./kv-factory-pattern-zh.md)
- [API 使用指南](./api-usage-guide-zh.md)
- [配置文件说明](./config-guide-zh.md)

## 故障排除

如果仍然遇到构建问题：

1. **清理构建缓存：**
   ```bash
   cargo clean
   cargo build --features sled
   ```

2. **检查依赖版本：**
   ```bash
   cargo tree --features sled
   ```

3. **验证特性启用：**
   ```bash
   cargo build --features sled --verbose
   ```

4. **检查 Sled 依赖：**
   确保 `Cargo.toml` 中包含 sled 依赖：
   ```toml
   [dependencies]
   sled = { version = "0.34", optional = true }
   
   [features]
   sled = ["dep:sled"]
   ```