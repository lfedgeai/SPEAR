//! Artifact Implementation
//! Artifact 实现
//!
//! An Artifact represents a deployable unit that contains one or more tasks.
//! Artifact 表示包含一个或多个任务的可部署单元。

use super::{ExecutionError, ExecutionResult, TaskId, RuntimeType};
use crate::proto::spearlet::{ArtifactSpec as ProtoArtifactSpec, InvocationType as ProtoInvocationType};
use dashmap::DashMap;
use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::sync::atomic::{AtomicU64, Ordering};
use std::sync::Arc;
use std::time::{Duration, SystemTime, UNIX_EPOCH};
use uuid::Uuid;

/// Unique identifier for an artifact / Artifact 的唯一标识符
pub type ArtifactId = String;

/// Invocation type for artifacts / Artifact 调用类型
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub enum InvocationType {
    /// Unknown invocation type / 未知调用类型
    Unknown,
    /// New task invocation / 新任务调用
    NewTask,
    /// Existing task invocation / 现有任务调用
    ExistingTask,
}

impl From<ProtoInvocationType> for InvocationType {
    fn from(proto: ProtoInvocationType) -> Self {
        match proto {
            ProtoInvocationType::Unknown => InvocationType::Unknown,
            ProtoInvocationType::NewTask => InvocationType::NewTask,
            ProtoInvocationType::ExistingTask => InvocationType::ExistingTask,
        }
    }
}

impl From<InvocationType> for ProtoInvocationType {
    fn from(local: InvocationType) -> Self {
        match local {
            InvocationType::Unknown => ProtoInvocationType::Unknown,
            InvocationType::NewTask => ProtoInvocationType::NewTask,
            InvocationType::ExistingTask => ProtoInvocationType::ExistingTask,
        }
    }
}

/// Artifact status enumeration / Artifact 状态枚举
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub enum ArtifactStatus {
    /// Artifact is being created / 正在创建 Artifact
    Creating,
    /// Artifact is ready for execution / Artifact 准备就绪可执行
    Ready,
    /// Artifact is currently running / Artifact 正在运行
    Running,
    /// Artifact is paused / Artifact 已暂停
    Paused,
    /// Artifact is being stopped / 正在停止 Artifact
    Stopping,
    /// Artifact has stopped / Artifact 已停止
    Stopped,
    /// Artifact encountered an error / Artifact 遇到错误
    Error(String),
}

/// Artifact specification / Artifact 规格
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ArtifactSpec {
    /// Artifact name / Artifact 名称
    pub name: String,
    /// Artifact version / Artifact 版本
    pub version: String,
    /// Description / 描述
    pub description: Option<String>,
    /// Runtime type for this artifact / 此 Artifact 的运行时类型
    pub runtime_type: RuntimeType,
    /// Runtime-specific configuration / 运行时特定配置
    pub runtime_config: HashMap<String, serde_json::Value>,
    /// Artifact location (URI) / Artifact 位置（URI）
    pub location: Option<String>,
    /// SHA-256 checksum / SHA-256 校验和
    pub checksum_sha256: Option<String>,
    /// Environment variables / 环境变量
    pub environment: HashMap<String, String>,
    /// Resource limits / 资源限制
    pub resource_limits: ResourceLimits,
    /// Invocation type / 调用类型
    pub invocation_type: InvocationType,
    /// Maximum execution timeout in milliseconds / 最大执行超时时间（毫秒）
    pub max_execution_timeout_ms: u64,
    /// Labels for categorization / 分类标签
    pub labels: HashMap<String, String>,
}

/// Resource limits for an artifact / Artifact 的资源限制
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ResourceLimits {
    /// Maximum CPU cores (fractional values allowed) / 最大 CPU 核心数（允许小数值）
    pub max_cpu_cores: f64,
    /// Maximum memory in bytes / 最大内存字节数
    pub max_memory_bytes: u64,
    /// Maximum disk space in bytes / 最大磁盘空间字节数
    pub max_disk_bytes: u64,
    /// Maximum network bandwidth in bytes per second / 最大网络带宽（字节/秒）
    pub max_network_bps: u64,
}

impl Default for ResourceLimits {
    fn default() -> Self {
        Self {
            max_cpu_cores: 1.0,
            max_memory_bytes: 512 * 1024 * 1024, // 512MB
            max_disk_bytes: 1024 * 1024 * 1024,  // 1GB
            max_network_bps: 100 * 1024 * 1024,  // 100MB/s
        }
    }
}

/// Artifact metrics / Artifact 指标
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ArtifactMetrics {
    /// Total number of executions / 总执行次数
    pub total_executions: u64,
    /// Number of successful executions / 成功执行次数
    pub successful_executions: u64,
    /// Number of failed executions / 失败执行次数
    pub failed_executions: u64,
    /// Average execution time in milliseconds / 平均执行时间（毫秒）
    pub avg_execution_time_ms: f64,
    /// Current CPU usage percentage / 当前 CPU 使用率百分比
    pub cpu_usage_percent: f64,
    /// Current memory usage in bytes / 当前内存使用量（字节）
    pub memory_usage_bytes: u64,
    /// Last execution timestamp / 最后执行时间戳
    pub last_execution_time: Option<SystemTime>,
}

impl Default for ArtifactMetrics {
    fn default() -> Self {
        Self {
            total_executions: 0,
            successful_executions: 0,
            failed_executions: 0,
            avg_execution_time_ms: 0.0,
            cpu_usage_percent: 0.0,
            memory_usage_bytes: 0,
            last_execution_time: None,
        }
    }
}

/// Artifact implementation / Artifact 实现
#[derive(Debug)]
pub struct Artifact {
    /// Unique identifier / 唯一标识符
    pub id: ArtifactId,
    /// Artifact specification / Artifact 规格
    pub spec: ArtifactSpec,
    /// Current status / 当前状态
    pub status: Arc<parking_lot::RwLock<ArtifactStatus>>,
    /// Associated tasks / 关联的任务
    pub tasks: Arc<DashMap<TaskId, Arc<super::Task>>>,
    /// Artifact metrics / Artifact 指标
    pub metrics: Arc<parking_lot::RwLock<ArtifactMetrics>>,
    /// Creation timestamp / 创建时间戳
    pub created_at: SystemTime,
    /// Last updated timestamp / 最后更新时间戳
    pub updated_at: Arc<parking_lot::RwLock<SystemTime>>,
    /// Execution counter for generating unique execution IDs / 执行计数器用于生成唯一执行ID
    execution_counter: AtomicU64,
}

impl Artifact {
    /// Create a new artifact / 创建新的 Artifact
    pub fn new(spec: ArtifactSpec) -> Self {
        let id = format!("artifact-{}", Uuid::new_v4());
        let now = SystemTime::now();
        
        Self {
            id,
            spec,
            status: Arc::new(parking_lot::RwLock::new(ArtifactStatus::Creating)),
            tasks: Arc::new(DashMap::new()),
            metrics: Arc::new(parking_lot::RwLock::new(ArtifactMetrics::default())),
            created_at: now,
            updated_at: Arc::new(parking_lot::RwLock::new(now)),
            execution_counter: AtomicU64::new(0),
        }
    }

    /// Get artifact ID / 获取 Artifact ID
    pub fn id(&self) -> &str {
        &self.id
    }

    /// Get current status / 获取当前状态
    pub fn status(&self) -> ArtifactStatus {
        self.status.read().clone()
    }

    /// Update status / 更新状态
    pub fn set_status(&self, status: ArtifactStatus) {
        *self.status.write() = status;
        *self.updated_at.write() = SystemTime::now();
    }

    /// Add a task to this artifact / 向此 Artifact 添加任务
    pub fn add_task(&self, task: Arc<super::Task>) -> ExecutionResult<()> {
        // Verify runtime compatibility / 验证运行时兼容性
        if task.spec.runtime_type != self.spec.runtime_type {
            return Err(ExecutionError::InvalidConfiguration {
                message: format!(
                    "Task runtime type {:?} does not match artifact runtime type {:?}",
                    task.spec.runtime_type, self.spec.runtime_type
                ),
            });
        }

        self.tasks.insert(task.id.clone(), task);
        *self.updated_at.write() = SystemTime::now();
        Ok(())
    }

    /// Remove a task from this artifact / 从此 Artifact 移除任务
    pub fn remove_task(&self, task_id: &str) -> ExecutionResult<Arc<super::Task>> {
        self.tasks
            .remove(task_id)
            .map(|(_, task)| {
                *self.updated_at.write() = SystemTime::now();
                task
            })
            .ok_or_else(|| ExecutionError::TaskNotFound {
                id: task_id.to_string(),
            })
    }

    /// Get a task by ID / 根据 ID 获取任务
    pub fn get_task(&self, task_id: &str) -> Option<Arc<super::Task>> {
        self.tasks.get(task_id).map(|entry| entry.value().clone())
    }

    /// List all tasks / 列出所有任务
    pub fn list_tasks(&self) -> Vec<Arc<super::Task>> {
        self.tasks.iter().map(|entry| entry.value().clone()).collect()
    }

    /// Get the number of tasks in this artifact / 获取此 Artifact 中的任务数量
    pub fn task_count(&self) -> usize {
        self.tasks.len()
    }

    /// Generate a unique execution ID / 生成唯一的执行 ID
    pub fn generate_execution_id(&self) -> String {
        let counter = self.execution_counter.fetch_add(1, Ordering::SeqCst);
        let timestamp = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .unwrap_or_default()
            .as_millis();
        format!("{}-exec-{}-{}", self.id, timestamp, counter)
    }

    /// Update metrics / 更新指标
    pub fn update_metrics<F>(&self, updater: F)
    where
        F: FnOnce(&mut ArtifactMetrics),
    {
        let mut metrics = self.metrics.write();
        updater(&mut metrics);
        *self.updated_at.write() = SystemTime::now();
    }

    /// Get current metrics / 获取当前指标
    pub fn get_metrics(&self) -> ArtifactMetrics {
        self.metrics.read().clone()
    }

    /// Check if artifact is ready for execution / 检查 Artifact 是否准备就绪可执行
    pub fn is_ready(&self) -> bool {
        matches!(self.status(), ArtifactStatus::Ready | ArtifactStatus::Running)
    }

    /// Check if artifact can be modified / 检查 Artifact 是否可以修改
    pub fn is_modifiable(&self) -> bool {
        matches!(
            self.status(),
            ArtifactStatus::Creating | ArtifactStatus::Stopped | ArtifactStatus::Error(_)
        )
    }

    /// Get artifact age / 获取 Artifact 年龄
    pub fn age(&self) -> Duration {
        SystemTime::now()
            .duration_since(self.created_at)
            .unwrap_or_default()
    }

    /// Get time since last update / 获取自上次更新以来的时间
    pub fn time_since_update(&self) -> Duration {
        let last_update = *self.updated_at.read();
        SystemTime::now()
            .duration_since(last_update)
            .unwrap_or_default()
    }
}

impl From<ProtoArtifactSpec> for ArtifactSpec {
    fn from(proto: ProtoArtifactSpec) -> Self {
        let runtime_type = match proto.artifact_type.as_str() {
            "docker" => RuntimeType::Kubernetes, // Migrate Docker to Kubernetes / 将 Docker 迁移到 Kubernetes
            "kubernetes" => RuntimeType::Kubernetes,
            "process" => RuntimeType::Process,
            "wasm" => RuntimeType::Wasm,
            _ => RuntimeType::Process, // default fallback
        };
        
        Self {
            name: proto.artifact_id.clone(), // Use artifact_id as name / 使用 artifact_id 作为名称
            version: proto.version,
            description: None, // Proto doesn't have description field / Proto 没有 description 字段
            runtime_type,
            runtime_config: std::collections::HashMap::new(), // Proto doesn't have runtime_config / Proto 没有 runtime_config
            location: None,
            checksum_sha256: None,
            environment: std::collections::HashMap::new(), // Proto doesn't have environment / Proto 没有 environment
            resource_limits: ResourceLimits::default(), // TODO: Add to proto
            invocation_type: InvocationType::NewTask, // Default value / 默认值
            max_execution_timeout_ms: 30000, // Default timeout / 默认超时时间
            labels: proto.metadata, // Use metadata as labels / 使用 metadata 作为 labels
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_artifact_creation() {
        let spec = ArtifactSpec {
            name: "test-artifact".to_string(),
            version: "1.0.0".to_string(),
            description: Some("Test artifact".to_string()),
            runtime_type: RuntimeType::Kubernetes,
            runtime_config: HashMap::new(),
            location: None,
            checksum_sha256: None,
            environment: HashMap::new(),
            resource_limits: ResourceLimits::default(),
            invocation_type: InvocationType::NewTask,
            max_execution_timeout_ms: 30000,
            labels: HashMap::new(),
        };

        let artifact = Artifact::new(spec);
        assert!(artifact.id.starts_with("artifact-"));
        assert_eq!(artifact.status(), ArtifactStatus::Creating);
        assert!(artifact.tasks.is_empty());
    }

    #[test]
    fn test_execution_id_generation() {
        let spec = ArtifactSpec {
            name: "test-artifact".to_string(),
            version: "1.0.0".to_string(),
            description: None,
            runtime_type: RuntimeType::Kubernetes,
            runtime_config: HashMap::new(),
            location: None,
            checksum_sha256: None,
            environment: HashMap::new(),
            resource_limits: ResourceLimits::default(),
            invocation_type: InvocationType::NewTask,
            max_execution_timeout_ms: 30000,
            labels: HashMap::new(),
        };

        let artifact = Artifact::new(spec);
        let id1 = artifact.generate_execution_id();
        let id2 = artifact.generate_execution_id();
        
        assert_ne!(id1, id2);
        assert!(id1.contains(&artifact.id));
        assert!(id2.contains(&artifact.id));
    }
}
