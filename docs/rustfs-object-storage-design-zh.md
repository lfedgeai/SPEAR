# RustFS Object Storage 重构设计

## 概述

本文档基于“允许 breaking change、优先清晰优雅架构”的前提，重新定义 `spearlet` 的对象存储设计。

核心判断如下：

- 不再围绕当前 `KV 里存完整对象` 的模型做兼容演进
- 不把 RustFS 伪装成 `KvStore` backend
- 不为了“未来可能需要”引入过多中间层
- 直接把对象系统收敛成一个简洁、可维护、语义清晰的 `metadata + blob` 架构

本设计的目标不是做一个“抽象很多但主干模糊”的体系，而是做一个主干非常清楚的体系：

- `ObjectCatalog` 是对象元数据的权威来源
- `BlobStore` 是对象内容的后端
- `ObjectService` 只负责编排业务生命周期
- `ObjectJanitor` 负责后台清理与失败收敛

相关背景文档：

- [kv-abstraction-zh.md](./kv-abstraction-zh.md)
- [kv-factory-pattern-zh.md](./kv-factory-pattern-zh.md)
- [spearlet-object-service-cleanup-zh.md](./spearlet-object-service-cleanup-zh.md)
- [spearlet-kv-backend-analysis-zh.md](./spearlet-kv-backend-analysis-zh.md)

## 设计原则

本次设计遵循以下原则：

- breaking change 可接受时，应优先修正边界，而不是保留历史包袱
- 抽象数量应尽量少，只保留真正高价值的扩展点
- `list`、`head`、`stats` 默认只能访问 metadata，不读取 blob
- blob backend 的差异必须被限制在 backend 层，不能泄漏到业务层
- 删除、回收、重试必须幂等
- 配置与装配应直接表达系统结构，而不是隐藏在通用 `storage` 概念里

## 当前设计的问题

当前实现的主要问题不是“后端不够多”，而是模型本身不够干净。

### 1. 对象内容与元数据耦合

当前对象服务将以下内容放在同一个序列化结构中：

- `value`
- `metadata`
- `ref_count`
- `pinned`
- 时间戳

这样会导致：

- 读取元数据时容易把大对象内容一起读出
- `list`、统计、清理都被大对象拖慢
- 很难替换成真正的 blob backend

### 2. `KvStore` 语义不适合作为对象后端抽象

`KvStore` 适合键值型存储，不适合直接抽象 RustFS 这样的 blob store。

如果强行把 RustFS 适配成 KV，会带来：

- `scan_prefix`、`range`、`count` 等语义变形
- blob 路径、head、流式读写难以自然表达
- 服务层出现越来越多的 backend 特判

### 3. API 语义不够干净

当前对象 API 有几个问题：

- `GetObjectRequest.include_value` 通过布尔值切换两种行为，语义含混
- `ListObjectsRequest.include_values` 让 `list` 变成潜在的大对象读取接口
- `start_after` 的分页语义不稳定，实际更接近索引分页
- `add_ref/remove_ref` 的命名偏底层，不够贴合对象生命周期语义

### 4. 运行时装配边界不清楚

对象存储在配置层、服务层、后端层之间没有形成稳定边界，导致运行时仍然偏向“直接塞一个内存实现”的方式。

## 目标架构

### 核心结论

最终只保留 3 个核心抽象：

- `ObjectCatalog`
- `BlobStore`
- `ObjectService`

后台清理不再做成泛化框架，而是一个具体组件：

- `ObjectJanitor`

配置装配不再引入单独的 factory 概念，而是使用直接的构建函数：

- `build_object_service(config)`

### 组件关系

```text
Client
  -> gRPC / HTTP
  -> ObjectService
      -> ObjectCatalog
      -> BlobStore

Background Worker
  -> ObjectJanitor
      -> ObjectCatalog
      -> BlobStore
```

### 职责划分

- `ObjectCatalog`
  - 管理对象元数据
  - 提供对象查询、列表、状态更新能力
  - 作为对象是否存在、是否可见的权威来源
- `BlobStore`
  - 管理对象内容的写入、读取、删除、head
  - 屏蔽 RustFS 等底层 blob backend 差异
- `ObjectService`
  - 编排写入、读取、删除、保留、释放、pin、unpin 等业务规则
  - 不直接了解 KV 实现细节，也不直接拼接 RustFS 路径
- `ObjectJanitor`
  - 重试删除失败的对象
  - 清理孤儿 blob
  - 收敛临时失败后的残留状态

## 核心抽象

### `BlobStore`

`BlobStore` 是对象内容存储的唯一抽象。

建议接口：

```rust
#[async_trait]
pub trait BlobStore: Send + Sync + Debug {
    async fn put(&self, req: PutBlobRequest) -> Result<BlobInfo, ObjectError>;
    async fn get(&self, blob_id: &str) -> Result<BlobBytes, ObjectError>;
    async fn head(&self, blob_id: &str) -> Result<BlobInfo, ObjectError>;
    async fn delete(&self, blob_id: &str) -> Result<(), ObjectError>;
}
```

设计要求：

- `put` 返回 `blob_id`、`size`、`checksum`
- `head` 只返回 blob 元信息
- `delete` 必须幂等
- 第一阶段 `get` 可以返回完整字节数组，未来再升级 streaming

### `ObjectCatalog`

相比 `ObjectMetadataStore`，`ObjectCatalog` 这个命名更准确，因为它不仅仅是“存储元数据”，还负责对象目录语义。

建议接口：

```rust
#[async_trait]
pub trait ObjectCatalog: Send + Sync + Debug {
    async fn get(&self, key: &str) -> Result<Option<ObjectRecord>, ObjectError>;
    async fn put(&self, record: ObjectRecord) -> Result<(), ObjectError>;
    async fn delete(&self, key: &str) -> Result<bool, ObjectError>;
    async fn list(&self, query: ListQuery) -> Result<ListPage<ObjectRecord>, ObjectError>;
    async fn patch(&self, key: &str, patch: ObjectPatch) -> Result<ObjectRecord, ObjectError>;
}
```

设计要求：

- `list` 只返回 metadata
- `patch` 统一承载 ref/pin/state 等元数据更新
- `ObjectCatalog` 是对象可见性的权威来源
- 内部实现可以基于 `KvStore`，但这种实现细节不暴露给业务层

### `ObjectService`

`ObjectService` 是业务编排层，但不应该成为“大而全服务对象”。

建议定义：

```rust
pub struct ObjectService {
    catalog: Arc<dyn ObjectCatalog>,
    blobs: Arc<dyn BlobStore>,
    policy: ObjectPolicy,
}
```

职责：

- 校验对象写入请求
- 实现 overwrite 语义
- 先查 metadata，再按需读取 blob
- 维护 retain/release 与 pin/unpin 生命周期语义
- 驱动删除流程

### `ObjectJanitor`

不建议一开始抽象成通用 GC framework，直接做成具体后台组件即可。

建议定义：

```rust
pub struct ObjectJanitor {
    catalog: Arc<dyn ObjectCatalog>,
    blobs: Arc<dyn BlobStore>,
    policy: ObjectPolicy,
}
```

职责只保留两项：

- 重试删除 `Deleting` 状态对象
- 清理孤儿 blob

## 数据模型

### `ObjectRecord`

建议将 `ObjectRecord` 定义为系统唯一权威元数据模型。

```rust
pub struct ObjectRecord {
    pub key: String,
    pub blob_id: String,
    pub size: u64,
    pub checksum: String,
    pub created_at: i64,
    pub updated_at: i64,
    pub ref_count: u32,
    pub pinned: bool,
    pub state: ObjectState,
    pub metadata: BTreeMap<String, String>,
}
```

设计说明：

- `ref_count` 使用 `u32`，避免负数状态
- `metadata` 使用 `BTreeMap`，让序列化和测试更稳定
- 不再保留“value 和 metadata 混放”的旧结构

### `ObjectState`

第一版建议只保留 2 个状态：

- `Ready`
- `Deleting`

```rust
pub enum ObjectState {
    Ready,
    Deleting,
}
```

不建议第一版引入 `Pending`，原因如下：

- 当前没有明确业务需要暴露“上传中对象”
- `Pending` 会引入额外的可见性与恢复复杂度
- 临时写入失败可以通过孤儿 blob 清理解决

### `key` 与 `blob_id`

应明确区分：

- `key`
  - 业务逻辑键
  - 对外 API 使用
- `blob_id`
  - 底层 blob 对象标识
  - 用于 RustFS 路径映射

建议布局示例：

```text
key: workflow/123/output.json
blob_id: objects/ab/cd/550e8400-e29b-41d4-a716-446655440000
```

这使得覆盖写、旧版本延迟删除、未来版本化都更自然。

## API 设计

既然允许 breaking change，建议直接清理历史 API 里的歧义。

### 建议删除的旧语义

- 删除 `ListObjects.include_values`
- 删除 `GetObject.include_value`
- 删除基于索引语义的 `start_after`
- 删除 `add_ref/remove_ref` 这种偏底层命名

### 建议的新 API 语义

- `PutObject`
  - 写入或覆盖对象
- `HeadObject`
  - 只返回对象元信息
- `GetObject`
  - 返回对象内容
- `ListObjects`
  - 只返回对象摘要
- `DeleteObject`
  - 删除对象
- `RetainObject`
  - 增加引用
- `ReleaseObject`
  - 减少引用
- `PinObject`
  - 固定对象
- `UnpinObject`
  - 取消固定

### 为什么要拆 `HeadObject` 和 `GetObject`

相比布尔开关 `include_value`，拆成两个接口更优雅：

- 协议语义更清晰
- 实现更简单
- HTTP 映射更自然
- 不会在一个接口里隐藏两条行为路径

### `ListObjects` 只返回摘要

`list` 不应成为读取大对象的接口。

因此：

- `ListObjects` 只返回 `ObjectSummary`
- 不支持返回 value
- 若未来需要批量内容拉取，应单独设计 API

### 分页语义

建议直接改为 token 分页：

- 请求：`prefix`、`limit`、`page_token`
- 响应：`items`、`next_page_token`

不再保留旧的 `start_after` 兼容语义。

## 请求流程

### Put Object

建议流程：

1. 校验 `key`、`size`、`metadata`
2. 若对象存在且 `overwrite=false`，返回冲突
3. 生成新的 `blob_id`
4. 先写入 `BlobStore`
5. 再写入 `ObjectCatalog`
6. 若 catalog 写入失败，执行 best-effort blob 删除
7. 若回滚失败，交给 `ObjectJanitor` 清理

原因：

- metadata 是对象可见性的最终开关
- 这样更容易保证读取只看到完整对象

### Head Object

建议流程：

1. 从 `ObjectCatalog` 查询 `ObjectRecord`
2. 若不存在，返回 not found
3. 仅返回对象摘要，不读取 blob

### Get Object

建议流程：

1. 从 `ObjectCatalog` 查询 `ObjectRecord`
2. 若不存在，返回 not found
3. 检查对象状态是否为 `Ready`
4. 通过 `blob_id` 从 `BlobStore` 读取内容
5. 返回内容与必要元信息

### Delete Object

建议流程：

1. 从 `ObjectCatalog` 查询对象
2. 若 `ref_count > 0` 或 `pinned=true`，拒绝删除
3. 将对象状态修改为 `Deleting`
4. 删除 blob
5. 删除 catalog 记录
6. 若 blob 删除失败，保持 `Deleting` 状态并由 `ObjectJanitor` 重试

### Retain / Release

对象生命周期接口建议改名为：

- `RetainObject`
- `ReleaseObject`

比 `AddObjectRef` / `RemoveObjectRef` 更符合对象系统语义。

规则如下：

- `RetainObject`
  - `ref_count += 1`
- `ReleaseObject`
  - `ref_count -= 1`
  - 当 `ref_count == 0 && !pinned` 时，可触发删除流程

## 一致性与失败恢复

### 一致性原则

本设计不引入复杂分布式事务，而采用简单、可恢复的最终一致模型：

- metadata 是对象可见性的唯一依据
- blob 删除失败可以重试
- 对象删除必须幂等
- 写入失败必须可回滚

### 孤儿 blob

孤儿 blob 主要来自两类场景：

- blob 已写入，但 catalog 写入失败
- overwrite 成功后，旧 blob 延迟删除失败

处理策略：

- 请求路径先做 best-effort cleanup
- 后台由 `ObjectJanitor` 周期性清理

### `Deleting` 状态

`Deleting` 是唯一需要保留的中间状态，用途非常明确：

- 对象已进入删除流程
- 不应再被当作正常对象暴露
- 后台可以安全重试删除

## 配置设计

既然接受 breaking change，建议直接用新的对象存储配置模型，不再沿用混杂的 `storage` 语义。

建议配置：

```rust
pub struct ObjectStorageConfig {
    pub max_object_size: u64,
    pub metadata_backend: MetadataBackendConfig,
    pub blob_backend: BlobBackendConfig,
}
```

其中：

- `MetadataBackendConfig`
  - 描述 catalog 后端
- `BlobBackendConfig`
  - 描述 blob 后端

建议 TOML：

```toml
[spearlet.object_storage]
max_object_size = 67108864

[spearlet.object_storage.metadata_backend]
backend = "rocksdb"
data_dir = "./data/spearlet/meta"
max_cache_size_mb = 512
compression_enabled = true

[spearlet.object_storage.blob_backend]
backend = "rustfs"
endpoint = "http://127.0.0.1:9000"
bucket = "spear-objects"
root = "prod/node-a"
connect_timeout_ms = 3000
read_timeout_ms = 10000
write_timeout_ms = 10000
create_bucket_if_missing = true
```

## RustFS 接入设计

### `RustFsBlobStore`

建议提供一个明确的 `RustFsBlobStore` 实现 `BlobStore`。

职责：

- 初始化 RustFS client
- 管理 bucket / root 配置
- 实现 `put/get/head/delete`
- 将底层错误映射为统一 `ObjectError`

要求：

- 请求必须带超时
- 删除必须幂等
- not found、暂时性错误、永久性错误要清楚区分

### 路径布局

不建议把路径策略做成大而全抽象。

如果只是路径生成，使用一个小型具体组件即可，例如：

- `BlobPathLayout`

职责：

- 根据 `blob_id` 生成 RustFS path
- 统一 shard 布局
- 不让路径拼接逻辑散落到服务层

## 模块组织建议

建议将对象系统收敛在一个小模块树中，不要拆得过碎。

推荐结构：

- `src/spearlet/object/mod.rs`
- `src/spearlet/object/model.rs`
- `src/spearlet/object/service.rs`
- `src/spearlet/object/catalog.rs`
- `src/spearlet/object/blob.rs`
- `src/spearlet/object/janitor.rs`
- `src/spearlet/object/catalog_kv.rs`
- `src/spearlet/object/blob_rustfs.rs`

建议说明：

- `catalog.rs`
  - trait 与查询/patch 模型
- `blob.rs`
  - trait 与 blob 请求/响应模型
- `service.rs`
  - 业务编排
- `janitor.rs`
  - 后台重试和清理
- `catalog_kv.rs`
  - 基于 `KvStore` 的 catalog 实现
- `blob_rustfs.rs`
  - RustFS backend 实现

## 装配方式

不建议引入独立 `ObjectStoreFactory` 抽象。

如果系统只在启动时装配一次，用一个清晰的构建函数就足够：

```rust
pub async fn build_object_service(
    config: &ObjectStorageConfig,
) -> Result<ObjectService, ObjectError>
```

这样更简单，且不会为了抽象而抽象。

## 迁移建议

### Phase 1: 先定义新模型

- 新增 `ObjectRecord`
- 新增 `ObjectState`
- 新增 `ObjectCatalog`
- 新增 `BlobStore`

### Phase 2: 重做 API 语义

- 拆分 `HeadObject` 与 `GetObject`
- 让 `ListObjects` 只返回摘要
- 将 `AddObjectRef/RemoveObjectRef` 改为 `RetainObject/ReleaseObject`
- 引入 `page_token`

### Phase 3: 落地 catalog + blob

- 实现 `KvObjectCatalog`
- 先引入 `BlobStore` 抽象与 `LocalFsBlobStore` 兼容实现
- 再将 `LocalFsBlobStore` 替换为 `RustFsBlobStore`
- 将 `ObjectService` 改为双后端注入

### Phase 4: 引入后台清理

- 实现 `ObjectJanitor`
- 支持 `Deleting` 对象重试删除
- 清理孤儿 blob

### Phase 5: 删除旧模型

- 删除 `StoredObject.value` 存储模式
- 删除旧对象 API
- 删除旧 `storage` 配置映射

## 当前实现进展

截至当前代码版本，SMS 已经完成：

- `BlobStore` trait 抽象
- `LocalFsBlobStore` 作为当前本地兼容实现
- 基于 S3-compatible client 的 `RustFsBlobStore`
- objects handler 改为通过 `BlobStore` 读写对象内容与 sidecar 元数据
- runtime 已经把共享 `BlobStore` 注入到 gateway 与 web admin state 中
- 新增了结构化 `object_storage` 配置，包含 `local_fs` 与 `rustfs` 配置段
- 新增 `sms-rustfs` Cargo feature，使 RustFS/AWS 依赖只编入 SMS 定向构建；在 `sms`、`spearlet`、`debug-server` 仍共享单 crate 的前提下，它刻意不是默认 feature
- 本地 Docker Compose 路径改为使用 debug profile 容器构建，以控制启用 RustFS 后的本地启动成本

这意味着后续 RustFS 相关工作应主要聚焦在部署加固、凭证约定和真实 RustFS 集群联调验证，而不是继续把文件系统细节散落在 handler 中。

## 测试策略

建议重点围绕“语义正确性”测试，而不是围绕旧实现兼容性测试。

### 单元测试

- `ObjectService`
  - put/head/get/list/delete/retain/release/pin/unpin 规则
- `KvObjectCatalog`
  - catalog 编解码、分页、patch 逻辑
- `RustFsBlobStore`
  - 错误映射、超时、幂等删除

### 集成测试

- 配置装配
  - 是否正确创建 catalog 和 blob backend
- gRPC / HTTP API
  - 是否符合新协议语义
- RustFS 联调
  - blob 写入成功但 catalog 写入失败时的恢复行为

### 故障测试

- blob 写入成功但 catalog 写入失败
- 对象进入 `Deleting` 后 blob 删除失败
- `HeadObject` 不触发 blob 读取
- `ListObjects` 永不读取 blob 内容

## 可观测性要求

建议保留以下关键指标：

- `object_put_total`
- `object_head_total`
- `object_get_total`
- `object_delete_total`
- `object_blob_delete_failures_total`
- `object_catalog_write_failures_total`
- `object_janitor_retry_total`
- `object_orphan_blob_cleanup_total`

建议日志字段：

- `key`
- `blob_id`
- `size`
- `operation`
- `backend`
- `latency_ms`
- `result`

## 风险与权衡

### 好处

- 主干更清晰
- RustFS 接入更自然
- API 语义更干净
- 后续支持其他 blob backend 成本更低
- 代码组织更符合 Rust 模块化风格

### 成本

- breaking change 需要同步修改 proto、HTTP API 与调用方
- 旧对象模型与旧配置需要一次性清理
- 需要引入后台 janitor 收敛失败状态

### 判断

这些成本是合理的，且远小于继续维持旧模型所带来的长期复杂度。

## 结论

推荐将 `spearlet` 对象系统重构为：

- `ObjectCatalog` 负责 metadata
- `BlobStore` 负责 blob
- `ObjectService` 负责编排生命周期
- `ObjectJanitor` 负责后台清理
- `RustFsBlobStore` 作为 RustFS 接入点

相比“继续围绕当前对象服务做兼容式扩展”，该方案更清晰、更优雅，也更符合长期可维护的 best practice。

如果后续进入实现阶段，建议优先完成：

1. 新数据模型与新 API 定义
2. `KvObjectCatalog` 与 `RustFsBlobStore`
3. `ObjectService` 与 `ObjectJanitor`
4. 删除旧协议与旧存储模型
