# 临时文件清理指南

## 概述

本文档记录了项目中临时文件的识别和清理过程，以及 `.gitignore` 配置的优化。

## 已清理的临时文件

### 1. 代码覆盖率相关文件

- `build_rs_cov.profraw` - LLVM 覆盖率原始数据文件
- `coverage/tarpaulin-report.html` - Tarpaulin 生成的 HTML 覆盖率报告

### 2. 重要说明：不要删除的文件

⚠️ **`.github/workflows/coverage.yml`** - 这是 GitHub Actions 工作流配置文件，**不是临时文件**，不应删除！

这个文件的作用：
- 自动运行代码覆盖率测试
- 生成覆盖率报告和徽章
- 在 PR 中自动评论覆盖率结果
- 上传覆盖率数据到 Codecov

### 2. 文件特征

| 文件类型 | 扩展名 | 描述 | 可删除 |
|---------|--------|------|--------|
| LLVM 覆盖率数据 | `.profraw`, `.profdata` | 代码覆盖率原始数据 | ✅ |
| 覆盖率报告 | `.html`, `.xml`, `.lcov`, `.json` | 覆盖率报告文件 | ✅ |
| 构建产物 | `target/` | Rust 编译输出 | ✅ |
| 编辑器文件 | `.swp`, `.swo`, `*~` | 编辑器临时文件 | ✅ |
| 系统文件 | `.DS_Store`, `Thumbs.db` | 操作系统生成 | ✅ |

## 更新的 .gitignore 规则

```gitignore
# Coverage files / 覆盖率文件
*.profraw
*.profdata
tarpaulin-report.html
cobertura.xml
lcov.info
coverage.json
coverage-summary/
coverage-report-*/

# Rust build artifacts / Rust构建产物
target/
Cargo.lock

# IDE and editor files / IDE和编辑器文件
*.swp
*.swo
*~
.vscode/
*.code-workspace

# OS generated files / 操作系统生成的文件
Thumbs.db
ehthumbs.db

# Temporary files / 临时文件
*.tmp
*.temp
*.log
*.pid
*.lock
*.cache
```

## 清理建议

### 定期清理

1. **覆盖率文件**：每次运行覆盖率测试后会重新生成
2. **构建产物**：使用 `cargo clean` 清理
3. **临时文件**：可以安全删除，程序会重新创建

### 自动化清理

可以创建清理脚本：

```bash
#!/bin/bash
# 清理临时文件
find . -name "*.profraw" -delete
find . -name "*.profdata" -delete
find . -name "tarpaulin-report.html" -delete
cargo clean
```

## 注意事项

- ⚠️ 不要删除配置文件（如 `tarpaulin.toml`）
- ⚠️ 不要删除源代码和测试文件
- ✅ 临时文件可以安全删除，会在需要时重新生成
- ✅ 构建产物可以通过重新编译恢复

## 相关文档

- [代码覆盖率配置](./code-coverage-zh.md)
- [TOML 配置修复](./toml-config-fix-zh.md)