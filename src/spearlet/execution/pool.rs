//! Instance Pool
//! 实例池
//!
//! This module provides instance pooling and load balancing capabilities
//! for efficient resource management and request distribution.
//! 该模块提供实例池和负载均衡功能，用于高效的资源管理和请求分发。

use super::{
    instance::{InstanceId, InstanceStatus, TaskInstance},
    runtime::{Runtime, RuntimeType},
    scheduler::InstanceScheduler,
    task::{Task, TaskId},
    ExecutionError, ExecutionResult,
};
use dashmap::DashMap;
use parking_lot::RwLock;
use serde::{Deserialize, Serialize};
use std::collections::{HashMap, VecDeque};
use std::sync::atomic::{AtomicU64, Ordering};
use std::sync::Arc;
use std::time::{Duration, Instant, SystemTime};
use tokio::sync::{oneshot, Semaphore};
use tokio::time::{interval, timeout};
use tracing::{debug, info, warn};

/// Instance pool configuration / 实例池配置
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct InstancePoolConfig {
    /// Minimum instances per task / 每个任务的最小实例数
    pub min_instances_per_task: usize,
    /// Maximum instances per task / 每个任务的最大实例数
    pub max_instances_per_task: usize,
    /// Target utilization threshold / 目标利用率阈值
    pub target_utilization: f64,
    /// Scale up threshold / 扩容阈值
    pub scale_up_threshold: f64,
    /// Scale down threshold / 缩容阈值
    pub scale_down_threshold: f64,
    /// Scale up cooldown / 扩容冷却时间
    pub scale_up_cooldown_ms: u64,
    /// Scale down cooldown / 缩容冷却时间
    pub scale_down_cooldown_ms: u64,
    /// Instance warmup time / 实例预热时间
    pub instance_warmup_time_ms: u64,
    /// Health check interval / 健康检查间隔
    pub health_check_interval_ms: u64,
    /// Metrics collection interval / 指标收集间隔
    pub metrics_collection_interval_ms: u64,
    /// Instance idle timeout / 实例空闲超时
    pub instance_idle_timeout_ms: u64,
    /// Pool cleanup interval / 池清理间隔
    pub pool_cleanup_interval_ms: u64,
}

impl Default for InstancePoolConfig {
    fn default() -> Self {
        Self {
            min_instances_per_task: 1,
            max_instances_per_task: 10,
            target_utilization: 0.7,
            scale_up_threshold: 0.8,
            scale_down_threshold: 0.3,
            scale_up_cooldown_ms: 30000,          // 30 seconds
            scale_down_cooldown_ms: 60000,        // 60 seconds
            instance_warmup_time_ms: 10000,       // 10 seconds
            health_check_interval_ms: 15000,      // 15 seconds
            metrics_collection_interval_ms: 5000, // 5 seconds
            instance_idle_timeout_ms: 300000,     // 5 minutes
            pool_cleanup_interval_ms: 120000,     // 2 minutes
        }
    }
}

/// Pool metrics / 池指标
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PoolMetrics {
    /// Total instances / 总实例数
    pub total_instances: u64,
    /// Active instances / 活跃实例数
    pub active_instances: u64,
    /// Idle instances / 空闲实例数
    pub idle_instances: u64,
    /// Failed instances / 失败实例数
    pub failed_instances: u64,
    /// Average utilization / 平均利用率
    pub average_utilization: f64,
    /// Total requests handled / 处理的总请求数
    pub total_requests: u64,
    /// Average response time / 平均响应时间
    pub average_response_time_ms: f64,
    /// Scale up events / 扩容事件数
    pub scale_up_events: u64,
    /// Scale down events / 缩容事件数
    pub scale_down_events: u64,
    /// Pool efficiency / 池效率
    pub pool_efficiency: f64,
}

impl Default for PoolMetrics {
    fn default() -> Self {
        Self {
            total_instances: 0,
            active_instances: 0,
            idle_instances: 0,
            failed_instances: 0,
            average_utilization: 0.0,
            total_requests: 0,
            average_response_time_ms: 0.0,
            scale_up_events: 0,
            scale_down_events: 0,
            pool_efficiency: 0.0,
        }
    }
}

/// Scaling decision / 扩缩容决策
#[derive(Debug, Clone)]
pub struct ScalingDecision {
    /// Task ID / 任务 ID
    pub task_id: TaskId,
    /// Scaling action / 扩缩容动作
    pub action: ScalingAction,
    /// Target instance count / 目标实例数
    pub target_count: usize,
    /// Current instance count / 当前实例数
    pub current_count: usize,
    /// Decision reason / 决策原因
    pub reason: String,
    /// Decision timestamp / 决策时间戳
    pub timestamp: SystemTime,
}

/// Scaling action / 扩缩容动作
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub enum ScalingAction {
    /// Scale up / 扩容
    ScaleUp,
    /// Scale down / 缩容
    ScaleDown,
    /// No action / 无动作
    NoAction,
}

/// Task pool state / 任务池状态
#[derive(Debug)]
struct TaskPoolState {
    /// Task reference / 任务引用
    task: Arc<Task>,
    /// Active instances / 活跃实例
    instances: Vec<Arc<TaskInstance>>,
    /// Last scaling time / 最后扩缩容时间
    last_scale_up: Option<Instant>,
    last_scale_down: Option<Instant>,
    /// Metrics / 指标
    metrics: PoolMetrics,
    /// Request queue / 请求队列
    #[allow(dead_code)]
    request_queue: VecDeque<oneshot::Sender<Arc<TaskInstance>>>,
}

impl TaskPoolState {
    fn new(task: Arc<Task>) -> Self {
        Self {
            task,
            instances: Vec::new(),
            last_scale_up: None,
            last_scale_down: None,
            metrics: PoolMetrics::default(),
            request_queue: VecDeque::new(),
        }
    }

    fn add_instance(&mut self, instance: Arc<TaskInstance>) {
        self.instances.push(instance);
        self.update_metrics();
    }

    fn remove_instance(&mut self, instance_id: &InstanceId) -> bool {
        let initial_len = self.instances.len();
        self.instances.retain(|inst| inst.id() != instance_id);
        let removed = self.instances.len() != initial_len;
        if removed {
            self.update_metrics();
        }
        removed
    }

    fn get_available_instance(&self) -> Option<Arc<TaskInstance>> {
        self.instances
            .iter()
            .find(|instance| {
                instance.status() == InstanceStatus::Running
                    && instance.is_healthy()
                    && instance.is_ready()
                    && !instance.is_at_capacity()
            })
            .cloned()
    }

    fn calculate_utilization(&self) -> f64 {
        if self.instances.is_empty() {
            return 0.0;
        }

        let total_capacity: u32 = self
            .instances
            .iter()
            .map(|inst| inst.config.max_concurrent_requests)
            .sum();

        let total_active: u32 = self
            .instances
            .iter()
            .map(|inst| inst.get_metrics().active_requests)
            .sum();

        if total_capacity == 0 {
            0.0
        } else {
            total_active as f64 / total_capacity as f64
        }
    }

    fn update_metrics(&mut self) {
        let total = self.instances.len() as u64;
        let (active, idle, failed) = self.instances.iter().fold((0, 0, 0), |(a, i, f), inst| {
            let metrics = inst.get_metrics();
            match inst.status() {
                InstanceStatus::Running if metrics.active_requests > 0 => (a + 1, i, f),
                InstanceStatus::Running => (a, i + 1, f),
                InstanceStatus::Error(_) | InstanceStatus::Unhealthy => (a, i, f + 1),
                _ => (a, i, f),
            }
        });

        let utilization = self.calculate_utilization();
        let avg_response_time = if !self.instances.is_empty() {
            self.instances
                .iter()
                .map(|inst| inst.get_metrics().avg_request_time_ms)
                .sum::<f64>()
                / self.instances.len() as f64
        } else {
            0.0
        };

        let efficiency = if total > 0 {
            active as f64 / total as f64
        } else {
            0.0
        };

        self.metrics = PoolMetrics {
            total_instances: total,
            active_instances: active,
            idle_instances: idle,
            failed_instances: failed,
            average_utilization: utilization,
            average_response_time_ms: avg_response_time,
            pool_efficiency: efficiency,
            ..self.metrics
        };
    }

    fn should_scale_up(&self, config: &InstancePoolConfig, now: Instant) -> bool {
        if self.instances.len() >= config.max_instances_per_task {
            return false;
        }

        if let Some(last_scale_up) = self.last_scale_up {
            if now.duration_since(last_scale_up).as_millis() < config.scale_up_cooldown_ms as u128 {
                return false;
            }
        }

        self.calculate_utilization() > config.scale_up_threshold
    }

    fn should_scale_down(&self, config: &InstancePoolConfig, now: Instant) -> bool {
        if self.instances.len() <= config.min_instances_per_task {
            return false;
        }

        if let Some(last_scale_down) = self.last_scale_down {
            if now.duration_since(last_scale_down).as_millis()
                < config.scale_down_cooldown_ms as u128
            {
                return false;
            }
        }

        self.calculate_utilization() < config.scale_down_threshold
    }
}

/// Instance pool / 实例池
pub struct InstancePool {
    /// Configuration / 配置
    config: InstancePoolConfig,
    /// Task pools / 任务池
    pools: Arc<RwLock<HashMap<TaskId, TaskPoolState>>>,
    /// Instance scheduler / 实例调度器
    scheduler: Arc<InstanceScheduler>,
    /// Runtime registry / 运行时注册表
    runtimes: Arc<DashMap<RuntimeType, Arc<dyn Runtime>>>,
    /// Global metrics / 全局指标
    global_metrics: Arc<RwLock<PoolMetrics>>,
    /// Scaling semaphore / 扩缩容信号量
    scaling_semaphore: Arc<Semaphore>,
    /// Request counter / 请求计数器
    request_counter: AtomicU64,
    /// Shutdown signal / 关闭信号
    shutdown_sender: Option<oneshot::Sender<()>>,
}

impl InstancePool {
    /// Create a new instance pool / 创建新的实例池
    pub async fn new(
        config: InstancePoolConfig,
        scheduler: Arc<InstanceScheduler>,
    ) -> ExecutionResult<Arc<Self>> {
        let (shutdown_sender, shutdown_receiver) = oneshot::channel();

        let pool = Arc::new(Self {
            config: config.clone(),
            pools: Arc::new(RwLock::new(HashMap::new())),
            scheduler,
            runtimes: Arc::new(DashMap::new()),
            global_metrics: Arc::new(RwLock::new(PoolMetrics::default())),
            scaling_semaphore: Arc::new(Semaphore::new(1)), // Only one scaling operation at a time
            request_counter: AtomicU64::new(0),
            shutdown_sender: Some(shutdown_sender),
        });

        // Start background tasks / 启动后台任务
        let pool_clone = pool.clone();
        tokio::spawn(async move {
            pool_clone.run_scaling_loop(shutdown_receiver).await;
        });

        let pool_clone = pool.clone();
        tokio::spawn(async move {
            pool_clone.run_health_check_loop().await;
        });

        let pool_clone = pool.clone();
        tokio::spawn(async move {
            pool_clone.run_metrics_collection_loop().await;
        });

        let pool_clone = pool.clone();
        tokio::spawn(async move {
            pool_clone.run_cleanup_loop().await;
        });

        info!("InstancePool started with config: {:?}", config);
        Ok(pool)
    }

    /// Register runtime / 注册运行时
    pub fn register_runtime(&self, runtime_type: RuntimeType, runtime: Arc<dyn Runtime>) {
        self.runtimes.insert(runtime_type, runtime);
        info!("Registered runtime: {:?}", runtime_type);
    }

    /// Get instance for task / 为任务获取实例
    pub async fn get_instance(&self, task: &Arc<Task>) -> ExecutionResult<Arc<TaskInstance>> {
        let request_id = self.request_counter.fetch_add(1, Ordering::SeqCst);
        debug!(
            "Getting instance for task {} (request {})",
            task.id(),
            request_id
        );

        // Try to get existing instance / 尝试获取现有实例
        if let Some(instance) = self.get_available_instance(task).await? {
            return Ok(instance);
        }

        // Create new instance if needed / 如果需要则创建新实例
        self.ensure_minimum_instances(task).await?;

        // Try again after ensuring minimum instances / 确保最小实例数后再次尝试
        if let Some(instance) = self.get_available_instance(task).await? {
            return Ok(instance);
        }

        Err(ExecutionError::ResourceExhausted {
            message: format!("No available instances for task {}", task.id()),
        })
    }

    /// Get available instance from pool / 从池中获取可用实例
    async fn get_available_instance(
        &self,
        task: &Arc<Task>,
    ) -> ExecutionResult<Option<Arc<TaskInstance>>> {
        let pools = self.pools.read();
        if let Some(pool_state) = pools.get(task.id()) {
            Ok(pool_state.get_available_instance())
        } else {
            Ok(None)
        }
    }

    /// Ensure minimum instances for task / 确保任务的最小实例数
    async fn ensure_minimum_instances(&self, task: &Arc<Task>) -> ExecutionResult<()> {
        let current_count = {
            let pools = self.pools.read();
            pools
                .get(task.id())
                .map(|state| state.instances.len())
                .unwrap_or(0)
        };

        if current_count < self.config.min_instances_per_task {
            let needed = self.config.min_instances_per_task - current_count;
            for _ in 0..needed {
                self.create_instance(task).await?;
            }
        }

        Ok(())
    }

    /// Create new instance / 创建新实例
    async fn create_instance(&self, task: &Arc<Task>) -> ExecutionResult<Arc<TaskInstance>> {
        let runtime = self.runtimes.get(&task.spec.runtime_type).ok_or_else(|| {
            ExecutionError::RuntimeNotFound {
                runtime_type: format!("{:?}", task.spec.runtime_type),
            }
        })?;

        let instance_config = task.create_instance_config();
        let instance = timeout(
            Duration::from_millis(30000), // 30 second timeout
            runtime.create_instance(&instance_config),
        )
        .await
        .map_err(|_| ExecutionError::ExecutionTimeout { timeout_ms: 30000 })??;

        // Start the instance / 启动实例
        runtime.start_instance(&instance).await?;

        // Add to pool / 添加到池
        self.add_instance_to_pool(task, instance.clone()).await?;

        // Register with scheduler / 注册到调度器
        self.scheduler.add_instance(instance.clone()).await?;

        info!(
            "Created new instance {} for task {}",
            instance.id(),
            task.id()
        );
        Ok(instance)
    }

    /// Add instance to pool / 将实例添加到池
    async fn add_instance_to_pool(
        &self,
        task: &Arc<Task>,
        instance: Arc<TaskInstance>,
    ) -> ExecutionResult<()> {
        let mut pools = self.pools.write();
        let pool_state = (*pools)
            .entry(task.id().to_string())
            .or_insert_with(|| TaskPoolState::new(task.clone()));
        pool_state.add_instance(instance);
        Ok(())
    }

    /// Remove instance from pool / 从池中移除实例
    async fn remove_instance_from_pool(
        &self,
        task_id: &TaskId,
        instance_id: &InstanceId,
    ) -> ExecutionResult<()> {
        let mut pools = self.pools.write();
        if let Some(pool_state) = pools.get_mut(task_id) {
            pool_state.remove_instance(instance_id);

            // Remove empty pools / 移除空池
            if pool_state.instances.is_empty() {
                pools.remove(task_id);
            }
        }
        Ok(())
    }

    /// Scale task instances / 扩缩容任务实例
    async fn scale_task(&self, task_id: &TaskId, decision: ScalingDecision) -> ExecutionResult<()> {
        let _permit =
            self.scaling_semaphore
                .acquire()
                .await
                .map_err(|_| ExecutionError::RuntimeError {
                    message: "Failed to acquire scaling permit".to_string(),
                })?;

        match decision.action {
            ScalingAction::ScaleUp => {
                let task = {
                    let pools = self.pools.read();
                    pools.get(task_id).map(|state| state.task.clone())
                };

                if let Some(task) = task {
                    let instances_to_create = decision.target_count - decision.current_count;
                    for _ in 0..instances_to_create {
                        if let Err(e) = self.create_instance(&task).await {
                            warn!("Failed to create instance during scale up: {}", e);
                            break;
                        }
                    }

                    // Update scaling timestamp / 更新扩容时间戳
                    let mut pools = self.pools.write();
                    if let Some(pool_state) = pools.get_mut(task_id) {
                        pool_state.last_scale_up = Some(Instant::now());
                        pool_state.metrics.scale_up_events += 1;
                    }

                    info!(
                        "Scaled up task {} to {} instances",
                        task_id, decision.target_count
                    );
                }
            }
            ScalingAction::ScaleDown => {
                let instances_to_remove = decision.current_count - decision.target_count;
                let mut removed_count = 0;

                let instances_to_stop = {
                    let pools = self.pools.read();
                    if let Some(pool_state) = pools.get(task_id) {
                        pool_state
                            .instances
                            .iter()
                            .filter(|inst| inst.is_idle(Duration::from_secs(300))) // 5 minutes idle threshold
                            .take(instances_to_remove)
                            .cloned()
                            .collect::<Vec<_>>()
                    } else {
                        Vec::new()
                    }
                };

                for instance in instances_to_stop {
                    if let Some(runtime) = self.runtimes.get(&instance.config.runtime_type) {
                        if let Err(e) = runtime.stop_instance(&instance).await {
                            warn!("Failed to stop instance during scale down: {}", e);
                            continue;
                        }

                        self.remove_instance_from_pool(task_id, &instance.id().to_string())
                            .await?;
                        let instance_id = instance.id().to_string();
                        self.scheduler.remove_instance(&instance_id).await?;
                        removed_count += 1;
                    }
                }

                // Update scaling timestamp / 更新缩容时间戳
                let mut pools = self.pools.write();
                if let Some(pool_state) = pools.get_mut(task_id) {
                    pool_state.last_scale_down = Some(Instant::now());
                    pool_state.metrics.scale_down_events += 1;
                }

                info!(
                    "Scaled down task {} by {} instances",
                    task_id, removed_count
                );
            }
            ScalingAction::NoAction => {
                debug!("No scaling action needed for task {}", task_id);
            }
        }

        Ok(())
    }

    /// Get pool metrics for task / 获取任务的池指标
    pub fn get_task_metrics(&self, task_id: &TaskId) -> Option<PoolMetrics> {
        let pools = self.pools.read();
        pools.get(task_id).map(|state| state.metrics.clone())
    }

    /// Get global pool metrics / 获取全局池指标
    pub fn get_global_metrics(&self) -> PoolMetrics {
        self.global_metrics.read().clone()
    }

    /// List all tasks in pool / 列出池中的所有任务
    pub fn list_tasks(&self) -> Vec<TaskId> {
        let pools = self.pools.read();
        pools.keys().cloned().collect()
    }

    /// List instances for task / 列出任务的实例
    pub fn list_instances(&self, task_id: &TaskId) -> Vec<Arc<TaskInstance>> {
        let pools = self.pools.read();
        pools
            .get(task_id)
            .map(|state| state.instances.clone())
            .unwrap_or_default()
    }

    /// Shutdown the pool / 关闭池
    pub async fn shutdown(&mut self) -> ExecutionResult<()> {
        if let Some(sender) = self.shutdown_sender.take() {
            let _ = sender.send(());
        }

        // Stop all instances / 停止所有实例
        let all_instances: Vec<_> = {
            let pools = self.pools.read();
            pools
                .values()
                .flat_map(|state| state.instances.iter().cloned())
                .collect()
        };

        for instance in all_instances {
            if let Some(runtime) = self.runtimes.get(&instance.config.runtime_type) {
                if let Err(e) = runtime.stop_instance(&instance).await {
                    warn!(
                        "Failed to stop instance {} during shutdown: {}",
                        instance.id(),
                        e
                    );
                }
            }
        }

        info!("InstancePool shutdown completed");
        Ok(())
    }

    /// Scaling loop / 扩缩容循环
    async fn run_scaling_loop(&self, mut shutdown_receiver: oneshot::Receiver<()>) {
        let mut interval = interval(Duration::from_millis(10000)); // Check every 10 seconds

        loop {
            tokio::select! {
                _ = interval.tick() => {
                    self.check_scaling_decisions().await;
                }
                _ = &mut shutdown_receiver => {
                    info!("Scaling loop shutting down");
                    break;
                }
            }
        }
    }

    /// Check scaling decisions / 检查扩缩容决策
    async fn check_scaling_decisions(&self) {
        let now = Instant::now();
        let scaling_decisions: Vec<_> = {
            let pools = self.pools.read();
            pools
                .iter()
                .filter_map(|(task_id, state)| {
                    let current_count = state.instances.len();

                    if state.should_scale_up(&self.config, now) {
                        let target_count =
                            (current_count + 1).min(self.config.max_instances_per_task);
                        Some(ScalingDecision {
                            task_id: task_id.clone(),
                            action: ScalingAction::ScaleUp,
                            target_count,
                            current_count,
                            reason: format!(
                                "Utilization {} > threshold {}",
                                state.calculate_utilization(),
                                self.config.scale_up_threshold
                            ),
                            timestamp: SystemTime::now(),
                        })
                    } else if state.should_scale_down(&self.config, now) {
                        let target_count =
                            (current_count - 1).max(self.config.min_instances_per_task);
                        Some(ScalingDecision {
                            task_id: task_id.clone(),
                            action: ScalingAction::ScaleDown,
                            target_count,
                            current_count,
                            reason: format!(
                                "Utilization {} < threshold {}",
                                state.calculate_utilization(),
                                self.config.scale_down_threshold
                            ),
                            timestamp: SystemTime::now(),
                        })
                    } else {
                        None
                    }
                })
                .collect()
        };

        for decision in scaling_decisions {
            if let Err(e) = self.scale_task(&decision.task_id, decision.clone()).await {
                warn!("Failed to execute scaling decision: {}", e);
            }
        }
    }

    /// Health check loop / 健康检查循环
    async fn run_health_check_loop(&self) {
        let mut interval = interval(Duration::from_millis(self.config.health_check_interval_ms));

        loop {
            interval.tick().await;

            let all_instances: Vec<_> = {
                let pools = self.pools.read();
                pools
                    .values()
                    .flat_map(|state| state.instances.iter().cloned())
                    .collect()
            };

            for instance in all_instances {
                if let Some(runtime) = self.runtimes.get(&instance.config.runtime_type) {
                    if let Err(e) = runtime.health_check(&instance).await {
                        warn!("Health check failed for instance {}: {}", instance.id(), e);
                        instance.set_status(InstanceStatus::Error(e.to_string()));
                    }
                }
            }
        }
    }

    /// Metrics collection loop / 指标收集循环
    async fn run_metrics_collection_loop(&self) {
        let mut interval = interval(Duration::from_millis(
            self.config.metrics_collection_interval_ms,
        ));

        loop {
            interval.tick().await;
            self.update_global_metrics().await;
        }
    }

    /// Update global metrics / 更新全局指标
    async fn update_global_metrics(&self) {
        let pools = self.pools.read();

        let mut global_metrics = PoolMetrics::default();
        let mut total_utilization = 0.0;
        let mut total_response_time = 0.0;
        let mut pool_count = 0;

        for state in pools.values() {
            global_metrics.total_instances += state.metrics.total_instances;
            global_metrics.active_instances += state.metrics.active_instances;
            global_metrics.idle_instances += state.metrics.idle_instances;
            global_metrics.failed_instances += state.metrics.failed_instances;
            global_metrics.total_requests += state.metrics.total_requests;
            global_metrics.scale_up_events += state.metrics.scale_up_events;
            global_metrics.scale_down_events += state.metrics.scale_down_events;

            total_utilization += state.metrics.average_utilization;
            total_response_time += state.metrics.average_response_time_ms;
            pool_count += 1;
        }

        if pool_count > 0 {
            global_metrics.average_utilization = total_utilization / pool_count as f64;
            global_metrics.average_response_time_ms = total_response_time / pool_count as f64;
            global_metrics.pool_efficiency =
                global_metrics.active_instances as f64 / global_metrics.total_instances as f64;
        }

        *self.global_metrics.write() = global_metrics;
    }

    /// Cleanup loop / 清理循环
    async fn run_cleanup_loop(&self) {
        let mut interval = interval(Duration::from_millis(self.config.pool_cleanup_interval_ms));

        loop {
            interval.tick().await;

            let _now = SystemTime::now();
            let idle_timeout = Duration::from_millis(self.config.instance_idle_timeout_ms);

            let instances_to_cleanup: Vec<_> = {
                let pools = self.pools.read();
                pools
                    .values()
                    .flat_map(|state| {
                        state.instances.iter().filter_map(|instance| {
                            if instance.is_idle(idle_timeout) {
                                return Some((state.task.id().to_string(), instance.clone()));
                            }
                            None
                        })
                    })
                    .collect()
            };

            for (task_id, instance) in instances_to_cleanup {
                if let Some(runtime) = self.runtimes.get(&instance.config.runtime_type) {
                    if let Err(e) = runtime.stop_instance(&instance).await {
                        warn!("Failed to cleanup idle instance {}: {}", instance.id(), e);
                        continue;
                    }

                    let instance_id = instance.id().to_string();
                    if let Err(e) = self.remove_instance_from_pool(&task_id, &instance_id).await {
                        warn!("Failed to remove instance from pool: {}", e);
                    }

                    let instance_id = instance.id().to_string();
                    if let Err(e) = self.scheduler.remove_instance(&instance_id).await {
                        warn!("Failed to remove instance from scheduler: {}", e);
                    }

                    info!("Cleaned up idle instance: {}", instance.id());
                }
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::spearlet::execution::{
        artifact::{Artifact, ArtifactSpec, InvocationType},
        instance::{InstanceConfig, InstanceResourceLimits},
        scheduler::SchedulingPolicy,
        task::{HealthCheckConfig, ScalingConfig, TaskSpec, TaskType, TimeoutConfig},
    };

    fn create_test_task() -> Arc<Task> {
        let artifact_spec = ArtifactSpec {
            name: "test-artifact".to_string(),
            version: "1.0.0".to_string(),
            description: Some("Test artifact".to_string()),
            runtime_type: RuntimeType::Process,
            runtime_config: HashMap::new(),
            location: None,
            checksum_sha256: None,
            environment: HashMap::new(),
            resource_limits: Default::default(),
            invocation_type: crate::spearlet::execution::artifact::InvocationType::NewTask,
            max_execution_timeout_ms: 30000,
            labels: HashMap::new(),
        };
        let task_spec = TaskSpec {
            name: artifact_spec.name.clone(),
            task_type: TaskType::HttpHandler,
            runtime_type: artifact_spec.runtime_type,
            entry_point: "main".to_string(),
            handler_config: HashMap::new(),
            environment: artifact_spec.environment.clone(),
            invocation_type: artifact_spec.invocation_type,
            min_instances: 1,
            max_instances: 10,
            target_concurrency: 1,
            scaling_config: Default::default(),
            health_check: Default::default(),
            timeout_config: Default::default(),
            execution_kind: crate::spearlet::execution::task::ExecutionKind::ShortRunning,
        };
        let artifact_id = "test-artifact".to_string();
        Arc::new(Task::new(artifact_id, task_spec))
    }

    #[tokio::test]
    async fn test_instance_pool_creation() {
        let config = InstancePoolConfig::default();
        let scheduler = Arc::new(InstanceScheduler::new(SchedulingPolicy::RoundRobin));

        let pool = InstancePool::new(config, scheduler).await;
        assert!(pool.is_ok());
    }

    #[test]
    fn test_pool_metrics() {
        let metrics = PoolMetrics::default();
        assert_eq!(metrics.total_instances, 0);
        assert_eq!(metrics.active_instances, 0);
        assert_eq!(metrics.idle_instances, 0);
        assert_eq!(metrics.failed_instances, 0);
        assert_eq!(metrics.average_utilization, 0.0);
        assert_eq!(metrics.total_requests, 0);
        assert_eq!(metrics.average_response_time_ms, 0.0);
        assert_eq!(metrics.scale_up_events, 0);
        assert_eq!(metrics.scale_down_events, 0);
        assert_eq!(metrics.pool_efficiency, 0.0);
    }

    #[test]
    fn test_scaling_decision() {
        let task_id = "test_task".to_string();
        let decision = ScalingDecision {
            task_id: task_id.clone(),
            action: ScalingAction::ScaleUp,
            target_count: 5,
            current_count: 3,
            reason: "High load detected".to_string(),
            timestamp: SystemTime::now(),
        };

        assert_eq!(decision.task_id, task_id);
        assert_eq!(decision.action, ScalingAction::ScaleUp);
        assert_eq!(decision.target_count, 5);
        assert_eq!(decision.current_count, 3);
    }

    #[test]
    fn test_task_pool_state() {
        let task = create_test_task();
        let state = TaskPoolState::new(task.clone());

        assert_eq!(state.task.id(), task.id());
        assert_eq!(state.instances.len(), 0);
        assert!(state.last_scale_up.is_none());
        assert!(state.last_scale_down.is_none());
    }
}
