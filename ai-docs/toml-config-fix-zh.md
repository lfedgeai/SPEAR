# TOML配置文件解析错误修复 / TOML Configuration Parse Error Fix

## 问题描述 / Problem Description

在运行 `cargo-tarpaulin` 进行代码覆盖率分析时，遇到了 TOML 配置解析错误：

### 1. `timeout` 字段错误（最新）
```
TOML parse error at line 46, column 11
|
46 | timeout = 120
|           ^^^
invalid type: integer `120`, expected a duration
```

### 2. `post-test-delay` 字段错误
```
TOML parse error at line 58, column 19
|
58 | post-test-delay = 1000
|                   ^^^^
invalid type: integer `1000`, expected struct Duration
```

### 3. 历史错误记录
之前也遇到过相反的错误，即期望数字格式而非字符串格式，这表明 `tarpaulin` 的配置格式要求可能会随版本变化。

## 错误原因 / Root Cause

在`tarpaulin.toml`配置文件中，`timeout`和`post-test-delay`字段期望的是duration类型（如"120s"、"1000ms"），但配置中使用了整数值。

## 解决方案 / Solution

根据最新的 `tarpaulin` 配置要求，`timeout` 和 `post-test-delay` 字段应为 duration 字符串格式。

### `timeout` 字段修复

**修复前：**
```toml
timeout = 120     # 错误：整数格式
```

**修复后：**
```toml
timeout = "120s"  # 正确：duration 字符串格式
```

### `post-test-delay` 字段修复

**修复前：**
```toml
post-test-delay = 1      # 错误：整数格式
```

**修复后：**
```toml
post-test-delay = "1s"   # 正确：duration 字符串格式
```

### 注意事项
- `timeout` 字段使用秒为单位：`"120s"`
- `post-test-delay` 字段可以使用毫秒：`"1000ms"` 或秒：`"1s"`
- 确保 duration 字符串包含单位（s, ms, m, h 等）

## 技术细节 / Technical Details

### 数值格式说明
在 `tarpaulin.toml` 配置文件中，时间相关字段使用数字格式（秒数）：
- `timeout`: 测试超时时间，数字格式，单位为秒，如 `120`（120秒）
- `post-test-delay`: 测试后延迟时间，数字格式，单位为秒，如 `1`（1秒）

### 相关配置字段
- `timeout`: 单个测试的超时时间（秒）
- `post-test-delay`: 测试完成后的延迟时间（秒），用于收集覆盖率数据

## 验证修复 / Verification

修复后运行以下命令验证配置文件正确性：
```bash
cargo tarpaulin --config tarpaulin.toml --help
```

如果配置正确，命令会正常显示帮助信息而不会报告TOML解析错误。

## 最佳实践 / Best Practices

1. **类型检查** / Type Checking：始终检查配置字段的期望类型
2. **文档参考** / Documentation Reference：参考官方文档了解正确的配置格式
3. **配置验证** / Configuration Validation：在使用前验证配置文件的正确性

## 相关文件 / Related Files

- `tarpaulin.toml` - 代码覆盖率工具配置文件
- `cargo-tarpaulin` - Rust代码覆盖率工具

## 参考资源 / References

- [cargo-tarpaulin官方文档](https://github.com/xd009642/tarpaulin)
- [TOML规范](https://toml.io/en/)