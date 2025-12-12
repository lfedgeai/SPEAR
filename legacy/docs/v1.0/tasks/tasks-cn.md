# SPEAR 项目任务分解（Rust + 接口优先）

## 1. 接口设计阶段（高优先级）

### 1.1 SPEARlet 接口设计
- **本地 RPC 接口**
  - Tasks：任务提交、状态查询、终止。
  - Object Store：对象操作、生命周期管理、并发控制。
  - Health：节点健康检查。

- **HTTP / WebSocket 接口**
  - REST API：任务管理、对象管理、节点状态。
  - WebSocket：任务状态流式推送。

**交付成果**：
- gRPC proto 文件。
- OpenAPI 文档。
- Rust stub 代码。
- 单元测试。

### 1.2 Metadata Server (SMS) 接口设计
- 集群信息接口：节点注册/注销、状态同步、心跳、节点列表查询。
- Job接口：任务提交、任务状态管理、任务分配、Job生命周期管理。

**交付成果**：
- gRPC proto 文件。
- OpenAPI 文档。
- Rust stub 代码。
- 单元测试。

### 1.3 Object Store 接口设计
- **对象操作 API**
  - `PutObject(bucket, key, data)`：上传对象。
  - `GetObject(bucket, key) -> data`：下载对象。
  - `ListObjects(bucket) -> key[]`：列出对象。

- **对象生命周期管理**
  - `AddObjectRef(ObjectID)`：增加引用。
  - `RemoveObjectRef(ObjectID)`：减少引用。
  - `PinObject(ObjectID)`：固定对象。
  - `UnpinObject(ObjectID)`：解固定对象。

- **并发控制**
  - 锁机制或事务保证多任务一致性。

**交付成果**：
- gRPC proto 文件。
- OpenAPI 文档。
- Rust stub 代码。
- 单元测试。

## 2. 模块实现阶段（中优先级）

### 2.1 SPEARlet 核心模块
- **Task Controller**
  - 任务队列实现。
  - 任务生命周期管理。

- **Worker Agent**
  - WasmAgent：通过 Wasmtime 执行。
  - ProcessAgent：本地命令执行。
  - DockerAgent：通过 Docker 执行。

- **Hostcall Controller**
  - 系统资源接口抽象。

- **Node Controller**
  - 节点状态上报。

**交付成果**：
- Rust 实现代码。
- 单元测试。

### 2.2 Metadata Server (SMS)
- **节点管理**
  - 注册、注销、心跳。
  - 查询节点列表。

- **任务管理**
  - 提交任务。
  - 查询任务状态。
  - 分配任务。

- **节点间通信**
  - 任务调度。
  - 状态同步。
  - 故障检测。

**交付成果**：
- Rust 实现代码。
- 单元测试。

### 2.3 Object Store
- **对象操作**
  - 上传、下载、列出对象。

- **生命周期管理**
  - 引用计数、固定/解固定。

- **并发控制**
  - 锁机制或事务。

**交付成果**：
- Rust 实现代码。
- 单元测试。

## 3. CLI 工具（低优先级，可并行开发）
- 提交任务。
- 查询任务状态。
- 终止任务。
- 获取任务日志。
- 获取节点列表。
- 获取对象列表。
- 获取对象元数据。

**交付成果**：
- Rust 实现代码。
- 单元测试。

## 4. 测试与文档
- **单元测试**：各模块功能测试。
- **集成测试**：模块间协作测试。
- **接口覆盖测试**：SPEARlet / SMS / Object Store API 测试。
- **API 文档**：OpenAPI、gRPC 文档。
- **开发者指南**：贡献流程、代码规范。
- **示例代码**：演示任务提交、对象操作、节点管理流程。

## 5. 任务拆分策略
- 每个接口方法、每个模块子任务都可以生成独立 Issue。
- 优先接口定义 -> 单元测试 -> 模块实现 -> 集成测试。
- 提供明确交付物：proto 文件、Rust stub、单元测试、文档。
- 贡献者可根据技能选择 Task、Worker Agent、Hostcall、Node Controller、SMS或Object Store。