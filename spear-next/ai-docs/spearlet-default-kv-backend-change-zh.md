# SPEARlet 默认 KV 后端配置变更

## 概述

本文档记录了将 SPEARlet 的默认 KV 后端配置从 RocksDB 更改为内存存储的变更。

## 变更摘要

**日期**: 2024-01-15  
**组件**: SPEARlet 配置  
**变更类型**: 默认配置更新  

### 变更前
- **默认 KV 后端**: `rocksdb`
- **配置位置**: `src/spearlet/config.rs` - `StorageConfig::default()`

### 变更后
- **默认 KV 后端**: `memory`
- **配置位置**: `src/spearlet/config.rs` - `StorageConfig::default()`

## 变更原因

此变更是为了使默认配置与代码库中观察到的实际使用模式保持一致：

1. **测试环境一致性**: 所有测试用例都已明确使用内存后端
2. **开发便利性**: 内存后端提供更快的启动速度和更容易的调试
3. **简化设置**: 开发期间无需设置持久化存储
4. **资源效率**: 开发和测试时资源需求更低

## 影响分析

### SPEARlet 服务
- ✅ **默认行为变更**: 新的 SPEARlet 实例将默认使用内存存储
- ✅ **向后兼容**: 指定 `rocksdb` 的现有配置将继续工作
- ✅ **CLI 覆盖**: 用户仍可通过 `--storage-backend rocksdb` 指定持久化存储

### SMS 服务
- ✅ **无影响**: SMS 继续使用 `sled` 作为默认数据库后端
- ✅ **独立配置**: SMS 数据库配置保持不变

## 修改的文件

1. **`src/spearlet/config.rs`**
   - 将 `StorageConfig::default()` 的 backend 从 `"rocksdb"` 改为 `"memory"`
   - 添加了解释性注释

2. **`src/spearlet/config_test.rs`**
   - 更新测试期望值以匹配新的默认值
   - 修改了 `test_spearlet_config_default()` 和 `test_storage_config_default()`

## 测试

所有测试均成功通过：
- ✅ SPEARlet 测试: 71 个通过，0 个失败
- ✅ SMS 测试: 101 个通过，0 个失败
- ✅ 配置测试已更新并通过

## 迁移指南

### 对于现有用户
如果您需要持久化存储（RocksDB），有以下几种选择：

1. **命令行**: `spearlet --storage-backend rocksdb`
2. **配置文件**: 在 TOML 配置中设置 `storage.backend = "rocksdb"`
3. **环境变量**: 设置 `STORAGE_BACKEND=rocksdb`

### 对于新用户
- 默认内存存储开箱即用
- 开发时无需额外设置
- 数据不会在重启之间持久化（设计如此）

## 配置选项

SPEARlet 支持多种 KV 后端：
- `memory`: 内存存储（新默认值）
- `rocksdb`: 持久化 RocksDB 存储
- `sled`: 持久化 Sled 存储（如果启用）

## 最佳实践

1. **开发**: 使用默认内存后端进行快速迭代
2. **测试**: 内存后端非常适合单元测试和集成测试
3. **生产**: 考虑使用 `rocksdb` 满足持久化存储需求
4. **配置**: 在生产配置中始终明确指定后端

## 相关文档

- [SPEARlet KV 后端分析](./spearlet-kv-backend-analysis-zh.md)
- [配置指南](./README-zh.md)