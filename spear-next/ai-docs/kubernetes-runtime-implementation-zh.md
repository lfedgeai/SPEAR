# Kubernetes 运行时实现

## 概述

本文档描述了 Spear 执行引擎的 Kubernetes 运行时实现。Kubernetes 运行时使任务能够作为 Kubernetes Jobs 执行，提供可扩展的容器化执行能力。

## 架构

### 核心组件

1. **KubernetesRuntime**: 管理 Kubernetes Jobs 的主要运行时实现
2. **KubernetesConfig**: 运行时的配置结构
3. **KubernetesJobHandle**: 跟踪运行中 Kubernetes 作业的句柄结构

### 主要特性

- **作业管理**: 创建、启动、停止和清理 Kubernetes Jobs
- **Pod 监控**: 跟踪 pod 状态和健康状况
- **资源管理**: 配置 CPU、内存和其他资源限制
- **命名空间支持**: 在指定的 Kubernetes 命名空间中部署作业
- **错误处理**: 针对 Kubernetes 操作的全面错误处理

## 实现细节

### 配置

`KubernetesConfig` 结构支持：
- `namespace`: 目标 Kubernetes 命名空间（默认："default"）
- `kubeconfig_path`: kubeconfig 文件路径（可选，如未指定则使用集群内配置）
- `job_timeout_seconds`: 作业执行超时时间（默认：3600 秒）
- `cleanup_policy`: 作业清理策略（"Always"、"OnSuccess"、"OnFailure"）

### 运行时能力

Kubernetes 运行时提供以下能力：
- 扩展支持：是
- 健康检查：是
- 指标收集：是
- 热重载：否
- 持久存储：是
- 网络隔离：是
- 最大并发实例数：100
- 支持的协议：["http", "grpc"]

### 作业生命周期

1. **创建实例**: 从实例配置生成 Kubernetes Job 清单
2. **启动实例**: 将 Job 清单应用到 Kubernetes 集群
3. **监控**: 跟踪作业和 pod 状态
4. **执行**: 向运行中的 pod 发送请求
5. **健康检查**: 验证 pod 健康状态
6. **停止**: 删除 Kubernetes Job
7. **清理**: 移除相关资源

### 错误处理

运行时处理各种错误场景：
- Kubernetes API 错误
- Pod 启动失败
- 网络连接问题
- 资源分配失败
- 超时场景

## 使用示例

```rust
use spear_next::spearlet::execution::runtime::{KubernetesRuntime, KubernetesConfig};

// 创建运行时配置
let config = KubernetesConfig {
    namespace: "spear-tasks".to_string(),
    kubeconfig_path: Some("/path/to/kubeconfig".to_string()),
    job_timeout_seconds: 1800,
    cleanup_policy: "OnSuccess".to_string(),
};

// 初始化运行时
let runtime = KubernetesRuntime::new(&config)?;

// 创建实例配置
let mut runtime_config = HashMap::new();
runtime_config.insert("image".to_string(), 
    serde_json::Value::String("my-task:latest".to_string()));

let instance_config = InstanceConfig {
    runtime_type: RuntimeType::Kubernetes,
    runtime_config,
    environment: HashMap::new(),
    resource_limits: InstanceResourceLimits::default(),
    network_config: NetworkConfig::default(),
    max_concurrent_requests: 10,
    request_timeout_ms: 30000,
};

// 创建并启动实例
let instance = runtime.create_instance(&instance_config).await?;
runtime.start_instance(&instance).await?;
```

## 测试

实现包含全面的测试：
- 配置验证的单元测试
- 作业生命周期的集成测试
- Kubernetes API 交互的模拟测试
- 错误场景测试

## 依赖项

- `k8s-openapi`: Kubernetes API 类型
- `kube`: Kubernetes 客户端库
- `serde`: 序列化/反序列化
- `tokio`: 异步运行时
- `uuid`: 唯一标识符生成

## 未来增强

Kubernetes 运行时的潜在改进：
1. 支持 Kubernetes Deployments
2. 高级调度策略
3. 自定义资源定义（CRDs）
4. 多集群支持
5. 增强的监控和可观测性
6. 基于负载的自动扩展

## 安全考虑

- 使用 RBAC 限制 Kubernetes 权限
- 安全的 kubeconfig 文件访问
- Pod 隔离的网络策略
- 防止资源耗尽的资源配额
- 镜像安全扫描集成