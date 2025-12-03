# Spearlet Function Invocation Interface Design

## Overview

This document describes the design of the new `invoke function` gRPC interface for Spearlet. The interface supports two primary operation modes:
1. **Create new task and invoke function** - Automatically create a new task when the specified task doesn't exist and execute function invocation
2. **Invoke function on existing task** - Execute function invocation on already registered task instances

## Core Design Philosophy

### 1. Unified Invocation Interface
- Use `InvocationType` enumeration to distinguish between new task creation and existing task invocation
- Single interface supports multiple execution modes (sync, async, streaming)
- Flexible parameter passing mechanism

### 2. Artifact Management Integration
- Use `ArtifactSpec` to replace the original "Binary" concept
- Support multiple artifact types: binary files, ZIP packages, container images, etc.
- Artifact locations support local paths, HTTP URLs, container registries, etc.

### 3. Execution Context Management
- Complete execution context information (session, user, environment variables, etc.)
- Timeout and retry mechanisms
- Detailed execution metrics collection

## Detailed Interface Design

### 1. Core Enumeration Types

#### InvocationType - Invocation Type
```protobuf
enum InvocationType {
  INVOCATION_TYPE_UNKNOWN = 0;     // Unknown type
  INVOCATION_TYPE_NEW_TASK = 1;    // Create new task
  INVOCATION_TYPE_EXISTING_TASK = 2; // Invoke existing task
}
```

#### ExecutionMode - Execution Mode
```protobuf
enum ExecutionMode {
  EXECUTION_MODE_UNKNOWN = 0;      // Unknown mode
  EXECUTION_MODE_SYNC = 1;         // Synchronous execution
  EXECUTION_MODE_ASYNC = 2;        // Asynchronous execution
  EXECUTION_MODE_STREAM = 3;       // Streaming execution
}
```

#### ExecutionStatus - Execution Status
```protobuf
enum ExecutionStatus {
  EXECUTION_STATUS_UNKNOWN = 0;    // Unknown status
  EXECUTION_STATUS_PENDING = 1;    // Pending execution
  EXECUTION_STATUS_RUNNING = 2;    // Currently running
  EXECUTION_STATUS_COMPLETED = 3;  // Completed successfully
  EXECUTION_STATUS_FAILED = 4;     // Failed with error
  EXECUTION_STATUS_CANCELLED = 5;  // Cancelled by user
  EXECUTION_STATUS_TIMEOUT = 6;    // Execution timeout
}
```

### 2. Core Message Types

#### ArtifactSpec - Artifact Specification
```protobuf
message ArtifactSpec {
  string artifact_id = 1;           // Artifact identifier
  string artifact_type = 2;         // Artifact type (binary, zip, image, etc.)
  string location = 3;              // Artifact location (path, URL, registry)
  string version = 4;               // Artifact version
  string checksum = 5;              // Artifact checksum for verification
  map<string, string> metadata = 6; // Additional artifact metadata
}
```

Notes / 说明:
- The `artifact_id` provided by the client is treated as the fixed, canonical ID inside the system. Artifacts are created and stored using this ID without generating a separate internal UUID.
- For `artifact_type = "wasm"`, `location` must specify a supported scheme. Current support includes `sms+file://<file_id>`, which is fetched via SMS. Supplying `checksum` (SHA-256) is recommended for integrity verification.
- This fixed-ID strategy enables consistent lookup across artifacts, tasks, and instances, and avoids mismatches during artifact snapshot injection.

#### ExecutionContext - Execution Context
```protobuf
message ExecutionContext {
  string execution_id = 1;          // Unique execution identifier
  string session_id = 2;            // Session identifier
  string user_id = 3;               // User identifier
  map<string, string> environment = 4; // Environment variables
  map<string, string> headers = 5;  // Request headers
  int64 timeout_ms = 6;             // Execution timeout in milliseconds
  int32 max_retries = 7;            // Maximum retry attempts
}
```

### 3. Main Service Methods

#### InvokeFunction - Function Invocation
```protobuf
rpc InvokeFunction(InvokeFunctionRequest) returns (InvokeFunctionResponse);
```

**Request Parameters:**
- `invocation_type`: Specify whether to create new task or invoke existing task
- `task_name/task_description/artifact_spec`: Used when creating new task
- `task_id`: Used when invoking existing task
- `function_name`: Name of function to invoke
- `parameters`: Function parameter list
- `execution_mode`: Execution mode (sync/async/streaming)
- `context`: Execution context

#### GetExecutionStatus - Get Execution Status
```protobuf
rpc GetExecutionStatus(GetExecutionStatusRequest) returns (GetExecutionStatusResponse);
```

Used to query the status and results of asynchronous execution.

#### CancelExecution - Cancel Execution
```protobuf
rpc CancelExecution(CancelExecutionRequest) returns (CancelExecutionResponse);
```

Used to cancel running asynchronous tasks.

#### StreamFunction - Streaming Execution
```protobuf
rpc StreamFunction(InvokeFunctionRequest) returns (stream StreamExecutionResult);
```

Used for streaming execution mode, returning execution results in real-time.

## Usage Scenarios and Logic Flow

### Scenario 1: Create New Task and Invoke Function

```
Client Request -> Spearlet
├── invocation_type = INVOCATION_TYPE_NEW_TASK
├── task_name = "my-new-task"
├── artifact_spec = { artifact_type: "zip", location: "http://example.com/task.zip" }
├── function_name = "process_data"
└── parameters = [...]

Spearlet Processing Flow:
1. Check if task already exists
2. If not exists, create new task:
   - Download and verify artifact
   - Create task instance
   - Register to SMS (optional)
3. Execute function invocation
4. Return execution result
```

### Scenario 2: Invoke Function on Existing Task

```
Client Request -> Spearlet
├── invocation_type = INVOCATION_TYPE_EXISTING_TASK
├── task_id = "existing-task-123"
├── function_name = "analyze"
└── parameters = [...]

Spearlet Processing Flow:
1. Find specified task instance
2. If task doesn't exist and create_if_not_exists=true, create it
3. Get or create task instance
4. Execute function invocation
5. Return execution result
```

### Scenario 3: Asynchronous Execution Mode

```
Client Request -> Spearlet
├── execution_mode = EXECUTION_MODE_ASYNC
└── [other parameters...]

Spearlet Response:
├── success = true
├── execution_id = "exec-456"
├── status_endpoint = "/status/exec-456"
└── estimated_completion_ms = 30000

Client Subsequent Query:
GetExecutionStatus(execution_id="exec-456")
```

## Integration with Existing Architecture

### 1. Coordination with SMS
- Optional registration to SMS when creating new tasks
- Support SMS task discovery and load balancing
- Execution status can be synchronized to SMS

### 2. Integration with Artifact Management
- Use unified `ArtifactSpec` specification
- Support artifact caching and sharing
- Artifact version management and verification

### 3. Integration with Instance Management
- Reuse existing instance pool mechanisms
- Support instance lifecycle management
- Intelligent scheduling and resource optimization

## Error Handling and Monitoring

### 1. Error Classification
- **Artifact Errors**: Artifact download failure, verification failure, etc.
- **Task Errors**: Task creation failure, task not found, etc.
- **Execution Errors**: Function execution failure, timeout, etc.
- **System Errors**: Resource shortage, network errors, etc.

### 2. Monitoring Metrics
- Function invocation count and success rate
- Execution time distribution
- Resource usage
- Error rate and error type distribution

### 3. Logging
- Complete execution trace tracking
- Detailed error information recording
- Performance metrics collection

## Security Considerations

### 1. Authentication and Authorization
- Support user-based access control
- Task-level permission management
- Artifact access permission verification

### 2. Resource Isolation
- Execution environment isolation
- Resource quota limits
- Malicious code protection

### 3. Data Security
- Encrypted transmission of sensitive parameters
- Secure storage of execution results
- Audit log recording

## Performance Optimization

### 1. Artifact Caching
- Local artifact caching mechanism
- Artifact preloading strategy
- Cache invalidation and updates

### 2. Instance Reuse
- Hot instance pool management
- Instance warm-up mechanism
- Intelligent instance scheduling

### 3. Concurrency Control
- Concurrent execution limits
- Resource contention avoidance
- Load balancing strategies

## Summary

This design provides a flexible and powerful function invocation interface that supports:

1. **Unified Invocation Pattern** - New task creation and existing task invocation
2. **Multiple Execution Modes** - Sync, async, and streaming execution
3. **Complete Lifecycle Management** - From task creation to execution completion
4. **Rich Monitoring and Error Handling** - Comprehensive observability
5. **Good Extensibility** - Support for future feature extensions

This interface design integrates perfectly with the existing Spearlet architecture, providing users with a simple and easy-to-use function invocation experience.
