# Spearlet HTTP Gateway 启动问题修复

## 问题概述

spearlet 的 HTTP gateway 由于竞态条件导致启动失败，HTTP gateway 在 gRPC 服务器完全初始化之前就尝试连接，导致连接失败。

## 根本原因分析

1. **竞态条件**: HTTP gateway 和 gRPC 服务器在 main.rs 文件中并发启动
2. **连接失败**: HTTP gateway 立即尝试连接 gRPC 服务器，没有等待或重试机制
3. **传输错误**: 这导致了"transport error"并使 HTTP gateway 终止

## 实施的解决方案

### 1. 启动顺序修改
- **文件**: `src/bin/spearlet/main.rs`
- **变更**: 在 gRPC 服务器启动和 HTTP gateway 启动之间添加 500ms 延迟
- **目的**: 确保 gRPC 服务器有足够时间初始化

```rust
// Start gRPC server / 启动gRPC服务器
let grpc_handle = tokio::spawn(async move {
    if let Err(e) = grpc_server.start().await {
        error!("gRPC server error: {}", e);
    }
});

// Wait for gRPC server to start / 等待gRPC服务器启动
tokio::time::sleep(std::time::Duration::from_millis(500)).await;
```

### 2. HTTP Gateway 重试机制
- **文件**: `src/spearlet/http_gateway.rs`
- **变更**: 为 gRPC 连接添加重试逻辑
- **特性**:
  - 最多 5 次重试尝试
  - 重试间隔 1 秒
  - 详细的连接尝试日志
  - 优雅的错误处理

```rust
let mut grpc_client = None;
let max_retries = 5;
let mut retry_count = 0;

while retry_count < max_retries {
    match ObjectServiceClient::connect(grpc_endpoint.clone()).await {
        Ok(client) => {
            info!("Successfully connected to gRPC server");
            grpc_client = Some(client);
            break;
        }
        Err(e) => {
            retry_count += 1;
            if retry_count >= max_retries {
                error!("Failed to connect to gRPC server after {} retries: {}", max_retries, e);
                return Err(e.into());
            }
            info!("Failed to connect to gRPC server (attempt {}/{}): {}, retrying in 1s...", 
                  retry_count, max_retries, e);
            tokio::time::sleep(std::time::Duration::from_secs(1)).await;
        }
    }
}
```

## 测试结果

### 修复前
```
2025-09-14T06:41:47.508Z ERROR spear_next::spearlet::http_gateway: transport error
```

### 修复后
```
2025-09-14T06:43:25.010Z INFO spear_next::spearlet::http_gateway: Starting HTTP gateway on 0.0.0.0:8081
2025-09-14T06:43:25.010Z INFO spear_next::spearlet::http_gateway: Connecting to gRPC server at http://0.0.0.0:50052
2025-09-14T06:43:25.012Z INFO spear_next::spearlet::http_gateway: Successfully connected to gRPC server
2025-09-14T06:43:25.012Z INFO spear_next::spearlet::http_gateway: HTTP gateway listening on 0.0.0.0:8081
```

### HTTP Gateway 可访问性测试
```bash
$ curl -v http://localhost:8081/health
< HTTP/1.1 200 OK
< content-type: application/json
< content-length: 88
{"service":"spearlet","status":"healthy","timestamp":"2025-09-14T06:43:37.613862+00:00"}
```

## 影响

- ✅ HTTP gateway 现在可以成功启动
- ✅ URL `http://localhost:8081` 可以正常访问
- ✅ 健康检查端点正确响应
- ✅ 对现有功能无破坏性变更
- ✅ 改进了错误处理和日志记录

## 修改的文件

1. `src/bin/spearlet/main.rs` - 添加启动延迟
2. `src/spearlet/http_gateway.rs` - 添加重试机制

## 应用的最佳实践

1. **优雅启动**: 正确的服务初始化顺序
2. **重试逻辑**: 健壮的连接处理
3. **全面日志**: 更好的调试信息
4. **错误处理**: 正确的错误传播和报告
5. **双语注释**: 代码中的中英文文档

## 使用指南

修复后，spearlet 的 HTTP gateway 将在以下地址可用：
- 默认地址: `http://0.0.0.0:8081`
- 本地访问: `http://localhost:8081`
- 健康检查: `http://localhost:8081/health`

用户现在可以正常访问 spearlet 打印的 HTTP gateway URL。