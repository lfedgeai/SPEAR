//! Task API Integration tests for SMS service
//! SMS服务的Task API集成测试
//!
//! These tests verify the end-to-end functionality of Task management REST API
//! 这些测试验证任务管理REST API的端到端功能

use axum_test::TestServer;
use serde_json;
use serde_json::json;
use uuid::Uuid;
use spear_next::http::{create_gateway_router, GatewayState};

// Test utilities for Task HTTP integration / Task HTTP集成测试工具
mod task_test_utils {
    use super::*;
    use std::net::SocketAddr;
    use std::time::Duration;
    use tokio::time::sleep;
    use tonic::transport::Server;
    use tokio::net::TcpListener;
    use spear_next::proto::sms::node_service_server::NodeServiceServer;
    use spear_next::proto::sms::task_service_server::TaskServiceServer;

    use std::sync::Once;

    static INIT: Once = Once::new();

    /// Initialize tracing for tests, only once / 为测试初始化tracing，只执行一次
    pub fn init_test_tracing() {
        INIT.call_once(|| {
            // Filter out noisy logs, only show warnings and errors / 过滤掉嘈杂的日志，只显示警告和错误
            let _ = tracing_subscriber::fmt()
                .with_env_filter("spear_next=warn,h2=warn,hyper=warn,tower=warn,axum=warn")
                .try_init();
        });
    }
    
    /// Create a test gRPC server / 创建测试gRPC服务器
    pub async fn create_test_grpc_server() -> (SocketAddr, tokio::task::JoinHandle<()>) {
        let listener = TcpListener::bind("127.0.0.1:0").await.unwrap();
        let addr = listener.local_addr().unwrap();
        
        let sms_service = spear_next::SmsServiceImpl::with_kv_config(60, spear_next::storage::KvStoreConfig::memory()).await;
        let task_service = spear_next::services::TaskService::new_with_memory();
        
        let handle = tokio::spawn(async move {
            Server::builder()
                .add_service(NodeServiceServer::new(sms_service))
                .add_service(TaskServiceServer::new(task_service))
                .serve_with_incoming(tokio_stream::wrappers::TcpListenerStream::new(listener))
                .await
                .unwrap();
        });
        
        // Wait for server to start / 等待服务器启动
        sleep(Duration::from_millis(500)).await;
        
        (addr, handle)
    }
    
    /// Create a test HTTP server / 创建测试HTTP服务器
    pub async fn create_test_http_server() -> (TestServer, tokio::task::JoinHandle<()>) {
        // Start a test gRPC server / 启动测试gRPC服务器
        let (grpc_addr, grpc_handle) = create_test_grpc_server().await;
        
        // Connect to the test gRPC server / 连接到测试gRPC服务器
        let channel = tonic::transport::Channel::from_shared(format!("http://{}", grpc_addr))
            .unwrap()
            .connect()
            .await
            .expect("Failed to connect to test gRPC server");
        
        let sms_client = spear_next::proto::sms::node_service_client::NodeServiceClient::new(channel.clone());
        let task_client = spear_next::proto::sms::task_service_client::TaskServiceClient::new(channel.clone());
        
        let state = GatewayState {
            node_client: sms_client,
            task_client,
        };
        
        // Create HTTP router / 创建HTTP路由器
        let app = create_gateway_router(state);
        let server = TestServer::new(app).unwrap();
        
        (server, grpc_handle)
    }
    
    /// Generate test task JSON / 生成测试任务JSON
    pub fn generate_test_task_json() -> serde_json::Value {
        json!({
            "name": "test-task",
            "description": "A test task for integration testing",
            "priority": "normal",
            "endpoint": "http://localhost:8080/task",
            "version": "1.0.0",
            "capabilities": ["compute", "storage"],
            "config": {
                "max_memory": "1GB",
                "timeout": "300s"
            }
        })
    }
}

#[tokio::test]
async fn test_task_lifecycle() {
    // Initialize tracing for debugging / 初始化tracing用于调试
    task_test_utils::init_test_tracing();
    
    // Create test server / 创建测试服务器
    let (server, _grpc_handle) = task_test_utils::create_test_http_server().await;
    
    // Test 1: Register a task / 测试1：注册任务
    let task_data = task_test_utils::generate_test_task_json();
    let response = server
        .post("/api/v1/tasks")
        .json(&task_data)
        .await;
    
    response.assert_status_ok();
    let register_result: serde_json::Value = response.json();
    assert!(register_result["task_id"].is_string());
    let task_id = register_result["task_id"].as_str().unwrap();
    
    // Test 2: Get task details / 测试2：获取任务详情
    let response = server
        .get(&format!("/api/v1/tasks/{}", task_id))
        .await;
    
    response.assert_status_ok();
    let task_details: serde_json::Value = response.json();
    assert_eq!(task_details["name"], "test-task");
    assert_eq!(task_details["description"], "A test task for integration testing");
    assert_eq!(task_details["priority"], "normal");
    assert_eq!(task_details["endpoint"], "http://localhost:8080/task");
    
    // Test 3: List tasks / 测试3：列出任务
    let response = server
        .get("/api/v1/tasks")
        .await;
    
    response.assert_status_ok();
    let tasks_list: serde_json::Value = response.json();
    assert!(tasks_list["tasks"].is_array());
    let tasks = tasks_list["tasks"].as_array().unwrap();
    assert!(tasks.len() > 0);
    
    // Verify our task is in the list / 验证我们的任务在列表中
    let found_task = tasks.iter().find(|t| t["task_id"] == task_id);
    assert!(found_task.is_some());
    
    // Test 4: Unregister task / 测试4：注销任务
    let unregister_params = json!({
        "reason": "Test unregister"
    });
    let response = server
        .delete(&format!("/api/v1/tasks/{}", task_id))
        .json(&unregister_params)
        .await;
    
    response.assert_status_ok();
    
    // Test 5: Verify task is unregistered / 测试5：验证任务已注销
    let response = server
        .get(&format!("/api/v1/tasks/{}", task_id))
        .await;
    
    // Task should either be not found or have unregistered status / 任务应该不存在或状态为已注销
    assert!(response.status_code() == 404 || {
        let task_details: serde_json::Value = response.json();
        task_details["status"] == "unregistered"
    });
}

#[tokio::test]
async fn test_task_list_with_filters() {
    // Initialize tracing for debugging / 初始化tracing用于调试
    task_test_utils::init_test_tracing();
    
    // Create test server / 创建测试服务器
    let (server, _grpc_handle) = task_test_utils::create_test_http_server().await;
    
    // Register multiple tasks / 注册多个任务
    let task1 = json!({
        "name": "task-1",
        "description": "First test task",
        "priority": "high",
        "endpoint": "http://localhost:8080/task1",
        "version": "1.0.0",
        "capabilities": ["compute"]
    });
    
    let task2 = json!({
        "name": "task-2", 
        "description": "Second test task",
        "priority": "low",
        "endpoint": "http://localhost:8080/task2",
        "version": "1.1.0",
        "capabilities": ["storage"]
    });
    
    // Register tasks / 注册任务
    let response1 = server.post("/api/v1/tasks").json(&task1).await;
    response1.assert_status_ok();
    
    let response2 = server.post("/api/v1/tasks").json(&task2).await;
    response2.assert_status_ok();
    
    // Wait a bit for tasks to be stored / 等待任务被存储
    tokio::time::sleep(std::time::Duration::from_millis(100)).await;
    
    // Test health check first to ensure server is working / 先测试健康检查确保服务器正常工作
    let health_response = server
        .get("/health")
        .await;
    
    health_response.assert_status_ok();
    
    // Test basic list first / 先测试基本列表功能
    let response = server
        .get("/api/v1/tasks")
        .await;
    
    println!("Response status: {}", response.status_code());
    response.assert_status_ok();
    
    // Test list with limit / 测试带限制的列表
    let response = server
        .get("/api/v1/tasks")
        .add_query_param("limit", "1")
        .await;
    
    response.assert_status_ok();
    let tasks_list: serde_json::Value = response.json();
    let tasks = tasks_list["tasks"].as_array().unwrap();
    assert_eq!(tasks.len(), 1);
    
    // Test list with offset / 测试带偏移的列表
    let response = server
        .get("/api/v1/tasks")
        .add_query_param("limit", "10")
        .add_query_param("offset", "0")
        .await;
    
    response.assert_status_ok();
    let tasks_list: serde_json::Value = response.json();
    let tasks = tasks_list["tasks"].as_array().unwrap();
    assert!(tasks.len() >= 2);
    
    // Test list with status filter / 测试状态过滤
    let response = server
        .get("/api/v1/tasks")
        .add_query_param("status", "registered")
        .await;
    
    response.assert_status_ok();
    
    // Test list with priority filter / 测试优先级过滤
    let response = server
        .get("/api/v1/tasks")
        .add_query_param("priority", "high")
        .await;
    
    response.assert_status_ok();
}

#[tokio::test]
async fn test_task_error_handling() {
    // Initialize tracing for debugging / 初始化tracing用于调试
    task_test_utils::init_test_tracing();
    
    // Create test server / 创建测试服务器
    let (server, _grpc_handle) = task_test_utils::create_test_http_server().await;
    
    // Test 1: Submit task with invalid data / 测试1：提交无效数据的任务
    let invalid_task = json!({
        "invalid_field": "invalid_value"
    });
    
    let response = server
        .post("/api/v1/tasks")
        .json(&invalid_task)
        .await;
    
    // Should return error for missing required fields / 应该为缺少必需字段返回错误
    assert!(response.status_code().is_client_error());
    
    // Test 2: Get non-existent task / 测试2：获取不存在的任务
    let fake_task_id = Uuid::new_v4().to_string();
    let response = server
        .get(&format!("/api/v1/tasks/{}", fake_task_id))
        .await;
    
    // Should return 404 / 应该返回404
    assert_eq!(response.status_code(), 404);
    
    // Test 3: Unregister non-existent task / 测试3：注销不存在的任务
    let response = server
        .delete(&format!("/api/v1/tasks/{}", fake_task_id))
        .await;
    
    // Should return error / 应该返回错误
    assert!(response.status_code().is_client_error());
}

#[tokio::test]
async fn test_task_sequential_operations() {
    // Initialize tracing for debugging / 初始化tracing用于调试
    task_test_utils::init_test_tracing();
    
    // Create test server / 创建测试服务器
    let (server, _grpc_handle) = task_test_utils::create_test_http_server().await;
    
    // Register multiple tasks sequentially / 顺序注册多个任务
    let mut task_ids = vec![];
    
    for i in 0..5 {
        let task_data = json!({
            "name": format!("sequential-task-{}", i),
            "description": format!("Sequential test task number {}", i),
            "priority": if i % 2 == 0 { "high" } else { "normal" },
            "endpoint": format!("http://localhost:8080/task{}", i),
            "version": "1.0.0",
            "capabilities": ["compute"]
        });
        
        let response = server
            .post("/api/v1/tasks")
            .json(&task_data)
            .await;
        
        response.assert_status_ok();
        let result: serde_json::Value = response.json();
        let task_id = result["task_id"].as_str().unwrap().to_string();
        task_ids.push(task_id);
    }
    
    // Verify all tasks were created / 验证所有任务都已创建
    assert_eq!(task_ids.len(), 5);
    
    // List all tasks and verify they exist / 列出所有任务并验证它们存在
    let response = server
        .get("/api/v1/tasks")
        .await;
    
    response.assert_status_ok();
    let tasks_list: serde_json::Value = response.json();
    let tasks = tasks_list["tasks"].as_array().unwrap();
    assert!(tasks.len() >= 5);
    
    // Verify all our task IDs are present / 验证所有任务ID都存在
    for task_id in &task_ids {
        let found = tasks.iter().any(|t| t["task_id"] == *task_id);
        assert!(found, "Task ID {} not found in list", task_id);
    }
}

#[tokio::test]
async fn test_task_content_types() {
    // Initialize tracing for debugging / 初始化tracing用于调试
    task_test_utils::init_test_tracing();
    
    // Create test server / 创建测试服务器
    let (server, _grpc_handle) = task_test_utils::create_test_http_server().await;
    
    // Test JSON content type / 测试JSON内容类型
    let task_data = task_test_utils::generate_test_task_json();
    let response = server
        .post("/api/v1/tasks")
        .content_type("application/json")
        .json(&task_data)
        .await;
    
    response.assert_status_ok();
    
    // Test that response is JSON / 测试响应是JSON格式
    let content_type = response.headers().get("content-type").unwrap();
    assert!(content_type.to_str().unwrap().contains("application/json"));
}