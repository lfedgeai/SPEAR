# TaskService 优化：移除不必要的 node_uuid 字段

## 概述

本文档描述了通过移除不必要的 `node_uuid` 字段来优化 TaskService 的过程。这个改动简化了服务架构，并符合 SMS（Spear Management Service）本身不是 spearlet 因此不需要 UUID 的原则。

## 背景

在 Task API 重构过程中，发现 `TaskService` 结构体中的 `node_uuid` 字段在代码库中没有被使用。这个字段最初是基于服务可能需要跟踪自己节点身份的假设而包含的，但 SMS 作为管理服务而不是工作节点（spearlet）运行。

## 变更内容

### 1. TaskService 结构简化

**文件**: `src/services/task.rs`

**修改前**:
```rust
#[derive(Debug)]
pub struct TaskService {
    storage: Arc<dyn KvStore>,       // Task metadata storage / 任务元数据存储
    node_uuid: String,               // Current node UUID / 当前节点UUID
}
```

**修改后**:
```rust
#[derive(Debug)]
pub struct TaskService {
    storage: Arc<dyn KvStore>,       // Task metadata storage / 任务元数据存储
}
```

### 2. 构造函数更新

**修改前**:
```rust
pub fn new(storage: Arc<dyn KvStore>, node_uuid: String) -> Self {
    Self {
        storage,
        node_uuid,
    }
}

pub fn new_with_memory(node_uuid: String) -> Self {
    Self::new(Arc::new(MemoryKvStore::new()), node_uuid)
}
```

**修改后**:
```rust
pub fn new(storage: Arc<dyn KvStore>) -> Self {
    Self {
        storage,
    }
}

pub fn new_with_memory() -> Self {
    Self::new(Arc::new(MemoryKvStore::new()))
}
```

### 3. 更新调用点

以下文件被更新以移除 `node_uuid` 参数：

- `src/bin/sms/main.rs` - 主 SMS 服务器初始化
- `src/services/task.rs` - 单元测试
- `tests/task_integration_tests.rs` - 集成测试
- `tests/objectref_integration_tests.rs` - 对象引用集成测试

## 优势

1. **简化架构**: 从 TaskService 中移除了不必要的复杂性
2. **更清晰的语义**: 明确表示 SMS 是管理服务，而不是工作节点
3. **减少内存使用**: 消除了未使用字符串数据的存储
4. **更容易测试**: 简化了测试设置，减少了参数
5. **更好的可维护性**: 减少了需要维护的代码和潜在的混淆点

## 架构清晰度

这个改动强化了以下架构区别：

- **SMS (Spear Management Service)**: 协调任务和资源的集中式管理服务
- **Spearlets**: 执行任务并拥有自己 UUID 用于识别的工作节点

SMS 不需要 UUID 因为：
- 它是单例管理服务
- 它不会将自己注册为工作节点
- 它管理其他节点而不是被管理

## 测试

优化后所有测试继续通过：
- 单元测试：47 个测试通过
- 集成测试：27 个测试通过
- 移除未使用字段没有影响任何功能

## 迁移说明

对于可能创建 TaskService 实例的外部代码：
- 从 `TaskService::new()` 调用中移除 `node_uuid` 参数
- 从 `TaskService::new_with_memory()` 调用中移除 `node_uuid` 参数
- 不需要其他更改，因为该字段没有通过公共 API 暴露

## 代码质量影响

这个改动解决了编译器警告：
```
warning: field `node_uuid` is never read
```

优化通过以下方式提高了代码质量：
- 消除死代码
- 减少开发者的认知负担
- 使服务的目的更清晰
- 遵循"只包含需要的内容"原则