# Execution Mode Support Implementation Documentation

## Overview

This document describes the implementation details for adding asynchronous/synchronous execution mode support to the Spear project. This feature allows function calls to support three execution modes: Sync, Async, and Stream.

## Changes Made

### 1. ExecutionResponse Structure Refactoring

#### Problem
The original `ExecutionResponse` structure lacked execution mode-related fields and couldn't distinguish between synchronous and asynchronous execution states.

#### Solution
In `src/spearlet/execution/runtime/mod.rs`:

1. **Conflict Resolution**: Renamed `ExecutionResponse` in the runtime module to `RuntimeExecutionResponse` to avoid conflicts with `ExecutionResponse` in the execution module.

2. **Added New Fields**:
   ```rust
   pub struct RuntimeExecutionResponse {
       pub request_id: String,
       pub output_data: Vec<u8>,
       pub status: ExecutionStatus,
       pub error_message: String,
       pub execution_time_ms: u64,
       pub metadata: HashMap<String, String>,
       pub timestamp: SystemTime,
       // New fields
       pub execution_mode: ExecutionMode,     // Execution mode
       pub execution_status: ExecutionStatus, // Execution status
       pub is_async: bool,                    // Whether async execution
   }
   ```

3. **Convenience Constructors**:
   ```rust
   impl RuntimeExecutionResponse {
       pub fn new_sync(/* parameters */) -> Self { /* sync execution constructor */ }
       pub fn new_async(/* parameters */) -> Self { /* async execution constructor */ }
   }
   ```

### 2. Runtime Module Updates

Updated the following files to use the new `RuntimeExecutionResponse`:

- `src/spearlet/execution/runtime/docker.rs`
- `src/spearlet/execution/runtime/process.rs`
- `src/spearlet/execution/runtime/wasm.rs`

All runtime implementations now use the unified `RuntimeExecutionResponse` structure and create response objects through convenience constructors.

### 3. FunctionService Execution Mode Handling

#### Modified Method
In the `invoke_function` method in `src/spearlet/function_service.rs`:

```rust
async fn invoke_function(&self, request: Request<InvokeFunctionRequest>) 
    -> Result<Response<InvokeFunctionResponse>, Status> {
    
    match req.execution_mode() {
        ExecutionMode::Sync => {
            // Sync execution: wait for completion and return complete result
            let result = self.handle_sync_execution(&req, &execution_id, &task_id).await;
            // Return response with complete result
        },
        ExecutionMode::Async => {
            // Async execution: immediately return execution ID and status endpoint
            let (status_endpoint, estimated_ms) = self.handle_async_execution(&req, &execution_id, &task_id).await;
            // Return response with status endpoint
        },
        ExecutionMode::Stream => {
            // Streaming execution should use StreamFunction RPC
            // Return error suggesting correct RPC method
        },
        _ => {
            // Unknown execution mode handling
        }
    }
}
```

#### New Helper Methods

1. **`handle_sync_execution`**: Handles synchronous execution logic
2. **`handle_async_execution`**: Handles asynchronous execution logic, returns status endpoint and estimated completion time
3. **`create_failed_result`**: Creates failed execution results

## Execution Mode Descriptions

### Sync Mode
- Client waits for function execution completion after sending request
- Response contains complete execution result
- `status_endpoint` field is empty
- Suitable for quickly executing functions

### Async Mode
- Client receives immediate response after sending request
- Response contains execution ID and status query endpoint
- Client needs to poll execution result through status endpoint
- Suitable for long-running functions

### Stream Mode
- Uses separate `StreamFunction` RPC method
- Supports real-time data stream transmission
- Returns error message in `invoke_function` suggesting correct method

### Mode Comparison

| Mode | Response Timing | Result Retrieval | Use Case |
|------|----------------|------------------|----------|
| Sync | After completion | Directly in response | Quick functions |
| Async | Immediate return | Poll via status endpoint | Long-running |
| Stream | Use dedicated RPC | Real-time streaming | Stream processing |

## Response Structure Differences

### Sync Mode Response
```rust
InvokeFunctionResponse {
    success: true/false,
    message: "Execution result message",
    execution_id: "exec_xxx",
    task_id: "task_xxx",
    result: Some(ExecutionResult { /* complete result */ }),
    status_endpoint: "", // Empty string
    estimated_completion_ms: 0,
}
```

### Async Mode Response
```rust
InvokeFunctionResponse {
    success: true,
    message: "Async function execution started",
    execution_id: "exec_xxx",
    task_id: "task_xxx",
    result: Some(ExecutionResult { 
        status: Pending,
        completed_at: None, // Not yet completed
        // Other fields...
    }),
    status_endpoint: "/status/exec_xxx",
    estimated_completion_ms: 5000,
}
```

## Error Handling Strategy

1. **Unknown execution mode**: Return a clear error message
2. **Stream mode misuse**: Guide the client to use the correct RPC method
3. **Runtime errors**: Use unified error codes and a consistent message format

## Future Work

### Immediate Needs
1. Integrate `handle_sync_execution` and `handle_async_execution` with real runtimes
2. Implement status query endpoints (HTTP or gRPC)
3. Add async task scheduling and status tracking

### Medium-term Goals
1. Add execution timeout mechanisms
2. Implement task priority
3. Add execution monitoring and metrics

### Long-term Planning
1. Support task dependencies
2. Implement distributed execution
3. Add resource quota management

## Testing Strategy

### Unit Tests
1. Validate response structures for each execution mode
2. Test error scenario handling
3. Test convenience constructors

### Integration Tests
1. End-to-end execution flow tests
2. Compatibility tests across different runtimes
3. Performance benchmark tests

### Stress Tests
1. High-concurrency sync execution
2. Large-scale async task management
3. Resource usage monitoring

## Compatibility Notes

- This modification is backward compatible, existing client code requires no changes
- New execution mode fields have default values ensuring compatibility
- Recommend gradual client migration to new execution mode API
