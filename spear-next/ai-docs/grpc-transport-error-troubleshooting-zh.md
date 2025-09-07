# gRPC Transport Error 故障排除指南

## 问题描述

当运行 SPEAR Node Service 时，可能会遇到以下错误：

```
ERROR sms: gRPC server error: transport error
```

## 常见原因和解决方案

### 1. 端口被占用

**症状：**
- 应用程序启动后立即出现 transport error
- 服务无法绑定到指定端口

**诊断方法：**
```bash
# 检查端口 50051 是否被占用
lsof -i :50051

# 检查端口 8080 是否被占用  
lsof -i :8080
```

**解决方案：**
```bash
# 杀掉占用端口的进程
kill <PID>

# 或者修改配置文件使用不同端口
# 在 config.toml 中修改：
[sms]
grpc_addr = "0.0.0.0:50052"  # 使用不同端口
http_addr = "0.0.0.0:8081"   # 使用不同端口
```

### 2. 网络配置问题

**症状：**
- gRPC 服务器启动但无法接受连接
- HTTP 网关无法连接到 gRPC 服务

**解决方案：**
```bash
# 检查防火墙设置
sudo ufw status

# 确保端口未被防火墙阻止
sudo ufw allow 50051
sudo ufw allow 8080
```

### 3. 服务启动顺序问题

**症状：**
- HTTP 网关在 gRPC 服务器完全启动前尝试连接
- 出现连接重试消息

**正常行为：**
```
WARN sms: Failed to connect to Node gRPC service: transport error, retrying in 2s...
INFO sms: Connected to Node gRPC service at http://0.0.0.0:50051
```

这是正常的启动过程，HTTP 网关会自动重试连接。

### 4. 配置错误

**症状：**
- 服务无法解析配置的地址
- 绑定地址格式错误

**检查配置：**
```toml
[sms]
grpc_addr = "0.0.0.0:50051"  # 正确格式
http_addr = "0.0.0.0:8080"   # 正确格式

# 错误示例：
# grpc_addr = "50051"        # 缺少 IP 地址
# grpc_addr = ":50051"       # 格式不完整
```

## 验证服务状态

### 1. 检查服务是否正常运行

```bash
# 检查端口监听状态
netstat -an | grep -E "(50051|8080)"

# 应该看到类似输出：
# tcp4  0  0  *.50051  *.*  LISTEN
# tcp4  0  0  *.8080   *.*  LISTEN
```

### 2. 测试 HTTP 网关

```bash
# 健康检查
curl http://localhost:8080/health

# 预期响应：
# {"service":"sms","status":"healthy","timestamp":"..."}
```

### 3. 测试 gRPC 服务

```bash
# 使用 grpcurl 测试（如果已安装）
grpcurl -plaintext localhost:50051 list

# 或者检查服务日志中的成功消息：
# INFO sms: Registered services: NodeService, TaskService
```

## 日志分析

### 正常启动日志

```
INFO sms: Starting Node Service (Spear Node Management Service)...
INFO sms: Configuration loaded:
INFO sms:   gRPC server: 0.0.0.0:50051
INFO sms:   HTTP gateway: 0.0.0.0:8080
INFO sms: Starting HTTP gateway on 0.0.0.0:8080
INFO sms: Starting gRPC server on 0.0.0.0:50051
INFO sms: Registered services: NodeService, TaskService
WARN sms: Failed to connect to Node gRPC service: transport error, retrying in 2s...
INFO sms: Connected to Node gRPC service at http://0.0.0.0:50051
INFO sms: Connected to Task gRPC service at http://0.0.0.0:50051
INFO sms: HTTP gateway listening on 0.0.0.0:8080
INFO sms: Swagger UI available at: http://0.0.0.0:8080/swagger-ui/
```

### 问题日志模式

```
ERROR sms: gRPC server error: transport error
INFO sms: Node service stopped
```

如果看到这种模式，说明 gRPC 服务器无法启动，通常是端口被占用。

## 预防措施

1. **使用配置文件**：通过配置文件指定端口，避免硬编码
2. **端口检查**：启动前检查端口可用性
3. **优雅关闭**：使用 Ctrl+C 正确关闭服务，避免端口残留
4. **监控日志**：关注启动日志，及时发现问题

## 相关文档

- [CLI 配置指南](cli-configuration-zh.md)
- [服务配置文档](../config.toml)
- [HTTP API 文档](http-api-zh.md)