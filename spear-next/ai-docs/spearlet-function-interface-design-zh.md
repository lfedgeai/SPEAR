# Spearlet 函数调用接口设计

## 概述

本文档描述了 Spearlet 端新增的 `invoke function` gRPC 接口设计。该接口支持两种主要操作模式：
1. **创建新任务并调用函数** - 当指定的任务不存在时，自动创建新任务并执行函数调用
2. **调用现有任务的函数** - 在已注册的任务实例上执行函数调用

## 核心设计理念

### 1. 统一的调用接口
- 通过 `InvocationType` 枚举区分新任务创建和现有任务调用
- 单一接口支持多种执行模式（同步、异步、流式）
- 灵活的参数传递机制

### 2. 制品管理集成
- 使用 `ArtifactSpec` 替代原有的 "Binary" 概念
- 支持多种制品类型：二进制文件、ZIP 包、容器镜像等
- 制品位置支持本地路径、HTTP URL、容器仓库等

### 3. 执行上下文管理
- 完整的执行上下文信息（会话、用户、环境变量等）
- 超时和重试机制
- 详细的执行指标收集

## 接口详细设计

### 1. 核心枚举类型

#### InvocationType - 调用类型
```protobuf
enum InvocationType {
  INVOCATION_TYPE_UNKNOWN = 0;     // 未知类型
  INVOCATION_TYPE_NEW_TASK = 1;    // 创建新任务
  INVOCATION_TYPE_EXISTING_TASK = 2; // 调用现有任务
}
```

#### ExecutionMode - 执行模式
```protobuf
enum ExecutionMode {
  EXECUTION_MODE_UNKNOWN = 0;      // 未知模式
  EXECUTION_MODE_SYNC = 1;         // 同步执行
  EXECUTION_MODE_ASYNC = 2;        // 异步执行
  EXECUTION_MODE_STREAM = 3;       // 流式执行
}
```

#### ExecutionStatus - 执行状态
```protobuf
enum ExecutionStatus {
  EXECUTION_STATUS_UNKNOWN = 0;    // 未知状态
  EXECUTION_STATUS_PENDING = 1;    // 等待执行
  EXECUTION_STATUS_RUNNING = 2;    // 正在运行
  EXECUTION_STATUS_COMPLETED = 3;  // 成功完成
  EXECUTION_STATUS_FAILED = 4;     // 执行失败
  EXECUTION_STATUS_CANCELLED = 5;  // 用户取消
  EXECUTION_STATUS_TIMEOUT = 6;    // 执行超时
}
```

### 2. 核心消息类型

#### ArtifactSpec - 制品规范
```protobuf
message ArtifactSpec {
  string artifact_id = 1;           // 制品标识符
  string artifact_type = 2;         // 制品类型（binary, zip, image等）
  string location = 3;              // 制品位置（路径、URL、仓库）
  string version = 4;               // 制品版本
  string checksum = 5;              // 制品校验和
  map<string, string> metadata = 6; // 额外制品元数据
}
```

说明：
- `artifact_id` 为客户端提供的固定唯一标识，系统内部直接使用该 ID，不再生成内部 UUID。
- 当 `artifact_type="wasm"` 时，`location` 支持 `sms+file` 协议：
  - 显式指定：`sms+file://<host:port>/<file_id>`（优先使用该地址）
  - 简洁指定：`sms+file://<file_id>`（运行时使用 `SpearletConfig.sms_http_addr` 访问 SMS HTTP 网关）
- 建议提供 `checksum`（SHA-256）以进行内容校验。

#### ExecutionContext - 执行上下文
```protobuf
message ExecutionContext {
  string execution_id = 1;          // 唯一执行标识符
  string session_id = 2;            // 会话标识符
  string user_id = 3;               // 用户标识符
  map<string, string> environment = 4; // 环境变量
  map<string, string> headers = 5;  // 请求头
  int64 timeout_ms = 6;             // 执行超时时间（毫秒）
  int32 max_retries = 7;            // 最大重试次数
}
```

### 3. 主要服务方法

#### InvokeFunction - 函数调用
```protobuf
rpc InvokeFunction(InvokeFunctionRequest) returns (InvokeFunctionResponse);
```

**请求参数说明：**
- `invocation_type`: 指定是创建新任务还是调用现有任务
- `task_name/task_description/artifact_spec`: 新任务创建时使用
- `task_id`: 现有任务调用时使用
- `function_name`: 要调用的函数名称
- `parameters`: 函数参数列表
- `execution_mode`: 执行模式（同步/异步/流式）
- `context`: 执行上下文

#### GetExecutionStatus - 获取执行状态
```protobuf
rpc GetExecutionStatus(GetExecutionStatusRequest) returns (GetExecutionStatusResponse);
```

用于查询异步执行的状态和结果。

#### CancelExecution - 取消执行
```protobuf
rpc CancelExecution(CancelExecutionRequest) returns (CancelExecutionResponse);
```

用于取消正在执行的异步任务。

#### StreamFunction - 流式执行
```protobuf
rpc StreamFunction(InvokeFunctionRequest) returns (stream StreamExecutionResult);
```

用于流式执行模式，实时返回执行结果。

## 使用场景和逻辑流程

### 场景1：创建新任务并调用函数

```
客户端请求 -> Spearlet
├── invocation_type = INVOCATION_TYPE_NEW_TASK
├── task_name = "my-new-task"
├── artifact_spec = { artifact_type: "zip", location: "http://example.com/task.zip" }
├── function_name = "process_data"
└── parameters = [...]

Spearlet 处理流程：
1. 检查任务是否已存在
2. 如果不存在，创建新任务：
   - 下载并验证制品
   - 创建任务实例
   - 注册到 SMS（可选）
3. 执行函数调用
4. 返回执行结果
```

### 场景2：调用现有任务的函数

```
客户端请求 -> Spearlet
├── invocation_type = INVOCATION_TYPE_EXISTING_TASK
├── task_id = "existing-task-123"
├── function_name = "analyze"
└── parameters = [...]

Spearlet 处理流程：
1. 查找指定的任务实例
2. 如果任务不存在且 create_if_not_exists=true，则创建
3. 获取或创建任务实例
4. 执行函数调用
5. 返回执行结果
```

### 场景3：异步执行模式

```
客户端请求 -> Spearlet
├── execution_mode = EXECUTION_MODE_ASYNC
└── [其他参数...]

Spearlet 响应：
├── success = true
├── execution_id = "exec-456"
├── status_endpoint = "/status/exec-456"
└── estimated_completion_ms = 30000

客户端后续查询：
GetExecutionStatus(execution_id="exec-456")
```

## 与现有架构的集成

### 1. 与 SMS 的协调
- 新任务创建时可选择是否注册到 SMS
- 支持 SMS 的任务发现和负载均衡
- 执行状态可同步到 SMS

### 2. 与制品管理的集成
- 使用统一的 `ArtifactSpec` 规范
- 支持制品缓存和共享
- 制品版本管理和校验

### 3. 与实例管理的集成
- 复用现有的实例池机制
- 支持实例生命周期管理
- 智能调度和资源优化

## 错误处理和监控

### 1. 错误分类
- **制品错误**: 制品下载失败、校验失败等
- **任务错误**: 任务创建失败、任务不存在等
- **执行错误**: 函数执行失败、超时等
- **系统错误**: 资源不足、网络错误等

### 2. 监控指标
- 函数调用次数和成功率
- 执行时间分布
- 资源使用情况
- 错误率和错误类型分布

### 3. 日志记录
- 完整的执行链路追踪
- 详细的错误信息记录
- 性能指标收集

## 安全考虑

### 1. 认证和授权
- 支持基于用户的访问控制
- 任务级别的权限管理
- 制品访问权限验证

### 2. 资源隔离
- 执行环境隔离
- 资源配额限制
- 恶意代码防护

### 3. 数据安全
- 敏感参数加密传输
- 执行结果安全存储
- 审计日志记录

## 性能优化

### 1. 制品缓存
- 本地制品缓存机制
- 制品预加载策略
- 缓存失效和更新

### 2. 实例复用
- 热实例池管理
- 实例预热机制
- 智能实例调度

### 3. 并发控制
- 并发执行限制
- 资源争用避免
- 负载均衡策略

## 总结

这个设计提供了一个灵活、强大的函数调用接口，支持：

1. **统一的调用模式** - 新任务创建和现有任务调用
2. **多种执行模式** - 同步、异步、流式执行
3. **完整的生命周期管理** - 从任务创建到执行完成
4. **丰富的监控和错误处理** - 全面的可观测性
5. **良好的扩展性** - 支持未来功能扩展

该接口设计与现有的 Spearlet 架构完美集成，为用户提供了简单易用的函数调用体验。
