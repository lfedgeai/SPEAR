# Kubernetes Runtime Implementation

## Overview

This document describes the implementation of the Kubernetes runtime for the Spear execution engine. The Kubernetes runtime enables the execution of tasks as Kubernetes Jobs, providing scalable and containerized execution capabilities.

## Architecture

### Core Components

1. **KubernetesRuntime**: Main runtime implementation that manages Kubernetes Jobs
2. **KubernetesConfig**: Configuration structure for the runtime
3. **KubernetesJobHandle**: Handle structure to track running Kubernetes jobs

### Key Features

- **Job Management**: Create, start, stop, and cleanup Kubernetes Jobs
- **Pod Monitoring**: Track pod status and health
- **Resource Management**: Configure CPU, memory, and other resource limits
- **Namespace Support**: Deploy jobs in specified Kubernetes namespaces
- **Error Handling**: Comprehensive error handling for Kubernetes operations

## Implementation Details

### Configuration

The `KubernetesConfig` structure supports:
- `namespace`: Target Kubernetes namespace (default: "default")
- `kubeconfig_path`: Path to kubeconfig file (optional, uses in-cluster config if not specified)
- `job_timeout_seconds`: Timeout for job execution (default: 3600 seconds)
- `cleanup_policy`: Job cleanup policy ("Always", "OnSuccess", "OnFailure")

### Runtime Capabilities

The Kubernetes runtime provides the following capabilities:
- Scaling support: Yes
- Health checks: Yes
- Metrics collection: Yes
- Hot reload: No
- Persistent storage: Yes
- Network isolation: Yes
- Maximum concurrent instances: 100
- Supported protocols: ["http", "grpc"]

### Job Lifecycle

1. **Create Instance**: Generate Kubernetes Job manifest from instance configuration
2. **Start Instance**: Apply the Job manifest to the Kubernetes cluster
3. **Monitor**: Track job and pod status
4. **Execute**: Send requests to the running pod
5. **Health Check**: Verify pod health status
6. **Stop**: Delete the Kubernetes Job
7. **Cleanup**: Remove associated resources

### Error Handling

The runtime handles various error scenarios:
- Kubernetes API errors
- Pod startup failures
- Network connectivity issues
- Resource allocation failures
- Timeout scenarios

## Usage Example

```rust
use spear_next::spearlet::execution::runtime::{KubernetesRuntime, KubernetesConfig};

// Create runtime configuration
let config = KubernetesConfig {
    namespace: "spear-tasks".to_string(),
    kubeconfig_path: Some("/path/to/kubeconfig".to_string()),
    job_timeout_seconds: 1800,
    cleanup_policy: "OnSuccess".to_string(),
};

// Initialize runtime
let runtime = KubernetesRuntime::new(&config)?;

// Create instance configuration
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

// Create and start instance
let instance = runtime.create_instance(&instance_config).await?;
runtime.start_instance(&instance).await?;
```

## Testing

The implementation includes comprehensive tests:
- Unit tests for configuration validation
- Integration tests for job lifecycle
- Mock tests for Kubernetes API interactions
- Error scenario testing

## Dependencies

- `k8s-openapi`: Kubernetes API types
- `kube`: Kubernetes client library
- `serde`: Serialization/deserialization
- `tokio`: Async runtime
- `uuid`: Unique identifier generation

## Future Enhancements

Potential improvements for the Kubernetes runtime:
1. Support for Kubernetes Deployments
2. Advanced scheduling policies
3. Custom resource definitions (CRDs)
4. Multi-cluster support
5. Enhanced monitoring and observability
6. Automatic scaling based on load

## Security Considerations

- Use RBAC to limit Kubernetes permissions
- Secure kubeconfig file access
- Network policies for pod isolation
- Resource quotas to prevent resource exhaustion
- Image security scanning integration