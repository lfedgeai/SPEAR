# Spear Project Architecture Overview

## Project Summary
Spear is a distributed serverless computing platform built in Rust, designed to provide scalable function execution across multiple runtime environments including WASM, Docker, and native processes.

## Core Architecture

### 1. SMS (Spear Management Service)
The central management layer responsible for:
- **Node Management**: Registration, health monitoring, and lifecycle management of compute nodes
- **Resource Management**: Tracking and allocation of compute resources across the cluster
- **Task Management**: Registration, scheduling, and coordination of execution tasks

#### Key Components:
- `NodeService`: Handles node registration, heartbeat, and status management
- `ResourceService`: Manages resource allocation and monitoring
- `TaskService`: Coordinates task distribution and execution tracking

### 2. Spearlet (Execution Node)
Individual compute nodes that execute functions and tasks:
- **Function Execution**: Direct function invocation with multiple runtime support
- **Instance Management**: Pool-based instance lifecycle management
- **Resource Monitoring**: Real-time resource usage tracking and reporting

#### Key Components:
- `FunctionService`: gRPC service for function invocation and management
- `TaskExecutionManager`: Coordinates task execution across runtime environments
- `InstancePool`: Manages reusable execution instances for performance optimization
- `RuntimeManager`: Abstracts different execution environments (WASM, Docker, Process)

## Runtime Support

### 1. WASM Runtime
- **Security**: Sandboxed execution environment
- **Performance**: Fast startup and low overhead
- **Resource Limits**: Configurable CPU and memory constraints
- **Validation**: Comprehensive configuration validation

### 2. Docker Runtime
- **Isolation**: Container-based execution environment
- **Flexibility**: Support for complex dependencies and environments
- **Scalability**: Dynamic container lifecycle management

### 3. Process Runtime
- **Native Performance**: Direct process execution
- **System Integration**: Full system access when needed
- **Legacy Support**: Compatibility with existing applications

## Key Features

### Distributed Architecture
- **Horizontal Scaling**: Add compute nodes dynamically
- **Load Distribution**: Intelligent task distribution across nodes
- **Fault Tolerance**: Graceful handling of node failures

### Multi-Runtime Support
- **Runtime Abstraction**: Unified interface across different execution environments
- **Configuration Management**: Per-runtime configuration and validation
- **Resource Management**: Runtime-specific resource limits and monitoring

### gRPC API
- **High Performance**: Efficient binary protocol for inter-service communication
- **Type Safety**: Protocol buffer definitions ensure API consistency
- **Streaming Support**: Real-time execution monitoring and logging

### Resource Management
- **Dynamic Allocation**: Real-time resource allocation and deallocation
- **Monitoring**: Comprehensive resource usage tracking
- **Limits Enforcement**: Configurable resource limits per execution

## Data Flow

### Function Invocation Flow
1. **Request Reception**: Spearlet receives function invocation request
2. **Runtime Selection**: Determine appropriate runtime based on function requirements
3. **Instance Allocation**: Acquire or create execution instance from pool
4. **Execution**: Execute function in selected runtime environment
5. **Result Collection**: Gather execution results and resource usage metrics
6. **Response**: Return results to client with execution metadata

### Task Management Flow
1. **Task Registration**: SMS receives task registration from client
2. **Resource Planning**: Analyze resource requirements and availability
3. **Node Selection**: Choose optimal execution node based on resources and load
4. **Task Dispatch**: Send task to selected Spearlet for execution
5. **Execution Monitoring**: Track task progress and resource usage
6. **Result Aggregation**: Collect and store execution results

## Configuration Management

### Hierarchical Configuration
- **Global Settings**: System-wide configuration parameters
- **Service-Specific**: Individual service configuration (SMS, Spearlet)
- **Runtime-Specific**: Per-runtime configuration and limits
- **Instance-Level**: Per-execution instance configuration

### Storage Backends
- **Memory**: In-memory storage for development and testing
- **RocksDB**: High-performance embedded database for production
- **Sled**: Alternative embedded database option

## API Design

### RESTful HTTP API
- **Gateway Integration**: HTTP gateway for external client access
- **OpenAPI Documentation**: Comprehensive API documentation with Swagger UI
- **Content Negotiation**: Support for multiple content types

### gRPC Services
- **Internal Communication**: High-performance inter-service communication
- **Streaming**: Real-time data streaming for monitoring and logging
- **Error Handling**: Comprehensive error handling and status reporting

## Testing Strategy

### Unit Testing
- **Component Isolation**: Individual component testing with mocks
- **Runtime Validation**: Comprehensive runtime configuration testing
- **Error Scenarios**: Testing error handling and edge cases

### Integration Testing
- **Service Integration**: End-to-end service communication testing
- **Runtime Integration**: Multi-runtime execution testing
- **Resource Management**: Resource allocation and cleanup testing

### Performance Testing
- **Load Testing**: High-concurrency execution testing
- **Resource Monitoring**: Performance metrics collection and analysis
- **Scalability Testing**: Multi-node deployment testing

## Security Considerations

### Execution Isolation
- **Sandboxing**: WASM and container-based isolation
- **Resource Limits**: Strict resource usage enforcement
- **Network Isolation**: Controlled network access for executions

### API Security
- **Authentication**: Service-to-service authentication
- **Authorization**: Role-based access control
- **Input Validation**: Comprehensive input sanitization

## Monitoring and Observability

### Metrics Collection
- **Execution Metrics**: Function execution time, success/failure rates
- **Resource Metrics**: CPU, memory, disk, and network usage
- **System Metrics**: Node health, service availability

### Logging
- **Structured Logging**: JSON-formatted logs for analysis
- **Distributed Tracing**: Request tracing across services
- **Error Tracking**: Comprehensive error logging and alerting

## Future Roadmap

### Planned Enhancements
- **Auto-scaling**: Automatic node scaling based on load
- **Advanced Scheduling**: ML-based task scheduling optimization
- **Multi-cloud Support**: Deployment across multiple cloud providers
- **Enhanced Security**: Advanced isolation and security features

### Performance Optimizations
- **Cold Start Reduction**: Instance warming and pre-allocation
- **Network Optimization**: Advanced networking and caching
- **Resource Optimization**: Intelligent resource allocation algorithms