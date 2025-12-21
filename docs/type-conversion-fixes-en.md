# Type Conversion Fixes Documentation

## Overview

This document records the type conversion issues fixed in the spear-next project, mainly involving the conversion from `ArtifactSpec` to `TaskSpec`.

## Problem Description

### 1. TaskSpec vs ArtifactSpec Type Mismatch

**Issue**: In the `manager.rs` file, the `Task::new` method expects to receive a `TaskSpec` type, but the code was passing an `ArtifactSpec` type.

**Location**: `src/spearlet/execution/manager.rs:490`

**Original Code**:
```rust
Task::new(task_id.clone(), artifact.spec.clone())
```

**Solution**: Manually create a `TaskSpec` struct, mapping fields from `ArtifactSpec` to `TaskSpec`.

**Fixed Code**:
```rust
let task_spec = TaskSpec {
    scaling: Default::default(),
    health_check: Default::default(),
    timeout: Default::default(),
    environment: artifact.spec.environment.clone().unwrap_or_default(),
    resource_limits: artifact.spec.resource_limits.clone(),
    labels: artifact.spec.labels.clone().unwrap_or_default(),
};
let task = Task::new(task_id.clone(), task_spec);
```

### 2. Instance Status and Method Call Errors

**Issue**: Multiple instance status and method call errors existed in the `pool.rs` file:
- `InstanceStatus::Failed` doesn't exist, should use `InstanceStatus::Error(_)`
- Incorrect method call patterns

**Fixes**:
- Changed `inst.config()` to `inst.config.max_concurrent_requests`
- Changed `inst.active_requests()` to `inst.get_metrics().active_requests`
- Changed `InstanceStatus::Failed` to `InstanceStatus::Error(_) | InstanceStatus::Unhealthy`
- Changed `inst.metrics().average_response_time_ms` to `inst.get_metrics().avg_request_time_ms`

### 3. TaskSpec::new Errors in Test Code

**Issue**: Test code in `scheduler.rs` and `pool.rs` used the non-existent `TaskSpec::new` method.

**Solution**: Manually create `TaskSpec` structs with all required fields.

## Type Mapping

Field mapping from `ArtifactSpec` to `TaskSpec`:

| ArtifactSpec Field | TaskSpec Field | Conversion Method |
|-------------------|----------------|-------------------|
| name | name | Direct copy |
| runtime_type | runtime_type | Direct copy |
| environment | environment | unwrap_or_default() |
| resource_limits | - | Not directly mapped |
| labels | - | Not directly mapped |
| - | task_type | Default to HttpHandler |
| - | entry_point | Default to "main" |
| - | handler_config | Empty HashMap |
| - | invocation_type | Copy from ArtifactSpec |
| - | min_instances | Default to 1 |
| - | max_instances | Default to 10 |
| - | target_concurrency | Default to 1 |
| - | scaling_config | Default::default() |
| - | health_check | Default::default() |
| - | timeout_config | Default::default() |

## Impact Scope

The fixes involve the following files:
- `src/spearlet/execution/manager.rs`
- `src/spearlet/execution/pool.rs`
- `src/spearlet/execution/scheduler.rs`

## Detailed Fix Records

### pool.rs Fix Details

1. **Type conversion errors on lines 526 and 773**
   - Issue: `instance.id()` returns `&str`, but `remove_instance` expects `&InstanceId` (i.e., `&String`)
   - Fix: Convert `instance.id()` to `String`, then pass reference
   ```rust
   // Before fix
   self.scheduler.remove_instance(instance.id()).await;
   
   // After fix
   let instance_id = instance.id().to_string();
   self.scheduler.remove_instance(&instance_id).await;
   ```

2. **is_idle method parameter error on line 510**
   - Issue: `is_idle` method requires `Duration` parameter
   - Fix: Add 5-minute idle timeout threshold
   ```rust
   // Before fix
   if instance.is_idle() {
   
   // After fix
   if instance.is_idle(Duration::from_secs(300)) {
   ```

3. **Non-existent method call on line 750**
   - Issue: Called non-existent `last_activity_time()` method
   - Fix: Replace with `is_idle(idle_timeout)` call
   ```rust
   // Before fix
   if state.task.last_activity_time() > idle_timeout {
   
   // After fix
   if !state.task.is_idle(idle_timeout) {
   ```

4. **Missing ArtifactSpec fields in test code**
   - Issue: `ArtifactSpec` struct missing required fields
   - Fix: Add all required fields
   ```rust
   ArtifactSpec {
       name: "test_artifact".to_string(),
       version: "1.0.0".to_string(),
       description: Some("Test artifact".to_string()),
       runtime_type: RuntimeType::Process,
       runtime_config: HashMap::new(),
       environment: Some(HashMap::new()),
       resource_limits: Default::default(),
       invocation_type: InvocationType::Sync,
       max_execution_timeout_ms: 30000,
       labels: HashMap::new(),
   }
   ```

5. **Manual TaskSpec construction in test code**
   - Issue: `TaskSpec::new` method does not exist
   - Fix: Manually construct `TaskSpec` struct
   ```rust
   let task_spec = TaskSpec {
       name: artifact_spec.name.clone(),
       task_type: TaskType::HttpHandler,
       runtime_type: artifact_spec.runtime_type,
       entry_point: "main".to_string(),
       handler_config: HashMap::new(),
       environment: artifact_spec.environment.unwrap_or_default(),
       invocation_type: artifact_spec.invocation_type,
       min_instances: 1,
       max_instances: 10,
       target_concurrency: 1,
       scaling_config: ScalingConfig::default(),
       health_check: HealthCheckConfig::default(),
       timeout_config: TimeoutConfig::default(),
   };
   ```

## Verification Results

After the fixes, the project compiles successfully with only some warnings about the `evmap` feature, which do not affect functionality.

```bash
cargo check
# Compilation successful, only evmap feature warnings
```

All type conversion errors have been fixed, and test code can compile and run normally.

## Recommendations

1. **Type Safety**: Consider implementing the `From` trait for `ArtifactSpec` to `TaskSpec` conversion to provide type-safe conversion methods.

2. **Documentation**: Add detailed documentation for type conversions, clarifying the mapping relationships between fields.

3. **Test Coverage**: Add unit tests for type conversion logic to ensure conversion correctness.

## Related Links

- [TaskSpec Definition](../src/spearlet/execution/task.rs)
- [ArtifactSpec Definition](../src/spearlet/execution/artifact.rs)
- [Manager Implementation](../src/spearlet/execution/manager.rs)