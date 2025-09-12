//! Task service implementation / 任务服务实现

use std::collections::HashMap;
use std::sync::Arc;
use tokio::sync::RwLock;

use crate::proto::sms::Task;
use crate::sms::error::{SmsError, SmsResult};

/// Task service for managing distributed tasks / 管理分布式任务的服务
#[derive(Debug, Clone)]
pub struct TaskService {
    /// In-memory storage for tasks / 任务的内存存储
    tasks: Arc<RwLock<HashMap<String, Task>>>,
}

impl TaskService {
    /// Create a new task service / 创建新的任务服务
    pub fn new() -> Self {
        Self {
            tasks: Arc::new(RwLock::new(HashMap::new())),
        }
    }

    /// Register a task / 注册任务
    pub async fn register_task(&mut self, task: Task) -> SmsResult<()> {
        let mut tasks = self.tasks.write().await;
        tasks.insert(task.task_id.clone(), task);
        Ok(())
    }

    /// Get a task by ID / 根据ID获取任务
    pub async fn get_task(&self, task_id: &str) -> SmsResult<Option<Task>> {
        let tasks = self.tasks.read().await;
        Ok(tasks.get(task_id).cloned())
    }

    /// List all tasks / 列出所有任务
    pub async fn list_tasks(&self) -> SmsResult<Vec<Task>> {
        let tasks = self.tasks.read().await;
        Ok(tasks.values().cloned().collect())
    }

    /// Update task heartbeat / 更新任务心跳
    pub async fn update_task_heartbeat(&mut self, task_id: &str, timestamp: i64) -> SmsResult<()> {
        let mut tasks = self.tasks.write().await;
        if let Some(task) = tasks.get_mut(task_id) {
            task.last_heartbeat = timestamp;
        }
        Ok(())
    }

    /// Remove a task / 移除任务
    pub async fn remove_task(&mut self, task_id: &str) -> SmsResult<bool> {
        let mut tasks = self.tasks.write().await;
        Ok(tasks.remove(task_id).is_some())
    }

    /// List tasks with filters / 使用过滤器列出任务
    pub async fn list_tasks_with_filters(
        &self,
        node_uuid: Option<&str>,
        status_filter: Option<i32>,
        priority_filter: Option<i32>,
        limit: Option<i32>,
        offset: Option<i32>,
    ) -> SmsResult<Vec<Task>> {
        let tasks = self.tasks.read().await;
        let mut filtered_tasks: Vec<Task> = tasks
            .values()
            .filter(|task| {
                // Filter by node UUID if specified / 如果指定则按节点UUID过滤
                if let Some(uuid) = node_uuid {
                    if !uuid.is_empty() && task.node_uuid != uuid {
                        return false;
                    }
                }
                
                // Filter by status if specified / 如果指定则按状态过滤
                if let Some(status) = status_filter {
                    if status >= 0 && task.status != status {
                        return false;
                    }
                }
                
                // Filter by priority if specified / 如果指定则按优先级过滤
                if let Some(priority) = priority_filter {
                    if priority >= 0 && task.priority != priority {
                        return false;
                    }
                }
                
                true
            })
            .cloned()
            .collect();
        
        // Apply offset and limit / 应用偏移量和限制
        let offset = offset.unwrap_or(0) as usize;
        let limit = limit.unwrap_or(100) as usize;
        
        if offset < filtered_tasks.len() {
            filtered_tasks = filtered_tasks.into_iter().skip(offset).take(limit).collect();
        } else {
            filtered_tasks.clear();
        }
        
        Ok(filtered_tasks)
    }

    /// List tasks by node / 根据节点列出任务
    pub async fn list_tasks_by_node(&self, node_uuid: &str) -> SmsResult<Vec<Task>> {
        let tasks = self.tasks.read().await;
        Ok(tasks
            .values()
            .filter(|task| task.node_uuid == node_uuid)
            .cloned()
            .collect())
    }
}

impl Default for TaskService {
    fn default() -> Self {
        Self::new()
    }
}