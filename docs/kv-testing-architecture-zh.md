# KV存储测试架构

## 概述

本文档描述了spear-next项目中KV存储系统的全面测试架构。测试套件设计用于验证不同存储后端的功能性、性能和可靠性。

## 测试架构

### 测试层次结构

```
KV存储测试
├── 单元测试 (src/storage/kv.rs)
│   ├── 基本CRUD操作
│   ├── 序列化功能
│   └── 错误处理
├── 集成测试 (tests/kv_storage_integration_tests.rs)
│   ├── 跨后端兼容性
│   ├── 性能比较
│   ├── 并发操作
│   └── 资源管理
└── 边界条件测试 (tests/kv_storage_edge_cases.rs)
    ├── 特殊键值处理
    ├── 大数据处理
    ├── 内存压力测试
    └── 错误恢复
```

### 测试后端

所有测试都在以下后端上运行：
- **MemoryKvStore**: 内存存储，用于快速测试和开发
- **SledKvStore**: 持久化存储，用于生产环境验证

## 集成测试详解

### 文件: `tests/kv_storage_integration_tests.rs`

#### 测试用例

1. **跨后端兼容性测试** (`test_cross_backend_compatibility`)
   - 验证Memory和Sled后端的API一致性
   - 测试相同操作在不同后端的行为
   - 确保数据格式兼容性

2. **性能比较测试** (`test_performance_comparison`)
   - 比较不同后端的读写性能
   - 测量操作延迟和吞吐量
   - 生成性能报告

3. **大数据处理测试** (`test_large_data_handling`)
   - 测试大键值对的存储和检索
   - 验证内存使用效率
   - 测试超时处理

4. **范围操作测试** (`test_range_operations_comprehensive`)
   - 测试前缀扫描功能
   - 验证范围查询准确性
   - 测试排序和分页

5. **并发操作测试** (`test_concurrent_operations`)
   - 多线程环境下的数据一致性
   - 并发读写操作测试
   - 竞态条件检测

6. **资源清理测试** (`test_cleanup_and_resource_management`)
   - 验证资源正确释放
   - 测试临时文件清理
   - 内存泄漏检测

7. **错误处理测试** (`test_error_handling_and_edge_cases`)
   - 各种错误场景的处理
   - 异常恢复机制
   - 错误信息准确性

8. **工厂配置验证** (`test_factory_configuration_validation`)
   - 存储工厂配置测试
   - 参数验证
   - 默认值处理

## 边界条件测试详解

### 文件: `tests/kv_storage_edge_cases.rs`

#### 测试用例

1. **空键和空白键测试** (`test_empty_and_whitespace_keys`)
   - 空字符串键处理
   - 空白字符键处理
   - 特殊字符键验证

2. **问题值测试** (`test_problematic_values`)
   - Unicode字符处理
   - JSON数据存储
   - 二进制数据处理
   - 特殊字符转义

3. **大键值测试** (`test_very_large_keys_and_values`)
   - 极大键（1MB）处理
   - 极大值（100MB）处理
   - 内存限制测试
   - 超时保护

4. **并发同键访问** (`test_concurrent_same_key_access`)
   - 同一键的并发读写
   - 数据竞争检测
   - 一致性验证

5. **快速创建删除** (`test_rapid_key_creation_deletion`)
   - 高频率操作测试
   - 性能压力测试
   - 资源回收验证

6. **扫描操作边界** (`test_scan_operations_edge_cases`)
   - 空结果集处理
   - 大结果集处理
   - 前缀边界情况

7. **内存压力测试** (`test_memory_pressure_and_limits`)
   - 大量小键值对
   - 内存使用监控
   - 垃圾回收测试

## 测试工具和辅助函数

### 测试配置生成
```rust
pub fn create_test_configs() -> Vec<(&'static str, KvStoreConfig, Option<TempDir>)>
```
- 生成Memory和Sled后端的测试配置
- 自动创建临时目录
- 配置清理机制

### 问题数据生成
```rust
pub fn generate_problematic_keys() -> Vec<String>
pub fn generate_problematic_values() -> Vec<String>
```
- 生成各种边界情况的测试数据
- 包含特殊字符、Unicode、空值等
- 用于压力测试和边界验证

## 测试执行

### 运行所有测试
```bash
# 运行集成测试
cargo test --test kv_storage_integration_tests --features sled

# 运行边界条件测试
cargo test --test kv_storage_edge_cases --features sled

# 运行所有KV相关测试
cargo test kv --features sled
```

### 测试特性

1. **异步测试**: 使用tokio运行时进行异步操作测试
2. **超时保护**: 防止长时间运行的测试阻塞CI/CD
3. **资源清理**: 自动清理测试数据和临时文件
4. **多后端验证**: 确保所有后端的一致行为
5. **性能基准**: 基本的性能测量和比较

## 测试覆盖率

### 功能覆盖
- ✅ 基本CRUD操作
- ✅ 批量操作
- ✅ 范围查询
- ✅ 前缀扫描
- ✅ 并发操作
- ✅ 错误处理
- ✅ 资源管理

### 场景覆盖
- ✅ 正常使用场景
- ✅ 边界条件
- ✅ 错误场景
- ✅ 性能压力
- ✅ 并发竞争
- ✅ 资源限制

## 持续集成

测试套件设计用于在CI/CD环境中运行：
- 快速反馈（大部分测试在秒级完成）
- 可靠性（稳定的测试结果）
- 可维护性（清晰的测试结构和文档）

## 未来改进

1. **性能基准测试**: 更详细的性能分析和回归检测
2. **模糊测试**: 自动生成测试数据进行模糊测试
3. **负载测试**: 长时间运行的负载和稳定性测试
4. **兼容性测试**: 不同版本间的数据兼容性测试