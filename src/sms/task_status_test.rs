//! Tests for UpdateTaskStatus RPC in SMS service
use tonic::Request;

use crate::config::base::StorageConfig;
use crate::proto::sms::{
    task_service_server::TaskService as TaskServiceTrait, GetTaskRequest, RegisterTaskRequest,
    TaskPriority, TaskStatus, UpdateTaskStatusRequest,
};
use crate::sms::service::SmsServiceImpl;
use uuid::Uuid;

async fn create_test_sms_service() -> SmsServiceImpl {
    let storage_config = StorageConfig {
        backend: "memory".to_string(),
        data_dir: "/tmp/test_sms".to_string(),
        max_cache_size_mb: 100,
        compression_enabled: false,
        pool_size: 10,
    };
    SmsServiceImpl::with_storage_config(&storage_config).await
}

#[tokio::test]
async fn test_update_task_status_active_then_inactive() {
    let sms_service = create_test_sms_service().await;

    let req = RegisterTaskRequest {
        name: "echo".to_string(),
        description: "simple echo".to_string(),
        priority: TaskPriority::Normal as i32,
        node_uuid: Uuid::new_v4().to_string(),
        endpoint: "http://localhost:8080/echo".to_string(),
        version: "1.0.0".to_string(),
        capabilities: vec!["echo".to_string()],
        metadata: std::collections::HashMap::new(),
        config: std::collections::HashMap::new(),
        executable: None,
    };
    let resp = TaskServiceTrait::register_task(&sms_service, Request::new(req))
        .await
        .unwrap();
    let task_id = resp.get_ref().task_id.clone();

    let update_req = UpdateTaskStatusRequest {
        task_id: task_id.clone(),
        status: TaskStatus::Registered as i32,
        node_uuid: Uuid::new_v4().to_string(),
        status_version: 1,
        updated_at: chrono::Utc::now().timestamp(),
        reason: "registered".to_string(),
    };
    let update_resp = TaskServiceTrait::update_task_status(&sms_service, Request::new(update_req))
        .await
        .unwrap();
    assert!(update_resp.get_ref().success);

    let get_resp = TaskServiceTrait::get_task(
        &sms_service,
        Request::new(GetTaskRequest {
            task_id: task_id.clone(),
        }),
    )
    .await
    .unwrap();
    let task = get_resp.get_ref().task.as_ref().unwrap();
    assert_eq!(task.status, TaskStatus::Registered as i32);

    let update_req2 = UpdateTaskStatusRequest {
        task_id: task_id.clone(),
        status: TaskStatus::Inactive as i32,
        node_uuid: Uuid::new_v4().to_string(),
        status_version: 2,
        updated_at: chrono::Utc::now().timestamp(),
        reason: "deactivate".to_string(),
    };
    let update_resp2 =
        TaskServiceTrait::update_task_status(&sms_service, Request::new(update_req2))
            .await
            .unwrap();
    assert!(update_resp2.get_ref().success);

    let get_resp2 = TaskServiceTrait::get_task(
        &sms_service,
        Request::new(GetTaskRequest {
            task_id: task_id.clone(),
        }),
    )
    .await
    .unwrap();
    let task2 = get_resp2.get_ref().task.as_ref().unwrap();
    assert_eq!(task2.status, TaskStatus::Inactive as i32);

    let update_req3 = UpdateTaskStatusRequest {
        task_id: task_id.clone(),
        status: TaskStatus::Active as i32,
        node_uuid: Uuid::new_v4().to_string(),
        status_version: 3,
        updated_at: chrono::Utc::now().timestamp(),
        reason: "activate".to_string(),
    };
    let update_resp3 =
        TaskServiceTrait::update_task_status(&sms_service, Request::new(update_req3))
            .await
            .unwrap();
    assert!(update_resp3.get_ref().success);

    let get_resp3 = TaskServiceTrait::get_task(
        &sms_service,
        Request::new(GetTaskRequest {
            task_id: task_id.clone(),
        }),
    )
    .await
    .unwrap();
    let task3 = get_resp3.get_ref().task.as_ref().unwrap();
    assert_eq!(task3.status, TaskStatus::Active as i32);
}

#[tokio::test]
async fn test_update_task_status_nonexistent_task() {
    let sms_service = create_test_sms_service().await;

    let update_req = UpdateTaskStatusRequest {
        task_id: Uuid::new_v4().to_string(),
        status: TaskStatus::Active as i32,
        node_uuid: Uuid::new_v4().to_string(),
        status_version: 1,
        updated_at: chrono::Utc::now().timestamp(),
        reason: "activate".to_string(),
    };
    let update_resp = TaskServiceTrait::update_task_status(&sms_service, Request::new(update_req))
        .await
        .unwrap();
    assert!(!update_resp.get_ref().success);
}

#[tokio::test]
async fn test_register_task_sets_registered_status() {
    let sms_service = create_test_sms_service().await;

    let req = RegisterTaskRequest {
        name: "echo".to_string(),
        description: "simple echo".to_string(),
        priority: TaskPriority::Normal as i32,
        node_uuid: Uuid::new_v4().to_string(),
        endpoint: "http://localhost:8080/echo".to_string(),
        version: "1.0.0".to_string(),
        capabilities: vec!["echo".to_string()],
        metadata: std::collections::HashMap::new(),
        config: std::collections::HashMap::new(),
        executable: None,
    };
    let resp = TaskServiceTrait::register_task(&sms_service, Request::new(req))
        .await
        .unwrap();
    let registered = resp.get_ref();
    assert!(registered.success);
    let task_id = registered.task_id.clone();

    let get_resp = TaskServiceTrait::get_task(
        &sms_service,
        Request::new(GetTaskRequest {
            task_id: task_id.clone(),
        }),
    )
    .await
    .unwrap();
    let task = get_resp.get_ref().task.as_ref().unwrap();
    assert_eq!(task.status, TaskStatus::Registered as i32);
}
