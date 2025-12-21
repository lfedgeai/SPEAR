# Spearlet 架构重新设计

## 概述

基于用户需求，我们重新设计了 Spearlet 架构，明确了 Task 作为 spear-next 全局概念的定位，其信息存储在 SMS 中，而 Spearlet 中的 Task 实例是根据调度需求在本地存在的，支持一个 Task 在一个 Spearlet 中有多个实例以便于并行调度，这些实例共享同一个执行的 binary。

## 核心设计原则

### 1. 概念分离
- **全局 Task (SMS)**：任务的元数据、配置和全局状态管理
- **本地 TaskInstance (Spearlet)**：任务的具体执行实例，支持并行调度
- **共享 Binary**：多个实例共享同一个二进制执行文件

### 2. 架构层次
```
SMS (全局层)
├── Task 定义和元数据
├── 全局调度策略
└── 跨节点协调

Spearlet (本地层)
├── TaskInstance 管理
├── Binary 共享机制
├── 本地调度优化
└── 资源生命周期管理
```

## 核心组件设计

### 1. Task 引用层 (TaskRef)

```rust
/// 任务引用 - SMS 中全局 Task 的本地引用
/// Task reference - Local reference to global Task in SMS
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TaskRef {
    /// 任务ID (全局唯一) / Task ID (globally unique)
    pub task_id: String,
    /// 任务名称 / Task name
    pub name: String,
    /// 任务版本 / Task version
    pub version: String,
    /// 二进制规格 / Binary specification
    pub binary_spec: BinarySpec,
    /// 能力列表 / Capabilities list
    pub capabilities: Vec<String>,
    /// 资源需求 / Resource requirements
    pub resource_requirements: ResourceRequirements,
    /// 调度配置 / Scheduling configuration
    pub scheduling_config: SchedulingConfig,
    /// 本地缓存时间 / Local cache time
    pub cached_at: SystemTime,
    /// SMS 同步版本 / SMS sync version
    pub sync_version: u64,
}
```

### 2. 二进制共享层 (SharedBinary)

```rust
/// 共享二进制管理器
/// Shared binary manager
#[derive(Debug)]
pub struct BinaryShareManager {
    /// 已注册的二进制 / Registered binaries
    pub registered_binaries: DashMap<String, Arc<SharedBinary>>,
    /// 二进制加载器 / Binary loader
    pub loader: Arc<BinaryLoader>,
    /// 版本管理器 / Version manager
    pub version_manager: VersionManager,
    /// 监控器 / Monitor
    pub monitor: BinaryMonitor,
}

/// 共享二进制
/// Shared binary
#[derive(Debug)]
pub struct SharedBinary {
    /// 二进制规格 / Binary specification
    pub spec: BinarySpec,
    /// 运行时工厂 / Runtime factory
    pub runtime_factory: Arc<dyn RuntimeFactory>,
    /// 并发控制 / Concurrency control
    pub concurrency_control: Arc<ConcurrencyControl>,
    /// 性能统计 / Performance statistics
    pub performance_stats: Arc<RwLock<PerformanceStats>>,
    /// 引用计数 / Reference count
    pub ref_count: AtomicUsize,
}
```

### 3. 实例管理层 (TaskInstance)

```rust
/// 任务实例
/// Task instance
#[derive(Debug)]
pub struct TaskInstance {
    /// 实例ID (本地唯一) / Instance ID (locally unique)
    pub instance_id: String,
    /// 关联的任务引用 / Associated task reference
    pub task_ref: Arc<TaskRef>,
    /// 共享二进制引用 / Shared binary reference
    pub shared_binary: Arc<SharedBinary>,
    /// 运行时实例 / Runtime instance
    pub runtime: Box<dyn Runtime>,
    /// 实例状态 / Instance state
    pub state: Arc<RwLock<InstanceState>>,
    /// 性能指标 / Performance metrics
    pub metrics: Arc<InstanceMetrics>,
    /// 创建时间 / Creation time
    pub created_at: SystemTime,
    /// 最后活跃时间 / Last active time
    pub last_active: Arc<RwLock<SystemTime>>,
}

/// 实例状态
/// Instance state
#[derive(Debug, Clone, PartialEq)]
pub enum InstanceState {
    /// 初始化中 / Initializing
    Initializing,
    /// 预热中 / Warming up
    WarmingUp,
    /// 就绪 / Ready
    Ready,
    /// 执行中 / Executing
    Executing { request_id: String, function_name: String },
    /// 冷却中 / Cooling down
    CoolingDown,
    /// 暂停 / Paused
    Paused,
    /// 错误 / Error
    Error { error: String },
    /// 终止中 / Terminating
    Terminating,
    /// 已终止 / Terminated
    Terminated,
}
```

### 4. 调度管理层 (SpearletTaskManager)

```rust
/// Spearlet 任务管理器
/// Spearlet task manager
#[derive(Debug)]
pub struct SpearletTaskManager {
    /// 任务引用缓存 / Task reference cache
    pub task_cache: DashMap<String, Arc<TaskRef>>,
    /// 二进制共享管理器 / Binary share manager
    pub binary_manager: Arc<BinaryShareManager>,
    /// 实例池管理器 / Instance pool manager
    pub instance_pool: Arc<InstancePoolManager>,
    /// 智能调度器 / Intelligent scheduler
    pub scheduler: Arc<IntelligentScheduler>,
    /// SMS 同步器 / SMS synchronizer
    pub sms_sync: Arc<SmsSynchronizer>,
    /// 生命周期管理器 / Lifecycle manager
    pub lifecycle_manager: Arc<InstanceLifecycleManager>,
    /// 资源回收管理器 / Resource reclamation manager
    pub reclamation_manager: Arc<ResourceReclamationManager>,
}
```

## 核心机制设计

### 1. Binary 共享机制

#### 多层实例池
```rust
/// 实例池管理器
/// Instance pool manager
#[derive(Debug)]
pub struct InstancePoolManager {
    /// 热池 (立即可用) / Hot pool (immediately available)
    pub hot_pool: InstancePool,
    /// 温池 (快速启动) / Warm pool (quick start)
    pub warm_pool: InstancePool,
    /// 冷池 (需要初始化) / Cold pool (needs initialization)
    pub cold_pool: InstancePool,
    /// 池配置 / Pool configuration
    pub config: PoolConfig,
}
```

#### 智能分配策略
- **轮询分配 (Round Robin)**：均匀分配负载
- **最少连接 (Least Connections)**：选择当前负载最小的实例
- **加权轮询 (Weighted Round Robin)**：基于实例性能权重分配
- **负载感知 (Load Aware)**：基于实时负载指标动态选择
- **亲和性调度 (Affinity Scheduling)**：优先选择有数据亲和性的实例

### 2. 并行调度机制

#### 并发控制
```rust
/// 并发控制器
/// Concurrency controller
#[derive(Debug)]
pub struct ConcurrencyControl {
    /// 全局并发限制 / Global concurrency limit
    pub global_limiter: Arc<Semaphore>,
    /// 二进制级并发限制 / Binary-level concurrency limit
    pub binary_limiters: DashMap<String, Arc<Semaphore>>,
    /// 函数级并发限制 / Function-level concurrency limit
    pub function_limiters: DashMap<String, Arc<Semaphore>>,
    /// 令牌桶限流器 / Token bucket rate limiter
    pub rate_limiter: Arc<TokenBucket>,
    /// 自适应限流器 / Adaptive rate limiter
    pub adaptive_limiter: Arc<AdaptiveLimiter>,
}
```

#### 智能调度策略
- **先来先服务 (FCFS)**：按请求到达顺序调度
- **最短作业优先 (SJF)**：优先调度预期执行时间短的任务
- **优先级调度 (Priority)**：基于任务优先级调度
- **负载均衡 (Load Balancing)**：动态平衡各实例负载
- **自适应调度 (Adaptive)**：基于历史性能数据动态调整

### 3. SMS 同步机制

#### 双向同步
```rust
/// SMS 同步器
/// SMS synchronizer
#[derive(Debug)]
pub struct SmsSynchronizer {
    /// 任务同步管理器 / Task sync manager
    pub task_sync: Arc<TaskSyncManager>,
    /// 状态报告器 / Status reporter
    pub status_reporter: Arc<InstanceStatusReporter>,
    /// 事件发布器 / Event publisher
    pub event_publisher: Arc<EventPublisher>,
    /// 配置同步器 / Configuration synchronizer
    pub config_sync: Arc<ConfigSynchronizer>,
}
```

#### 同步策略
- **增量同步**：只同步变更的数据，提高效率
- **全量同步**：定期进行完整同步，确保一致性
- **事件驱动**：基于事件的实时同步
- **批量报告**：批量上报状态，减少网络开销

### 4. 生命周期管理

#### 状态机设计
```
Initializing → WarmingUp → Ready → Executing → Ready
     ↓             ↓         ↓         ↓         ↓
   Error ←——————————————————————————————————————————
     ↓
Terminating → Terminated
```

#### 资源回收策略
- **基于空闲时间**：回收长时间空闲的实例
- **基于内存压力**：在内存不足时主动回收
- **基于负载**：根据系统负载动态调整实例数量
- **基于错误率**：回收频繁出错的实例
- **预测性回收**：基于机器学习预测未来需求

## 性能优化特性

### 1. 智能预热
- **预测性预热**：基于历史数据预测需求并提前预热
- **分层预热**：不同层次的预热策略
- **渐进式预热**：逐步增加实例数量

### 2. 自适应优化
- **动态参数调整**：根据运行时性能动态调整参数
- **负载预测**：基于历史数据预测未来负载
- **资源优化**：智能调整资源分配

### 3. 监控和告警
- **实时监控**：全面的性能指标监控
- **智能告警**：基于趋势分析的智能告警
- **性能分析**：深入的性能瓶颈分析

## 关键优势

### 1. 高性能
- **零拷贝共享**：多实例共享同一二进制，减少内存占用
- **智能调度**：多种调度策略优化响应时间
- **并行执行**：支持同一任务的多实例并行处理

### 2. 高可用
- **故障隔离**：实例级故障不影响其他实例
- **自动恢复**：智能的错误检测和恢复机制
- **优雅降级**：在资源不足时优雅降级服务

### 3. 高扩展性
- **动态扩缩容**：根据负载动态调整实例数量
- **水平扩展**：支持跨多个 Spearlet 节点扩展
- **插件化架构**：支持自定义调度策略和回收策略

### 4. 智能化
- **机器学习优化**：基于历史数据的智能决策
- **自适应调整**：系统自动学习和优化
- **预测性管理**：提前预测和处理潜在问题

## 实现路径

### 阶段一：核心架构
1. 实现 TaskRef 和 SharedBinary 基础结构
2. 构建基本的实例管理和生命周期
3. 实现简单的调度策略

### 阶段二：高级特性
1. 实现智能调度和并发控制
2. 构建 SMS 同步机制
3. 实现资源回收策略

### 阶段三：智能优化
1. 集成机器学习模型
2. 实现自适应优化
3. 完善监控和告警系统

## 总结

这个重新设计的架构清晰地分离了全局 Task 概念和本地 TaskInstance 实现，通过共享 Binary 机制实现了高效的资源利用，通过智能调度和生命周期管理实现了高性能的并行执行。整个架构具有高度的可扩展性和智能化特性，能够很好地满足 spear-next 的需求。