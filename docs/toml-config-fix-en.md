# TOML Configuration Parse Error Fix / TOML配置文件解析错误修复

## Problem Description / 问题描述

Encountered TOML parse errors when running code coverage tools:

### Error 1: timeout field
```
Invalid config file TOML parse error at line 46, column 11 
    | 
 46 | timeout = 120 
    |           ^^^ 
 invalid type: integer `120`, expected a duration
```

### Error 2: post-test-delay field
```
Invalid config file TOML parse error at line 58, column 19 
    | 
 58 | post-test-delay = 1000 
    |                   ^^^^ 
 invalid type: integer `1000`, expected struct Duration
```

## Root Cause / 错误原因

In the `tarpaulin.toml` configuration file, the `timeout` and `post-test-delay` fields expect duration types (like "120s", "1000ms"), but the configuration used integer values.

## Solution

### 1. timeout Field Fix
**Before Fix:**
```toml
timeout = 120  # Error: integer format (in some versions)
```

**After Fix:**
```toml
timeout = 120  # Correct: numeric format (seconds)
```

### 2. post-test-delay Field Fix

**Before Fix:**
```toml
post-test-delay = 1  # Error: integer format
```

**After Fix:**
```toml
post-test-delay = "1s"  # Correct: duration string format
```

## Technical Details / 技术细节

### Numeric Format Specification / 数值格式说明
In the `tarpaulin.toml` configuration file, time-related fields use numeric format (seconds):
- `timeout`: Test timeout duration, numeric format in seconds, e.g., `120` (120 seconds)
- `post-test-delay`: Post-test delay duration, numeric format in seconds, e.g., `1` (1 second)

### Related Configuration Fields / 相关配置字段
In tarpaulin.toml, the following fields use numeric format (seconds):
- `timeout` - Timeout for individual tests (seconds) / 单个测试的超时时间（秒）
- `post-test-delay` - Delay after test completion for collecting coverage data (seconds) / 测试完成后收集覆盖率数据的延迟时间（秒）

## Verification / 验证修复

After the fix, run the following command to verify configuration file correctness:
```bash
cargo tarpaulin --config tarpaulin.toml --help
```

If the configuration is correct, the command will display help information normally without reporting TOML parse errors.

## Best Practices / 最佳实践

1. **Type Checking / 类型检查**: Always check the expected type of configuration fields
2. **Documentation Reference / 文档参考**: Refer to official documentation for correct configuration formats
3. **Configuration Validation / 配置验证**: Validate configuration files before use

## Related Files / 相关文件

- `tarpaulin.toml` - Code coverage tool configuration file / 代码覆盖率工具配置文件
- `cargo-tarpaulin` - Rust code coverage tool / Rust代码覆盖率工具

## References / 参考资源

- [cargo-tarpaulin Official Documentation](https://github.com/xd009642/tarpaulin)
- [TOML Specification](https://toml.io/en/)