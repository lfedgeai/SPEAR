# spear-next 项目转换为纯库项目

## 概述

将 `spear-next` 项目从混合项目（库 + 默认二进制文件）转换为纯库项目，移除不需要的 `src/main.rs` 文件。每个需要的组件将有自己独立的 `main.rs` 文件。

## 执行的更改

### 1. 删除文件
- **删除**: `src/main.rs`
  - 原因：不需要默认的二进制入口点
  - 影响：移除了简单的 "Hello, world!" 程序

### 2. 修改 Cargo.toml
**文件**: `spear-next/Cargo.toml`

**添加的配置**:
```toml
[lib]
name = "spear_next"
path = "src/lib.rs"
```

**说明**:
- 明确指定项目为库项目
- 设置库入口点为 `src/lib.rs`
- 保留现有的 `sms` 二进制目标配置

### 3. 更新 tarpaulin.toml
**文件**: `spear-next/tarpaulin.toml`

**修改前**:
```toml
exclude = [
    # Main entry points / 主入口点
    "src/main.rs",
    "src/bin/*",
]
```

**修改后**:
```toml
exclude = [
    # Binary entry points / 二进制入口点
    "src/bin/*",
]
```

**说明**:
- 移除对已删除的 `src/main.rs` 的引用
- 更新注释以反映当前配置

## 项目结构变更

### 修改前
```
spear-next/
├── src/
│   ├── main.rs          # 默认二进制入口（已删除）
│   ├── lib.rs           # 库入口
│   └── bin/
│       ├── sms/main.rs  # SMS 服务二进制
│       └── spearlet/main.rs # Spearlet 二进制
└── Cargo.toml
```

### 修改后
```
spear-next/
├── src/
│   ├── lib.rs           # 库入口（主要入口点）
│   └── bin/
│       ├── sms/main.rs  # SMS 服务二进制
│       └── spearlet/main.rs # Spearlet 二进制
└── Cargo.toml
```

## 构建验证

### 库构建
```bash
cargo check --lib
# 成功：生成 20 个警告（未使用的代码），但构建成功
```

### 二进制构建
```bash
cargo check --bin sms
# 成功：SMS 二进制文件构建正常
```

## 影响分析

### 正面影响
1. **项目结构清晰**: 明确定义为库项目
2. **组件独立**: 每个组件有自己的二进制入口点
3. **配置一致**: 所有配置文件都反映当前项目结构

### 注意事项
1. **现有二进制文件**: `sms` 和 `spearlet` 二进制文件保持不变
2. **库功能**: 项目仍可作为库被其他项目引用
3. **构建警告**: 存在未使用的代码警告，可通过 `cargo fix` 修复

## 相关文件
- `spear-next/Cargo.toml` - 项目配置
- `spear-next/tarpaulin.toml` - 代码覆盖率配置
- `spear-next/src/lib.rs` - 库入口点
- `spear-next/src/bin/sms/main.rs` - SMS 服务入口
- `spear-next/src/bin/spearlet/main.rs` - Spearlet 入口

## 后续建议
1. 运行 `cargo fix --lib -p spear-next` 修复未使用的代码警告
2. 根据需要添加新的组件二进制文件到 `src/bin/` 目录
3. 更新项目文档以反映新的项目结构