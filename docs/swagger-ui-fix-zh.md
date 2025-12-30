# Swagger UI API 路径修复指南

## 问题描述

在 Swagger UI 中点击 API 端点（如 `nodes`）时出现调用失败的问题。经过诊断发现，问题的根本原因是 OpenAPI 规范中定义的路径与实际的 HTTP 路由不匹配。

## 问题分析

### 原始问题
- **OpenAPI 规范中的路径**: `/nodes`, `/nodes/{uuid}`, `/resources` 等
- **实际的 HTTP 路由**: `/api/v1/nodes`, `/api/v1/nodes/{uuid}`, `/api/v1/resources` 等
- **结果**: Swagger UI 尝试调用错误的路径，导致 404 错误

### 诊断过程
1. 确认服务正常运行，HTTP 网关和 gRPC 服务器都已启动
2. 验证实际的 API 端点 `/api/v1/nodes` 可以正常工作
3. 检查 Swagger UI 页面可以正常访问
4. 发现 OpenAPI 规范中的路径定义缺少 `/api/v1` 前缀

## 修复方案

### 修复的文件
- `src/http/handlers/docs.rs` - OpenAPI 规范定义文件

### 修复的路径
1. **节点管理路径**:
   - `/nodes` → `/api/v1/nodes`
   - `/nodes/{uuid}` → `/api/v1/nodes/{uuid}`
   - `/nodes/{uuid}/heartbeat` → `/api/v1/nodes/{uuid}/heartbeat`

2. **资源管理路径**:
   - `/nodes/{uuid}/resource` → `/api/v1/nodes/{uuid}/resource`
   - `/resources` → `/api/v1/resources`
   - `/nodes/{uuid}/with-resource` → `/api/v1/nodes/{uuid}/with-resource`

3. **任务管理路径**:
   - 新增 `/api/v1/tasks` (POST, GET)
   - 新增 `/api/v1/tasks/{task_id}` (GET, DELETE)

### 修复示例

```rust
// 修复前
"/nodes": {
    "get": {
        "summary": "List all nodes",
        // ...
    }
}

// 修复后
"/api/v1/nodes": {
    "get": {
        "summary": "List all nodes",
        // ...
    }
}
```

## 验证步骤

### 1. 重启服务
```bash
cargo run --bin sms
```

### 2. 验证 OpenAPI 规范
```bash
curl -s http://localhost:8080/api/openapi.json | jq '.paths | keys'
```

### 3. 测试 API 端点
```bash
# 获取节点列表
curl http://localhost:8080/api/v1/nodes

# 注册新节点
curl -X POST http://localhost:8080/api/v1/nodes \
  -H "Content-Type: application/json" \
  -d '{
    "ip_address": "192.168.1.100",
    "port": 8080,
    "region": "us-west-1",
    "zone": "us-west-1a",
    "instance_type": "t3.medium",
    "metadata": {
      "environment": "test",
      "version": "1.0.0"
    }
  }'
```

### 4. 验证 Swagger UI
访问 `http://localhost:8080/swagger-ui/` 并测试各个 API 端点。

## 修复结果

修复完成后：
- ✅ Swagger UI 中的所有 API 端点都可以正常调用
- ✅ OpenAPI 规范与实际路由完全匹配
- ✅ 新增了任务管理相关的 API 定义
- ✅ 所有路径都包含正确的 `/api/v1` 前缀

## 最佳实践

1. **保持一致性**: 确保 OpenAPI 规范中的路径与实际的 HTTP 路由完全匹配
2. **版本控制**: 使用 `/api/v1` 前缀进行 API 版本控制
3. **完整性**: 确保所有可用的 API 端点都在 OpenAPI 规范中有定义
4. **测试验证**: 修改 OpenAPI 规范后，务必重启服务并进行完整测试

## 相关文档

- [API 使用指南 (中文)](./api-usage-guide-zh.md)
- [API 使用指南 (英文)](./api-usage-guide-en.md)
- [gRPC 传输错误故障排除指南](./grpc-transport-error-troubleshooting-zh.md)