# 代码覆盖率测试指南

## 概述

本文档介绍如何在 SPEAR Next 项目中进行代码覆盖率测试。代码覆盖率是衡量测试质量的重要指标，帮助我们了解代码的测试覆盖情况。

## 工具选择

我们使用 `cargo-tarpaulin` 作为主要的代码覆盖率工具，它是 Rust 生态系统中最流行和功能强大的覆盖率工具。

### 安装 cargo-tarpaulin

```bash
cargo install cargo-tarpaulin
```

## 配置文件

### tarpaulin.toml

项目根目录下的 `tarpaulin.toml` 文件包含了覆盖率测试的配置：

```toml
[report]
# 输出格式：HTML报告、LCOV格式、JSON格式
out = ["Html", "Lcov", "Json"]

# 输出目录
output-dir = "target/coverage"

# 排除的文件和目录
exclude = [
    "proto/*",           # 生成的protobuf文件
    "tests/*",           # 测试文件本身
    "src/main.rs",       # 主入口点
    "src/bin/*",         # 二进制入口点
    "examples/*",        # 示例代码
    "benches/*",         # 基准测试
]

# 包含的文件和目录
include = [
    "src/config/*",
    "src/services/*",
    "src/sms/*",
    "src/spearlet/*",
    "src/storage/*",
    "src/utils/*",
    "src/lib.rs",
]

# 最小覆盖率阈值
fail-under = 70

# 超时设置（秒）
timeout = 300

# 并行测试线程数
jobs = 4

# 详细输出
verbose = true

# 跳过清理
no-clean = false
```

## 使用方法

### 1. 快速覆盖率测试

使用提供的快速脚本：

```bash
# 运行快速覆盖率测试
./scripts/quick-coverage.sh

# 或使用 Makefile
make coverage-quick
```

### 2. 完整覆盖率测试

运行完整的覆盖率分析（包括所有特性）：

```bash
# 运行完整覆盖率测试
./scripts/coverage.sh

# 或使用 Makefile
make coverage
```

### 3. 手动运行

直接使用 cargo-tarpaulin：

```bash
# 基本覆盖率测试
cargo tarpaulin --config tarpaulin.toml

# 指定特性
cargo tarpaulin --features sled --config tarpaulin.toml

# 所有特性
cargo tarpaulin --all-features --config tarpaulin.toml
```

## 输出格式

### HTML 报告

- 位置：`target/coverage/tarpaulin-report.html`
- 提供详细的可视化覆盖率报告
- 可以查看每个文件的行级覆盖情况

### LCOV 格式

- 位置：`target/coverage/lcov.info`
- 适用于 CI/CD 集成和第三方工具

### JSON 格式

- 位置：`target/coverage/tarpaulin-report.json`
- 机器可读格式，适用于自动化处理

## Makefile 目标

项目提供了以下 Makefile 目标：

```bash
# 快速覆盖率测试（默认特性）
make coverage-quick

# 完整覆盖率测试（所有特性）
make coverage

# 清理覆盖率数据
make clean-coverage
```

## 脚本说明

### scripts/quick-coverage.sh

快速覆盖率测试脚本：
- 检查 cargo-tarpaulin 安装
- 清理旧的覆盖率数据
- 运行默认特性的覆盖率测试
- 生成 HTML 和控制台输出
- 自动打开浏览器查看报告

### scripts/coverage.sh

完整覆盖率测试脚本：
- 支持多种特性组合测试
- 生成详细的覆盖率报告
- 包含错误处理和日志记录
- 生成汇总报告

## 覆盖率目标

### 当前设置

- **最小覆盖率阈值**：70%
- **目标覆盖率**：80%+
- **关键模块覆盖率**：90%+

### 覆盖率分类

1. **核心业务逻辑**：要求 90%+ 覆盖率
   - SMS 服务
   - 配置管理
   - 存储层

2. **工具和辅助模块**：要求 80%+ 覆盖率
   - 工具函数
   - 中间件

3. **集成和接口层**：要求 70%+ 覆盖率
   - HTTP 路由
   - gRPC 服务

## 最佳实践

### 1. 定期运行

- 每次提交前运行快速覆盖率测试
- 每周运行完整覆盖率分析
- CI/CD 流水线中集成覆盖率检查

### 2. 关注质量而非数量

- 重点关注关键业务逻辑的覆盖
- 确保边界条件和错误处理的测试
- 避免为了覆盖率而写无意义的测试

### 3. 分析报告

- 定期查看 HTML 报告，识别未覆盖的代码
- 关注覆盖率趋势变化
- 针对低覆盖率模块制定改进计划

## 故障排除

### 常见问题

1. **编译错误**
   ```bash
   # 确保项目能正常编译
   cargo check
   cargo test
   ```

2. **权限问题**
   ```bash
   # 确保脚本有执行权限
   chmod +x scripts/*.sh
   ```

3. **依赖问题**
   ```bash
   # 重新安装 cargo-tarpaulin
   cargo install --force cargo-tarpaulin
   ```

### 性能优化

- 使用 `--jobs` 参数调整并行度
- 排除不必要的文件和目录
- 使用 `--skip-clean` 跳过清理步骤（调试时）

## 集成到 CI/CD

参考 `.github/workflows/coverage.yml` 文件，了解如何在 GitHub Actions 中集成代码覆盖率测试。

## 相关资源

- [cargo-tarpaulin 官方文档](https://github.com/xd009642/tarpaulin)
- [Rust 测试指南](https://doc.rust-lang.org/book/ch11-00-testing.html)
- [代码覆盖率最佳实践](https://martinfowler.com/bliki/TestCoverage.html)