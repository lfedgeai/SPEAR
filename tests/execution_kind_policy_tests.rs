use std::sync::Arc;

#[tokio::test]
async fn test_long_running_existing_task_invocation_allowed() {
    use async_trait::async_trait;
    use spear_next::spearlet::execution::instance;
    use spear_next::spearlet::execution::manager::{
        TaskExecutionManager, TaskExecutionManagerConfig,
    };
    use spear_next::spearlet::execution::runtime::{
        ExecutionContext as RtCtx, Runtime, RuntimeCapabilities, RuntimeExecutionResponse,
        RuntimeManager, RuntimeType,
    };
    use spear_next::spearlet::execution::DEFAULT_ENTRY_FUNCTION_NAME;

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

    let resp = mgr
        .submit_invocation(spear_next::proto::spearlet::InvokeRequest {
            invocation_id: "inv-long-1".to_string(),
            execution_id: "exec-long-1".to_string(),
            task_id: "task-long-1".to_string(),
            function_name: DEFAULT_ENTRY_FUNCTION_NAME.to_string(),
            input: None,
            headers: Default::default(),
            environment: Default::default(),
            timeout_ms: 0,
            session_id: String::new(),
            mode: spear_next::proto::spearlet::ExecutionMode::Sync as i32,
            force_new_instance: false,
            metadata: Default::default(),
        })
        .await
        .unwrap();
    assert_eq!(resp.execution_id, "exec-long-1");
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
    use spear_next::spearlet::execution::DEFAULT_ENTRY_FUNCTION_NAME;

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

    let _ = mgr
        .submit_invocation(spear_next::proto::spearlet::InvokeRequest {
            invocation_id: "inv-short-1".to_string(),
            execution_id: "exec-short-1".to_string(),
            task_id: "task-short-1".to_string(),
            function_name: DEFAULT_ENTRY_FUNCTION_NAME.to_string(),
            input: None,
            headers: Default::default(),
            environment: Default::default(),
            timeout_ms: 0,
            session_id: String::new(),
            mode: spear_next::proto::spearlet::ExecutionMode::Sync as i32,
            force_new_instance: false,
            metadata: Default::default(),
        })
        .await
        .unwrap();
}
