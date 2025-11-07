# Spearlet Architecture Redesign

## Overview

Based on user requirements, we have redesigned the Spearlet architecture to clarify the positioning of Task as a global concept in spear-next, with information stored in SMS, while Task instances in Spearlet exist locally based on scheduling needs, supporting multiple instances of one Task in a Spearlet for parallel scheduling, with these instances sharing the same execution binary.

## Core Design Principles

### 1. Concept Separation
- **Global Task (SMS)**: Task metadata, configuration, and global state management
- **Local TaskInstance (Spearlet)**: Specific execution instances of tasks, supporting parallel scheduling
- **Shared Binary**: Multiple instances share the same binary execution file

### 2. Architecture Layers
```
SMS (Global Layer)
├── Task definitions and metadata
├── Global scheduling policies
└── Cross-node coordination

Spearlet (Local Layer)
├── TaskInstance management
├── Binary sharing mechanism
├── Local scheduling optimization
└── Resource lifecycle management
```

## Core Component Design

### 1. Task Reference Layer (TaskRef)

```rust
/// Task reference - Local reference to global Task in SMS
/// 任务引用 - SMS 中全局 Task 的本地引用
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TaskRef {
    /// Task ID (globally unique) / 任务ID (全局唯一)
    pub task_id: String,
    /// Task name / 任务名称
    pub name: String,
    /// Task version / 任务版本
    pub version: String,
    /// Binary specification / 二进制规格
    pub binary_spec: BinarySpec,
    /// Capabilities list / 能力列表
    pub capabilities: Vec<String>,
    /// Resource requirements / 资源需求
    pub resource_requirements: ResourceRequirements,
    /// Scheduling configuration / 调度配置
    pub scheduling_config: SchedulingConfig,
    /// Local cache time / 本地缓存时间
    pub cached_at: SystemTime,
    /// SMS sync version / SMS 同步版本
    pub sync_version: u64,
}
```

### 2. Binary Sharing Layer (SharedBinary)

```rust
/// Shared binary manager
/// 共享二进制管理器
#[derive(Debug)]
pub struct BinaryShareManager {
    /// Registered binaries / 已注册的二进制
    pub registered_binaries: DashMap<String, Arc<SharedBinary>>,
    /// Binary loader / 二进制加载器
    pub loader: Arc<BinaryLoader>,
    /// Version manager / 版本管理器
    pub version_manager: VersionManager,
    /// Monitor / 监控器
    pub monitor: BinaryMonitor,
}

/// Shared binary
/// 共享二进制
#[derive(Debug)]
pub struct SharedBinary {
    /// Binary specification / 二进制规格
    pub spec: BinarySpec,
    /// Runtime factory / 运行时工厂
    pub runtime_factory: Arc<dyn RuntimeFactory>,
    /// Concurrency control / 并发控制
    pub concurrency_control: Arc<ConcurrencyControl>,
    /// Performance statistics / 性能统计
    pub performance_stats: Arc<RwLock<PerformanceStats>>,
    /// Reference count / 引用计数
    pub ref_count: AtomicUsize,
}
```

### 3. Instance Management Layer (TaskInstance)

```rust
/// Task instance
/// 任务实例
#[derive(Debug)]
pub struct TaskInstance {
    /// Instance ID (locally unique) / 实例ID (本地唯一)
    pub instance_id: String,
    /// Associated task reference / 关联的任务引用
    pub task_ref: Arc<TaskRef>,
    /// Shared binary reference / 共享二进制引用
    pub shared_binary: Arc<SharedBinary>,
    /// Runtime instance / 运行时实例
    pub runtime: Box<dyn Runtime>,
    /// Instance state / 实例状态
    pub state: Arc<RwLock<InstanceState>>,
    /// Performance metrics / 性能指标
    pub metrics: Arc<InstanceMetrics>,
    /// Creation time / 创建时间
    pub created_at: SystemTime,
    /// Last active time / 最后活跃时间
    pub last_active: Arc<RwLock<SystemTime>>,
}

/// Instance state
/// 实例状态
#[derive(Debug, Clone, PartialEq)]
pub enum InstanceState {
    /// Initializing / 初始化中
    Initializing,
    /// Warming up / 预热中
    WarmingUp,
    /// Ready / 就绪
    Ready,
    /// Executing / 执行中
    Executing { request_id: String, function_name: String },
    /// Cooling down / 冷却中
    CoolingDown,
    /// Paused / 暂停
    Paused,
    /// Error / 错误
    Error { error: String },
    /// Terminating / 终止中
    Terminating,
    /// Terminated / 已终止
    Terminated,
}
```

### 4. Scheduling Management Layer (SpearletTaskManager)

```rust
/// Spearlet task manager
/// Spearlet 任务管理器
#[derive(Debug)]
pub struct SpearletTaskManager {
    /// Task reference cache / 任务引用缓存
    pub task_cache: DashMap<String, Arc<TaskRef>>,
    /// Binary share manager / 二进制共享管理器
    pub binary_manager: Arc<BinaryShareManager>,
    /// Instance pool manager / 实例池管理器
    pub instance_pool: Arc<InstancePoolManager>,
    /// Intelligent scheduler / 智能调度器
    pub scheduler: Arc<IntelligentScheduler>,
    /// SMS synchronizer / SMS 同步器
    pub sms_sync: Arc<SmsSynchronizer>,
    /// Lifecycle manager / 生命周期管理器
    pub lifecycle_manager: Arc<InstanceLifecycleManager>,
    /// Resource reclamation manager / 资源回收管理器
    pub reclamation_manager: Arc<ResourceReclamationManager>,
}
```

## Core Mechanism Design

### 1. Binary Sharing Mechanism

#### Multi-tier Instance Pools
```rust
/// Instance pool manager
/// 实例池管理器
#[derive(Debug)]
pub struct InstancePoolManager {
    /// Hot pool (immediately available) / 热池 (立即可用)
    pub hot_pool: InstancePool,
    /// Warm pool (quick start) / 温池 (快速启动)
    pub warm_pool: InstancePool,
    /// Cold pool (needs initialization) / 冷池 (需要初始化)
    pub cold_pool: InstancePool,
    /// Pool configuration / 池配置
    pub config: PoolConfig,
}
```

#### Intelligent Allocation Strategies
- **Round Robin**: Evenly distribute load
- **Least Connections**: Select instance with minimum current load
- **Weighted Round Robin**: Allocate based on instance performance weights
- **Load Aware**: Dynamically select based on real-time load metrics
- **Affinity Scheduling**: Prioritize instances with data affinity

### 2. Parallel Scheduling Mechanism

#### Concurrency Control
```rust
/// Concurrency controller
/// 并发控制器
#[derive(Debug)]
pub struct ConcurrencyControl {
    /// Global concurrency limit / 全局并发限制
    pub global_limiter: Arc<Semaphore>,
    /// Binary-level concurrency limit / 二进制级并发限制
    pub binary_limiters: DashMap<String, Arc<Semaphore>>,
    /// Function-level concurrency limit / 函数级并发限制
    pub function_limiters: DashMap<String, Arc<Semaphore>>,
    /// Token bucket rate limiter / 令牌桶限流器
    pub rate_limiter: Arc<TokenBucket>,
    /// Adaptive rate limiter / 自适应限流器
    pub adaptive_limiter: Arc<AdaptiveLimiter>,
}
```

#### Intelligent Scheduling Strategies
- **First Come First Served (FCFS)**: Schedule by request arrival order
- **Shortest Job First (SJF)**: Prioritize tasks with shorter expected execution time
- **Priority Scheduling**: Schedule based on task priority
- **Load Balancing**: Dynamically balance load across instances
- **Adaptive Scheduling**: Dynamically adjust based on historical performance data

### 3. SMS Synchronization Mechanism

#### Bidirectional Synchronization
```rust
/// SMS synchronizer
/// SMS 同步器
#[derive(Debug)]
pub struct SmsSynchronizer {
    /// Task sync manager / 任务同步管理器
    pub task_sync: Arc<TaskSyncManager>,
    /// Status reporter / 状态报告器
    pub status_reporter: Arc<InstanceStatusReporter>,
    /// Event publisher / 事件发布器
    pub event_publisher: Arc<EventPublisher>,
    /// Configuration synchronizer / 配置同步器
    pub config_sync: Arc<ConfigSynchronizer>,
}
```

#### Synchronization Strategies
- **Incremental Sync**: Only sync changed data for efficiency
- **Full Sync**: Periodic complete synchronization for consistency
- **Event-Driven**: Real-time synchronization based on events
- **Batch Reporting**: Batch status reporting to reduce network overhead

### 4. Lifecycle Management

#### State Machine Design
```
Initializing → WarmingUp → Ready → Executing → Ready
     ↓             ↓         ↓         ↓         ↓
   Error ←——————————————————————————————————————————
     ↓
Terminating → Terminated
```

#### Resource Reclamation Strategies
- **Idle Time Based**: Reclaim instances idle for extended periods
- **Memory Pressure Based**: Proactively reclaim when memory is insufficient
- **Load Based**: Dynamically adjust instance count based on system load
- **Error Rate Based**: Reclaim frequently failing instances
- **Predictive Reclamation**: Predict future demand using machine learning

## Performance Optimization Features

### 1. Intelligent Warmup
- **Predictive Warmup**: Predict demand based on historical data and warmup in advance
- **Tiered Warmup**: Different levels of warmup strategies
- **Progressive Warmup**: Gradually increase instance count

### 2. Adaptive Optimization
- **Dynamic Parameter Adjustment**: Adjust parameters based on runtime performance
- **Load Prediction**: Predict future load based on historical data
- **Resource Optimization**: Intelligently adjust resource allocation

### 3. Monitoring and Alerting
- **Real-time Monitoring**: Comprehensive performance metrics monitoring
- **Intelligent Alerting**: Smart alerts based on trend analysis
- **Performance Analysis**: In-depth performance bottleneck analysis

## Key Advantages

### 1. High Performance
- **Zero-Copy Sharing**: Multiple instances share the same binary, reducing memory usage
- **Intelligent Scheduling**: Multiple scheduling strategies optimize response time
- **Parallel Execution**: Support parallel processing of multiple instances of the same task

### 2. High Availability
- **Fault Isolation**: Instance-level failures don't affect other instances
- **Auto Recovery**: Intelligent error detection and recovery mechanisms
- **Graceful Degradation**: Gracefully degrade service when resources are insufficient

### 3. High Scalability
- **Dynamic Scaling**: Dynamically adjust instance count based on load
- **Horizontal Scaling**: Support scaling across multiple Spearlet nodes
- **Plugin Architecture**: Support custom scheduling and reclamation strategies

### 4. Intelligence
- **Machine Learning Optimization**: Intelligent decisions based on historical data
- **Adaptive Adjustment**: System automatically learns and optimizes
- **Predictive Management**: Proactively predict and handle potential issues

## Implementation Roadmap

### Phase 1: Core Architecture
1. Implement TaskRef and SharedBinary basic structures
2. Build basic instance management and lifecycle
3. Implement simple scheduling strategies

### Phase 2: Advanced Features
1. Implement intelligent scheduling and concurrency control
2. Build SMS synchronization mechanism
3. Implement resource reclamation strategies

### Phase 3: Intelligent Optimization
1. Integrate machine learning models
2. Implement adaptive optimization
3. Complete monitoring and alerting system

## Summary

This redesigned architecture clearly separates the global Task concept from local TaskInstance implementation, achieves efficient resource utilization through the shared Binary mechanism, and realizes high-performance parallel execution through intelligent scheduling and lifecycle management. The entire architecture has high scalability and intelligent features, which can well meet the needs of spear-next.