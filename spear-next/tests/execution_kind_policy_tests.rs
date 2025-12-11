use std::sync::Arc;

#[tokio::test]
async fn test_long_running_existing_task_invocation_rejected() {
    use async_trait::async_trait;
    use spear_next::spearlet::execution::instance;
    use spear_next::spearlet::execution::manager::{
        TaskExecutionManager, TaskExecutionManagerConfig,
    };
    use spear_next::spearlet::execution::runtime::{
        ExecutionContext as RtCtx, Runtime, RuntimeCapabilities, RuntimeExecutionResponse,
        RuntimeManager, RuntimeType,
    };

    struct DummyRuntime;
    #[async_trait]
    impl Runtime for DummyRuntime {
        fn runtime_type(&self) -> RuntimeType {
            RuntimeType::Process
        }
        async fn create_instance(
            &self,
            config: &instance::InstanceConfig,
        ) -> spear_next::spearlet::execution::ExecutionResult<Arc<instance::TaskInstance>> {
            Ok(Arc::new(instance::TaskInstance::new(
                config.task_id.clone(),
                config.clone(),
            )))
        }
        async fn start_instance(
            &self,
            _instance: &Arc<instance::TaskInstance>,
        ) -> spear_next::spearlet::execution::ExecutionResult<()> {
            Ok(())
        }
        async fn stop_instance(
            &self,
            _instance: &Arc<instance::TaskInstance>,
        ) -> spear_next::spearlet::execution::ExecutionResult<()> {
            Ok(())
        }
        async fn execute(
            &self,
            _instance: &Arc<instance::TaskInstance>,
            _context: RtCtx,
        ) -> spear_next::spearlet::execution::ExecutionResult<RuntimeExecutionResponse> {
            Ok(RuntimeExecutionResponse::new_sync(
                "exec-1".to_string(),
                vec![],
                1,
            ))
        }
        async fn health_check(
            &self,
            _instance: &Arc<instance::TaskInstance>,
        ) -> spear_next::spearlet::execution::ExecutionResult<bool> {
            Ok(true)
        }
        async fn get_metrics(
            &self,
            _instance: &Arc<instance::TaskInstance>,
        ) -> spear_next::spearlet::execution::ExecutionResult<
            std::collections::HashMap<String, serde_json::Value>,
        > {
            Ok(std::collections::HashMap::new())
        }
        async fn scale_instance(
            &self,
            _instance: &Arc<instance::TaskInstance>,
            _new_limits: &instance::InstanceResourceLimits,
        ) -> spear_next::spearlet::execution::ExecutionResult<()> {
            Ok(())
        }
        async fn cleanup_instance(
            &self,
            _instance: &Arc<instance::TaskInstance>,
        ) -> spear_next::spearlet::execution::ExecutionResult<()> {
            Ok(())
        }
        fn validate_config(
            &self,
            _config: &instance::InstanceConfig,
        ) -> spear_next::spearlet::execution::ExecutionResult<()> {
            Ok(())
        }
        fn get_capabilities(&self) -> RuntimeCapabilities {
            RuntimeCapabilities::default()
        }
    }

    let mut rm = RuntimeManager::new();
    rm.register_runtime(RuntimeType::Process, Box::new(DummyRuntime))
        .unwrap();
    let rm = Arc::new(rm);

    let mut cfg = spear_next::spearlet::config::SpearletConfig::default();
    cfg.sms_grpc_addr = "127.0.0.1:50051".to_string();
    let mgr = TaskExecutionManager::new(TaskExecutionManagerConfig::default(), rm, Arc::new(cfg))
        .await
        .unwrap();

    let mut labels = std::collections::HashMap::new();
    labels.insert("execution_kind".to_string(), "long_running".to_string());
    let artifact_spec = spear_next::proto::spearlet::ArtifactSpec {
        artifact_id: "artifact-a".to_string(),
        artifact_type: "process".to_string(),
        location: "file:///bin/foo".to_string(),
        version: "v1".to_string(),
        checksum: String::new(),
        metadata: labels,
    };

    // Pre-register task locally via SMS-like mapping
    let mut sms_task = spear_next::proto::sms::Task::default();
    sms_task.task_id = "task-long-1".to_string();
    sms_task.name = "t-long".to_string();
    sms_task.version = "v1".to_string();
    sms_task.metadata = std::collections::HashMap::new();
    sms_task.config = std::collections::HashMap::new();
    sms_task.execution_kind = spear_next::proto::sms::TaskExecutionKind::LongRunning as i32;
    sms_task.executable = Some(spear_next::proto::sms::TaskExecutable {
        r#type: 5,
        uri: "file:///bin/foo".to_string(),
        name: String::new(),
        checksum_sha256: String::new(),
        args: vec![],
        env: std::collections::HashMap::new(),
    });
    let artifact_arc = mgr.ensure_artifact_from_sms(&sms_task).await.unwrap();
    let _ = mgr
        .ensure_task_from_sms(&sms_task, &artifact_arc)
        .await
        .unwrap();

    let mut req2 = spear_next::proto::spearlet::InvokeFunctionRequest::default();
    req2.task_id = "task-long-1".to_string();
    req2.invocation_type = spear_next::proto::spearlet::InvocationType::ExistingTask as i32;
    req2.artifact_spec = Some(artifact_spec.clone());
    let err = mgr.submit_execution(req2).await.err().unwrap();
    match err {
        spear_next::spearlet::execution::ExecutionError::NotSupported { .. } => {}
        _ => panic!("expected NotSupported"),
    }
}

#[tokio::test]
async fn test_short_running_existing_task_invocation_allowed() {
    use async_trait::async_trait;
    use spear_next::spearlet::execution::instance;
    use spear_next::spearlet::execution::manager::{
        TaskExecutionManager, TaskExecutionManagerConfig,
    };
    use spear_next::spearlet::execution::runtime::{
        ExecutionContext as RtCtx, Runtime, RuntimeCapabilities, RuntimeExecutionResponse,
        RuntimeManager, RuntimeType,
    };

    struct DummyRuntime;
    #[async_trait]
    impl Runtime for DummyRuntime {
        fn runtime_type(&self) -> RuntimeType {
            RuntimeType::Process
        }
        async fn create_instance(
            &self,
            config: &instance::InstanceConfig,
        ) -> spear_next::spearlet::execution::ExecutionResult<Arc<instance::TaskInstance>> {
            Ok(Arc::new(instance::TaskInstance::new(
                config.task_id.clone(),
                config.clone(),
            )))
        }
        async fn start_instance(
            &self,
            _instance: &Arc<instance::TaskInstance>,
        ) -> spear_next::spearlet::execution::ExecutionResult<()> {
            Ok(())
        }
        async fn stop_instance(
            &self,
            _instance: &Arc<instance::TaskInstance>,
        ) -> spear_next::spearlet::execution::ExecutionResult<()> {
            Ok(())
        }
        async fn execute(
            &self,
            _instance: &Arc<instance::TaskInstance>,
            _context: RtCtx,
        ) -> spear_next::spearlet::execution::ExecutionResult<RuntimeExecutionResponse> {
            Ok(RuntimeExecutionResponse::new_sync(
                "exec-1".to_string(),
                vec![],
                1,
            ))
        }
        async fn health_check(
            &self,
            _instance: &Arc<instance::TaskInstance>,
        ) -> spear_next::spearlet::execution::ExecutionResult<bool> {
            Ok(true)
        }
        async fn get_metrics(
            &self,
            _instance: &Arc<instance::TaskInstance>,
        ) -> spear_next::spearlet::execution::ExecutionResult<
            std::collections::HashMap<String, serde_json::Value>,
        > {
            Ok(std::collections::HashMap::new())
        }
        async fn scale_instance(
            &self,
            _instance: &Arc<instance::TaskInstance>,
            _new_limits: &instance::InstanceResourceLimits,
        ) -> spear_next::spearlet::execution::ExecutionResult<()> {
            Ok(())
        }
        async fn cleanup_instance(
            &self,
            _instance: &Arc<instance::TaskInstance>,
        ) -> spear_next::spearlet::execution::ExecutionResult<()> {
            Ok(())
        }
        fn validate_config(
            &self,
            _config: &instance::InstanceConfig,
        ) -> spear_next::spearlet::execution::ExecutionResult<()> {
            Ok(())
        }
        fn get_capabilities(&self) -> RuntimeCapabilities {
            RuntimeCapabilities::default()
        }
    }

    let mut rm = RuntimeManager::new();
    rm.register_runtime(RuntimeType::Process, Box::new(DummyRuntime))
        .unwrap();
    let rm = Arc::new(rm);

    let cfg = Arc::new(spear_next::spearlet::config::SpearletConfig::default());
    let mgr = TaskExecutionManager::new(TaskExecutionManagerConfig::default(), rm, cfg)
        .await
        .unwrap();

    let labels = std::collections::HashMap::new();
    let artifact_spec = spear_next::proto::spearlet::ArtifactSpec {
        artifact_id: "artifact-b".to_string(),
        artifact_type: "process".to_string(),
        location: "file:///bin/foo".to_string(),
        version: "v1".to_string(),
        checksum: String::new(),
        metadata: labels,
    };

    // Pre-register short-running task
    let mut sms_task = spear_next::proto::sms::Task::default();
    sms_task.task_id = "task-short-1".to_string();
    sms_task.name = "t-short".to_string();
    sms_task.version = "v1".to_string();
    sms_task.metadata = std::collections::HashMap::new();
    sms_task.config = std::collections::HashMap::new();
    sms_task.execution_kind = spear_next::proto::sms::TaskExecutionKind::ShortRunning as i32;
    sms_task.executable = Some(spear_next::proto::sms::TaskExecutable {
        r#type: 5,
        uri: "file:///bin/foo".to_string(),
        name: String::new(),
        checksum_sha256: String::new(),
        args: vec![],
        env: std::collections::HashMap::new(),
    });
    let artifact_arc = mgr.ensure_artifact_from_sms(&sms_task).await.unwrap();
    let _ = mgr
        .ensure_task_from_sms(&sms_task, &artifact_arc)
        .await
        .unwrap();

    let mut req2 = spear_next::proto::spearlet::InvokeFunctionRequest::default();
    req2.task_id = "task-short-1".to_string();
    req2.invocation_type = spear_next::proto::spearlet::InvocationType::ExistingTask as i32;
    req2.artifact_spec = Some(artifact_spec.clone());
    let _ = mgr.submit_execution(req2).await.unwrap();
}
