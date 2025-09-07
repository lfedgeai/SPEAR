// Task service implementation / 任务服务实现
// This module provides task management functionality including submission, monitoring, and control
// 此模块提供任务管理功能，包括提交、监控和控制

use std::sync::Arc;
use std::time::{SystemTime, UNIX_EPOCH};
use tonic::{Request, Response, Status};
use tracing::{debug, error, info};
use uuid::Uuid;

use crate::constants::{NO_STATUS_FILTER, NO_PRIORITY_FILTER};
use crate::proto::sms::{
    task_service_server::TaskService as TaskServiceTrait,
    Task, TaskStatus, TaskPriority,
    RegisterTaskRequest, RegisterTaskResponse,
    ListTasksRequest, ListTasksResponse,
    GetTaskRequest, GetTaskResponse,
    UnregisterTaskRequest, UnregisterTaskResponse,
};
use crate::services::error::SmsError;
use crate::storage::{KvStore, MemoryKvStore};

// Task service implementation / 任务服务实现
#[derive(Debug)]
pub struct TaskService {
    storage: Arc<dyn KvStore>,       // Task metadata storage / 任务元数据存储
}

impl TaskService {
    // Create a new task service / 创建新的任务服务
    pub fn new(storage: Arc<dyn KvStore>) -> Self {
        Self {
            storage,
        }
    }

    // Create task service with memory storage / 使用内存存储创建任务服务
    pub fn new_with_memory() -> Self {
        Self::new(Arc::new(MemoryKvStore::new()))
    }

    // Generate current timestamp / 生成当前时间戳
    fn current_timestamp() -> i64 {
        SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .unwrap_or_default()
            .as_secs() as i64
    }

    // Generate task storage key / 生成任务存储键
    fn task_key(task_id: &str) -> String {
        format!("task:{}", task_id)
    }

    // Store task in storage / 在存储中保存任务
    async fn store_task(&self, task: &Task) -> Result<(), SmsError> {
        let key = Self::task_key(&task.task_id);
        let value = serde_json::to_string(task)
            .map_err(|e| SmsError::Serialization(e.to_string()))?;
        
        self.storage.put(&key, &value.into_bytes()).await
            .map_err(|e| SmsError::StorageError(e.to_string()))?;
        
        Ok(())
    }

    // Retrieve task from storage / 从存储中检索任务
    async fn get_task(&self, task_id: &str) -> Result<Option<Task>, SmsError> {
        let key = Self::task_key(task_id);
        
        match self.storage.get(&key).await {
            Ok(Some(value)) => {
                let value_str = String::from_utf8(value)
                    .map_err(|e| SmsError::Serialization(e.to_string()))?;
                let task: Task = serde_json::from_str(&value_str)
                    .map_err(|e| SmsError::Serialization(e.to_string()))?;
                Ok(Some(task))
            }
            Ok(None) => Ok(None),
            Err(e) => Err(SmsError::StorageError(e.to_string())),
        }
    }

    // List all tasks with optional filtering / 列出所有任务（可选过滤）
    async fn list_tasks_internal(&self, node_uuid: Option<&str>, status_filter: Option<TaskStatus>, priority_filter: Option<TaskPriority>) -> Result<Vec<Task>, SmsError> {
        let prefix = "task:";
        let pairs = self.storage.scan_prefix(prefix).await?;

        let mut tasks = Vec::new();
        
        for pair in pairs {
            if let Ok(value_str) = String::from_utf8(pair.value) {
                if let Ok(task) = serde_json::from_str::<Task>(&value_str) {
                // Apply filters / 应用过滤器
                if let Some(node_filter) = node_uuid {
                    if task.node_uuid != node_filter {
                        continue;
                    }
                }
                
                if let Some(status) = status_filter {
                    if task.status != status as i32 {
                        continue;
                    }
                }
                
                if let Some(priority) = priority_filter {
                    if task.priority != priority as i32 {
                        continue;
                    }
                }
                
                    tasks.push(task);
                }
            }
        }
        
        // Sort by registration time (newest first) / 按注册时间排序（最新的在前）
        tasks.sort_by(|a, b| b.registered_at.cmp(&a.registered_at));
        
        Ok(tasks)
    }

    // Unregister a task / 注销任务
    async fn unregister_task_internal(&self, task_id: &str, reason: &str) -> Result<bool, SmsError> {
        debug!("Attempting to unregister task: {} (reason: {})", task_id, reason);
        
        // Check if task exists / 检查任务是否存在
        match self.get_task(task_id).await? {
            Some(mut task) => {
                // Update task status to unregistered / 更新任务状态为已注销
                task.status = TaskStatus::Unregistered as i32;
                self.store_task(&task).await?;
                
                info!("Task {} unregistered successfully", task_id);
                Ok(true)
            }
            None => {
                debug!("Task {} not found", task_id);
                Ok(false)
            }
        }
    }
}

#[tonic::async_trait]
impl TaskServiceTrait for TaskService {
    // Register a new task / 注册新任务
    async fn register_task(
        &self,
        request: Request<RegisterTaskRequest>,
    ) -> Result<Response<RegisterTaskResponse>, Status> {
        let req = request.into_inner();
        debug!("Registering new task: {}", req.name);

        // Generate task ID / 生成任务ID
        let task_id = Uuid::new_v4().to_string();
        let current_time = Self::current_timestamp();

        // Create task / 创建任务
        let task = Task {
            task_id: task_id.clone(),
            name: req.name,
            description: req.description,
            status: TaskStatus::Registered as i32,
            priority: req.priority,
            node_uuid: req.node_uuid,
            endpoint: req.endpoint,
            version: req.version,
            capabilities: req.capabilities,
            registered_at: current_time,
            last_heartbeat: current_time,
            metadata: req.metadata,
            config: req.config,
        };

        // Store task / 存储任务
        match self.store_task(&task).await {
            Ok(_) => {
                info!("Task registered successfully: {} ({})", task.name, task.task_id);

                Ok(Response::new(RegisterTaskResponse {
                    success: true,
                    message: "Task registered successfully".to_string(),
                    task_id,
                    task: Some(task),
                }))
            }
            Err(e) => {
                error!("Failed to store task: {}", e);
                Ok(Response::new(RegisterTaskResponse {
                    success: false,
                    message: format!("Failed to register task: {}", e),
                    task_id: String::new(),
                    task: None,
                }))
            }
        }
    }

    // List tasks with optional filtering / 列出任务（可选过滤）
    async fn list_tasks(
        &self,
        request: Request<ListTasksRequest>,
    ) -> Result<Response<ListTasksResponse>, Status> {
        let req = request.into_inner();
        debug!("Listing tasks with filters");

        let node_filter = if req.node_uuid.is_empty() { None } else { Some(req.node_uuid.as_str()) };
        let status_filter = if req.status_filter == NO_STATUS_FILTER { None } else { TaskStatus::try_from(req.status_filter).ok() };
        let priority_filter = if req.priority_filter == NO_PRIORITY_FILTER { None } else { TaskPriority::try_from(req.priority_filter).ok() };

        match self.list_tasks_internal(node_filter, status_filter, priority_filter).await {
            Ok(mut tasks) => {
                // Apply pagination / 应用分页
                let total_count = tasks.len() as i32;
                let offset = req.offset.max(0) as usize;
                let limit = if req.limit > 0 { req.limit as usize } else { tasks.len() };
                
                if offset < tasks.len() {
                    let end = (offset + limit).min(tasks.len());
                    tasks = tasks[offset..end].to_vec();
                } else {
                    tasks.clear();
                }

                Ok(Response::new(ListTasksResponse {
                    tasks,
                    total_count,
                }))
            }
            Err(e) => {
                error!("Failed to list tasks: {}", e);
                Err(Status::internal(format!("Failed to list tasks: {}", e)))
            }
        }
    }

    // Get specific task details / 获取特定任务详情
    async fn get_task(
        &self,
        request: Request<GetTaskRequest>,
    ) -> Result<Response<GetTaskResponse>, Status> {
        let req = request.into_inner();
        debug!("Getting task: {}", req.task_id);

        match self.get_task(&req.task_id).await {
            Ok(Some(task)) => {
                Ok(Response::new(GetTaskResponse {
                    found: true,
                    task: Some(task),
                }))
            }
            Ok(None) => {
                Ok(Response::new(GetTaskResponse {
                    found: false,
                    task: None,
                }))
            }
            Err(e) => {
                error!("Failed to get task {}: {}", req.task_id, e);
                Err(Status::internal(format!("Failed to get task: {}", e)))
            }
        }
    }

    // Unregister a task / 注销任务
    async fn unregister_task(
        &self,
        request: Request<UnregisterTaskRequest>,
    ) -> Result<Response<UnregisterTaskResponse>, Status> {
        let req = request.into_inner();
        info!("Unregistering task: {} (reason: {})", req.task_id, req.reason);

        match self.unregister_task_internal(&req.task_id, &req.reason).await {
            Ok(true) => {
                Ok(Response::new(UnregisterTaskResponse {
                    success: true,
                    message: "Task unregistered successfully".to_string(),
                    task_id: req.task_id,
                }))
            }
            Ok(false) => {
                Ok(Response::new(UnregisterTaskResponse {
                    success: false,
                    message: "Task not found".to_string(),
                    task_id: req.task_id,
                }))
            }
            Err(e) => {
                error!("Failed to unregister task {}: {}", req.task_id, e);
                Ok(Response::new(UnregisterTaskResponse {
                    success: false,
                    message: format!("Failed to unregister task: {}", e),
                    task_id: req.task_id,
                }))
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::collections::HashMap;

    #[tokio::test]
    async fn test_task_service_creation() {
        let service = TaskService::new_with_memory();
        // Service should be created successfully / 服务应该成功创建
    }

    #[tokio::test]
    async fn test_task_registration() {
        let service = TaskService::new_with_memory();
        
        let request = Request::new(RegisterTaskRequest {
            name: "test-task".to_string(),
            description: "Test task description".to_string(),
            priority: TaskPriority::Normal as i32,
            node_uuid: "test-node".to_string(),
            endpoint: "http://localhost:8080".to_string(),
            version: "1.0.0".to_string(),
            capabilities: vec!["compute".to_string(), "storage".to_string()],
            metadata: HashMap::new(),
            config: HashMap::new(),
        });

        let response = service.register_task(request).await.unwrap();
        let response = response.into_inner();
        
        assert!(response.success);
        assert!(!response.task_id.is_empty());
        assert!(response.task.is_some());
        
        let task = response.task.unwrap();
        assert_eq!(task.name, "test-task");
        assert_eq!(task.status, TaskStatus::Registered as i32);
    }

    #[tokio::test]
    async fn test_task_unregistration() {
        let service = TaskService::new_with_memory();
        
        // First register a task / 首先注册一个任务
        let register_request = Request::new(RegisterTaskRequest {
            name: "test-task".to_string(),
            description: "Test task description".to_string(),
            priority: TaskPriority::Normal as i32,
            node_uuid: "test-node".to_string(),
            endpoint: "http://localhost:8080".to_string(),
            version: "1.0.0".to_string(),
            capabilities: vec!["compute".to_string()],
            metadata: HashMap::new(),
            config: HashMap::new(),
        });

        let register_response = service.register_task(register_request).await.unwrap();
        let task_id = register_response.into_inner().task_id;

        // Then unregister it / 然后注销它
        let unregister_request = Request::new(UnregisterTaskRequest {
            task_id: task_id.clone(),
            reason: "Test unregistration".to_string(),
        });

        let unregister_response = service.unregister_task(unregister_request).await.unwrap();
        let response = unregister_response.into_inner();
        
        assert!(response.success);
        assert_eq!(response.task_id, task_id);
    }
}