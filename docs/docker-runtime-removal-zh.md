# Docker 运行时移除文档

## 概述

本文档描述了从 Spear 执行系统中完全移除 Docker 运行时支持的过程。Docker 运行时已被 Kubernetes 运行时替代，作为主要的容器编排解决方案。

## 所做的更改

### 1. 运行时类型枚举更新
- 从 `src/spearlet/execution/runtime/mod.rs` 中的枚举移除了 `RuntimeType::Docker`
- 更新所有引用以使用 `RuntimeType::Kubernetes` 替代

### 2. 运行时工厂更改
- 从 `RuntimeFactory::create_runtime()` 中移除了 Docker 运行时创建逻辑
- 从 `available_runtimes()` 方法中移除了 Docker
- 更新工厂以仅支持 Process、WASM 和 Kubernetes 运行时

### 3. 模块结构清理
- 从 `src/spearlet/execution/runtime/mod.rs` 中移除了 `docker` 模块声明
- 移除了 `DockerRuntime` 和 `DockerConfig` 导出
- 删除了整个 `src/spearlet/execution/runtime/docker.rs` 文件

### 4. 字符串映射更新
更新了以下文件中的字符串到运行时类型的映射：
- `src/spearlet/function_service.rs`: 将 "docker" → `RuntimeType::Kubernetes`
- `src/spearlet/execution/artifact.rs`: 将 "docker" → `RuntimeType::Kubernetes`
- 添加了 "kubernetes" → `RuntimeType::Kubernetes` 映射

### 5. 测试代码更新
更新了多个文件中的所有测试用例：
- `src/spearlet/execution/runtime/mod.rs`
- `src/spearlet/execution/artifact.rs`
- `src/spearlet/execution/instance.rs`
- `src/spearlet/execution/task.rs`
- `src/spearlet/execution/runtime/process.rs`
- `src/spearlet/execution/runtime/wasm.rs`

所有之前使用 `RuntimeType::Docker` 的测试实例现在根据情况使用 `RuntimeType::Kubernetes` 或 `RuntimeType::Process`。

### 6. 文档更新
- 更新了 `src/spearlet/execution/mod.rs` 中的模块文档
- 将运行时支持描述从 "Docker、Process 和 WASM" 更改为 "Kubernetes、Process 和 WASM"

## 移除后支持的运行时

系统现在支持三种运行时类型：

1. **Kubernetes 运行时**: 使用 Kubernetes 的容器编排
2. **Process 运行时**: 直接进程执行
3. **WASM 运行时**: WebAssembly 模块执行

## 迁移指南

### 对于现有配置
- 将任何 `"docker"` 运行时类型字符串替换为 `"kubernetes"`
- 更新任何硬编码的 `RuntimeType::Docker` 引用为 `RuntimeType::Kubernetes`

### 对于新部署
- 对于容器化工作负载使用 `RuntimeType::Kubernetes`
- 对于原生进程执行使用 `RuntimeType::Process`
- 对于 WebAssembly 模块使用 `RuntimeType::Wasm`

## 测试结果

Docker 运行时移除后所有测试都成功通过：
- 44 个执行模块测试通过
- 完整测试套件（227 个测试）通过
- 无编译错误或警告

## 此更改的好处

1. **简化架构**: 通过移除冗余的容器运行时减少了复杂性
2. **更好的编排**: Kubernetes 提供更高级的编排功能
3. **行业标准**: Kubernetes 是容器编排的事实标准
4. **维护性**: 通过专注于更少的运行时类型减少了维护负担

## 未来考虑

- Kubernetes 运行时提供了之前 Docker 运行时提供的所有功能
- 容器工作负载应迁移到使用 Kubernetes 运行时
- 系统架构现在更符合云原生最佳实践