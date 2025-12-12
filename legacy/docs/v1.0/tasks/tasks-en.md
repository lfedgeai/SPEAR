# SPEAR Project Task Breakdown (Rust + Interface First)

## 1. Interface Design Phase (High Priority)

### 1.1 SPEARlet Interface Design
- **Local RPC Interface**
  - Tasks: Task submission, status query, termination.
  - Object Store: Object operations, lifecycle management, concurrency control.
  - Health: Node health check.

- **HTTP / WebSocket Interface**
  - REST API: Task management, object management, node status.
  - WebSocket: Task status streaming.

**Deliverables**:
- gRPC proto files.
- OpenAPI documentation.
- Rust stub code.
- Unit tests.

### 1.2 Metadata Server (SMS) Interface Design
- Cluster information interface: Node registration/deregistration, status synchronization, heartbeat, node list query.
- Job interface: Task submission, task status management, task assignment, job lifecycle management.

**Deliverables**:
- gRPC proto files.
- OpenAPI documentation.
- Rust stub code.
- Unit tests.

### 1.3 Object Store Interface Design
- **Object Operation API**
  - `PutObject(bucket, key, data)`: Upload object.
  - `GetObject(bucket, key) -> data`: Download object.
  - `ListObjects(bucket) -> key[]`: List objects.

- **Object Lifecycle Management**
  - `AddObjectRef(ObjectID)`: Increase reference.
  - `RemoveObjectRef(ObjectID)`: Decrease reference.
  - `PinObject(ObjectID)`: Pin object.
  - `UnpinObject(ObjectID)`: Unpin object.

- **Concurrency Control**
  - Lock mechanism or transaction to ensure multi-task consistency.

**Deliverables**:
- gRPC proto files.
- OpenAPI documentation.
- Rust stub code.
- Unit tests.

## 2. Module Implementation Phase (Medium Priority)

### 2.1 SPEARlet Core Modules
- **Task Controller**
  - Task queue implementation.
  - Task lifecycle management.

- **Worker Agent**
  - WasmAgent: Execution via Wasmtime.
  - ProcessAgent: Local command execution.
  - DockerAgent: Execution via Docker.

- **Hostcall Controller**
  - System resource interface abstraction.

- **Node Controller**
  - Node status reporting.

**Deliverables**:
- Rust implementation code.
- Unit tests.

### 2.2 Metadata Server (SMS)
- **Node Management**
  - Registration, deregistration, heartbeat.
  - Query node list.

- **Task Management**
  - Submit task.
  - Query task status.
  - Assign task.

- **Inter-node Communication**
  - Task scheduling.
  - Status synchronization.
  - Fault detection.

**Deliverables**:
- Rust implementation code.
- Unit tests.

### 2.3 Object Store
- **Object Operations**
  - Upload, download, list objects.

- **Lifecycle Management**
  - Reference counting, pin/unpin.

- **Concurrency Control**
  - Lock mechanism or transaction.

**Deliverables**:
- Rust implementation code.
- Unit tests.

## 3. CLI Tool (Low Priority, Can Be Developed in Parallel)
- Submit task.
- Query task status.
- Terminate task.
- Get task logs.
- Get node list.
- Get object list.
- Get object metadata.

**Deliverables**:
- Rust implementation code.
- Unit tests.

## 4. Testing and Documentation
- **Unit Tests**: Module functionality tests.
- **Integration Tests**: Inter-module collaboration tests.
- **Interface Coverage Tests**: SPEARlet / SMS / Object Store API tests.
- **API Documentation**: OpenAPI, gRPC documentation.
- **Developer Guide**: Contribution process, code standards.
- **Example Code**: Demonstrate task submission, object operations, node management process.

## 5. Task Breakdown Strategy
- Each interface method and module subtask can generate an independent Issue.
- Priority: Interface definition -> Unit tests -> Module implementation -> Integration tests.
- Provide clear deliverables: proto files, Rust stub, unit tests, documentation.
- Contributors can choose tasks based on their skills: Task, Worker Agent, Hostcall, Node Controller, SMS, or Object Store.