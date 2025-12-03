//! Function Service Integration Tests / 函数服务集成测试
//! 
//! This module contains integration tests for the function service,
//! testing the complete workflow from function invocation to execution.
//! 
//! 本模块包含函数服务的集成测试，
//! 测试从函数调用到执行的完整工作流程。

use std::collections::HashMap;
use std::time::Duration;
use tokio::time::sleep;
use uuid::Uuid;

use spear_next::proto::spearlet::{
    function_service_client::FunctionServiceClient,
    function_service_server::FunctionServiceServer,
    *,
};

mod test_utils {
    use super::*;
    use std::net::SocketAddr;
    use tonic::transport::Server;
    use tokio::net::TcpListener;
    use spear_next::spearlet::FunctionServiceImpl;
    use std::sync::Arc;
    use spear_next::spearlet::SpearletConfig;
    
    /// Create test function service / 创建测试函数服务
    pub async fn create_test_function_service() -> (SocketAddr, tokio::task::JoinHandle<()>) {
        let listener = TcpListener::bind("127.0.0.1:0").await.unwrap();
        let addr = listener.local_addr().unwrap();
        
        let function_service = FunctionServiceImpl::new(Arc::new(SpearletConfig::default())).await.unwrap();
        let service = FunctionServiceServer::new(function_service);
        
        let handle = tokio::spawn(async move {
            Server::builder()
                .add_service(service)
                .serve_with_incoming(tokio_stream::wrappers::TcpListenerStream::new(listener))
                .await
                .unwrap();
        });
        
        // Wait for server to start / 等待服务器启动
        sleep(Duration::from_millis(100)).await;
        
        (addr, handle)
    }
    
    /// Create test function client / 创建测试函数客户端
    pub async fn create_test_function_client(addr: SocketAddr) -> FunctionServiceClient<tonic::transport::Channel> {
        FunctionServiceClient::connect(format!("http://{}", addr)).await.unwrap()
    }
    
    /// Generate test artifact spec / 生成测试制品规范
    pub fn generate_test_artifact_spec() -> ArtifactSpec {
        ArtifactSpec {
            artifact_id: format!("test-artifact-{}", Uuid::new_v4()),
            artifact_type: "docker".to_string(),
            location: "test://example.py".to_string(),
            version: "1.0.0".to_string(),
            checksum: "sha256:abcd1234".to_string(),
            metadata: {
                let mut meta = HashMap::new();
                meta.insert("author".to_string(), "test".to_string());
                meta.insert("description".to_string(), "Test artifact".to_string());
                meta
            },
        }
    }
    
    /// Generate test function parameters / 生成测试函数参数
    pub fn generate_test_parameters() -> Vec<FunctionParameter> {
        vec![
            FunctionParameter {
                name: "input".to_string(),
                r#type: "string".to_string(),
                value: Some(prost_types::Any {
                    type_url: "type.googleapis.com/google.protobuf.StringValue".to_string(),
                    value: b"test input".to_vec(),
                }),
                required: true,
                description: "Test input parameter".to_string(),
            },
            FunctionParameter {
                name: "count".to_string(),
                r#type: "int32".to_string(),
                value: Some(prost_types::Any {
                    type_url: "type.googleapis.com/google.protobuf.Int32Value".to_string(),
                    value: vec![8, 10], // protobuf encoded int32 value 10
                }),
                required: false,
                description: "Test count parameter".to_string(),
            },
        ]
    }
    
    /// Generate test execution context / 生成测试执行上下文
    pub fn generate_test_context() -> Option<ExecutionContext> {
        Some(ExecutionContext {
            execution_id: Uuid::new_v4().to_string(),
            session_id: "test_session".to_string(),
            user_id: "test_user".to_string(),
            environment: {
                let mut env = HashMap::new();
                env.insert("ENV".to_string(), "test".to_string());
                env
            },
            headers: {
                let mut headers = HashMap::new();
                headers.insert("Content-Type".to_string(), "application/json".to_string());
                headers
            },
            timeout_ms: 30000,
            max_retries: 3,
        })
    }
}

#[tokio::test]
async fn test_function_service_health_check() {
    // Create test server / 创建测试服务器
    let (addr, _handle) = test_utils::create_test_function_service().await;
    let mut client = test_utils::create_test_function_client(addr).await;
    
    // Test health check / 测试健康检查
    let request = tonic::Request::new(GetHealthRequest {
        include_details: true,
    });
    let response = client.get_health(request).await.unwrap();
    let health_response = response.into_inner();
    
    assert_eq!(health_response.status, "健康 / Healthy");
    assert!(health_response.details.is_some());
    
    let details = health_response.details.unwrap();
    assert!(details.active_tasks >= 0);
    assert!(details.total_executions >= 0);
    assert!(health_response.uptime_seconds >= 0);
}

#[tokio::test]
async fn test_function_service_stats() {
    // Create test server / 创建测试服务器
    let (addr, _handle) = test_utils::create_test_function_service().await;
    let mut client = test_utils::create_test_function_client(addr).await;
    
    // Test stats retrieval / 测试统计信息获取
    let request = tonic::Request::new(GetStatsRequest {
        include_task_stats: true,
        include_execution_stats: true,
        time_range_hours: 24,
    });
    let response = client.get_stats(request).await.unwrap();
    let stats_response = response.into_inner();
    
    assert!(stats_response.service_stats.is_some());
    let service_stats = stats_response.service_stats.unwrap();
    assert!(service_stats.total_tasks >= 0);
    assert!(service_stats.total_executions >= 0);
    assert!(service_stats.uptime_seconds >= 0);
}

#[tokio::test]
async fn test_function_invocation_basic() {
    // Create test server / 创建测试服务器
    let (addr, _handle) = test_utils::create_test_function_service().await;
    let mut client = test_utils::create_test_function_client(addr).await;
    
    // Test basic function invocation / 测试基本函数调用
    let request = tonic::Request::new(InvokeFunctionRequest {
        invocation_type: InvocationType::NewTask as i32,
        task_name: "test-task".to_string(),
        task_description: "Test task description".to_string(),
        artifact_spec: Some(test_utils::generate_test_artifact_spec()),
        task_id: String::new(),
        function_name: "test_function".to_string(),
        parameters: test_utils::generate_test_parameters(),
        execution_mode: ExecutionMode::Sync as i32,
        context: test_utils::generate_test_context(),
        create_if_not_exists: true,
        force_new_instance: false,
        invocation_metadata: HashMap::new(),
    });
    
    let response = client.invoke_function(request).await;
    
    // For now, we expect this to fail gracefully since we don't have a real runtime
    // 目前，我们期望这会优雅地失败，因为我们没有真正的运行时
    assert!(response.is_err() || response.unwrap().into_inner().success == false);
}

#[tokio::test]
async fn test_execution_status_tracking() {
    // Create test server / 创建测试服务器
    let (addr, _handle) = test_utils::create_test_function_service().await;
    let mut client = test_utils::create_test_function_client(addr).await;
    
    // Test execution status tracking / 测试执行状态跟踪
    let request = tonic::Request::new(GetExecutionStatusRequest {
        execution_id: "non-existent-execution".to_string(),
        include_result: true,
        include_logs: true,
    });
    
    let response = client.get_execution_status(request).await.unwrap();
    let status_response = response.into_inner();
    
    // Should not find non-existent execution / 应该找不到不存在的执行
    assert!(!status_response.found);
}

#[tokio::test]
async fn test_task_management() {
    // Create test server / 创建测试服务器
    let (addr, _handle) = test_utils::create_test_function_service().await;
    let mut client = test_utils::create_test_function_client(addr).await;
    
    // Test task listing / 测试任务列表
    let request = tonic::Request::new(ListTasksRequest {
        prefix: String::new(),
        limit: 10,
        start_after: String::new(),
        include_details: true,
    });
    
    let response = client.list_tasks(request).await.unwrap();
    let tasks_response = response.into_inner();
    
    // Should return empty list initially / 初始应该返回空列表
    assert!(tasks_response.tasks.is_empty());
}

#[tokio::test]
async fn test_execution_cancellation() {
    // Create test server / 创建测试服务器
    let (addr, _handle) = test_utils::create_test_function_service().await;
    let mut client = test_utils::create_test_function_client(addr).await;
    
    // Test execution cancellation / 测试执行取消
    let request = tonic::Request::new(CancelExecutionRequest {
        execution_id: "non-existent-execution".to_string(),
        reason: "Test cancellation".to_string(),
        force: false,
    });
    
    let response = client.cancel_execution(request).await;
    
    // Should handle non-existent execution gracefully / 应该优雅地处理不存在的执行
    assert!(response.is_ok());
}

// Additional integration tests would go here...
// 其他集成测试将在这里...
