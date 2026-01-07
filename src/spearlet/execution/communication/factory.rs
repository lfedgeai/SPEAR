//! Communication factory implementations
//! 通信工厂实现
//!
//! This module provides factory patterns for creating communication channels
//! and managing runtime-specific communication strategies.
//!
//! 此模块提供用于创建通信通道和管理运行时特定通信策略的工厂模式。

use std::collections::HashMap;
use std::sync::Arc;

use super::channel::{GrpcChannel, TcpChannel, UnixSocketChannel};
use super::{
    ChannelConfig, CommunicationChannel, CommunicationError, CommunicationResult, RuntimeInstanceId,
};
use crate::spearlet::execution::RuntimeType;

/// Communication strategy for different runtime types
/// 不同运行时类型的通信策略
#[derive(Debug, Clone)]
pub struct CommunicationStrategy {
    /// Runtime type this strategy applies to
    /// 此策略适用的运行时类型
    pub runtime_type: RuntimeType,

    /// Preferred channel type
    /// 首选通道类型
    pub preferred_channel: String,

    /// Fallback channel types
    /// 备用通道类型
    pub fallback_channels: Vec<String>,

    /// Default configuration
    /// 默认配置
    pub default_config: ChannelConfig,
}

impl CommunicationStrategy {
    /// Create a new communication strategy
    /// 创建新的通信策略
    pub fn new(
        runtime_type: RuntimeType,
        preferred_channel: String,
        fallback_channels: Vec<String>,
        default_config: ChannelConfig,
    ) -> Self {
        Self {
            runtime_type,
            preferred_channel,
            fallback_channels,
            default_config,
        }
    }

    /// Get all channel types for this strategy
    /// 获取此策略的所有通道类型
    pub fn get_all_channels(&self) -> Vec<String> {
        let mut channels = vec![self.preferred_channel.clone()];
        channels.extend(self.fallback_channels.clone());
        channels
    }
}

/// Communication factory for creating and managing channels
/// 用于创建和管理通道的通信工厂
pub struct CommunicationFactory {
    /// Runtime type strategies
    /// 运行时类型策略
    strategies: HashMap<RuntimeType, CommunicationStrategy>,

    /// Active channels by instance ID
    /// 按实例 ID 索引的活跃通道
    active_channels: HashMap<RuntimeInstanceId, Arc<dyn CommunicationChannel>>,

    /// Channel pool configuration
    /// 通道池配置
    pool_enabled: bool,
    #[allow(dead_code)]
    max_channels_per_instance: usize,
}

impl CommunicationFactory {
    /// Create a new communication factory with default strategies
    /// 使用默认策略创建新的通信工厂
    pub fn new() -> Self {
        let mut factory = Self {
            strategies: HashMap::new(),
            active_channels: HashMap::new(),
            pool_enabled: true,
            max_channels_per_instance: 10,
        };

        // Initialize default strategies / 初始化默认策略
        factory.init_default_strategies();
        factory
    }

    /// Create a new communication factory with custom pool settings
    /// 创建带有自定义池设置的新通信工厂
    pub fn with_pool_config(pool_enabled: bool, max_channels_per_instance: usize) -> Self {
        let mut factory = Self {
            strategies: HashMap::new(),
            active_channels: HashMap::new(),
            pool_enabled,
            max_channels_per_instance,
        };
        factory.init_default_strategies();
        factory
    }

    /// Initialize default communication strategies for each runtime type
    /// 为每种运行时类型初始化默认通信策略
    fn init_default_strategies(&mut self) {
        // Process runtime strategy / Process 运行时策略
        let process_strategy = CommunicationStrategy::new(
            RuntimeType::Process,
            "unix".to_string(),
            vec!["tcp".to_string()],
            ChannelConfig {
                channel_type: "unix".to_string(),
                address: "/tmp/spear-process.sock".to_string(),
                ..Default::default()
            },
        );
        self.strategies
            .insert(RuntimeType::Process, process_strategy);

        // Kubernetes runtime strategy / Kubernetes 运行时策略
        let k8s_strategy = CommunicationStrategy::new(
            RuntimeType::Kubernetes,
            "grpc".to_string(),
            vec!["tcp".to_string()],
            ChannelConfig {
                channel_type: "grpc".to_string(),
                address: "http://127.0.0.1:50051".to_string(),
                ..Default::default()
            },
        );
        self.strategies
            .insert(RuntimeType::Kubernetes, k8s_strategy);

        // WASM runtime strategy (in-process communication)
        // WASM 运行时策略（进程内通信）
        let wasm_strategy = CommunicationStrategy::new(
            RuntimeType::Wasm,
            "memory".to_string(),
            vec![],
            ChannelConfig {
                channel_type: "memory".to_string(),
                address: "in-process".to_string(),
                ..Default::default()
            },
        );
        self.strategies.insert(RuntimeType::Wasm, wasm_strategy);
    }

    /// Create or get a communication channel for the specified runtime instance
    /// 为指定的运行时实例创建或获取通信通道
    pub async fn get_or_create_channel(
        &mut self,
        instance_id: RuntimeInstanceId,
        custom_config: Option<ChannelConfig>,
    ) -> CommunicationResult<Arc<dyn CommunicationChannel>> {
        // Check if we already have an active channel for this instance
        // 检查是否已经有此实例的活跃通道
        if self.pool_enabled {
            if let Some(channel) = self.active_channels.get(&instance_id) {
                if channel.is_connected().await {
                    return Ok(channel.clone());
                } else {
                    // Remove disconnected channel
                    // 移除已断开的通道
                    self.active_channels.remove(&instance_id);
                }
            }
        }

        // Create new channel
        // 创建新通道
        let channel = self
            .create_channel_for_instance(instance_id.clone(), custom_config)
            .await?;

        // Store in pool if enabled
        // 如果启用了池，则存储到池中
        if self.pool_enabled {
            self.active_channels.insert(instance_id, channel.clone());
        }

        Ok(channel)
    }

    /// Create a new communication channel for the specified runtime instance
    /// 为指定的运行时实例创建新的通信通道
    pub async fn create_channel_for_instance(
        &self,
        instance_id: RuntimeInstanceId,
        custom_config: Option<ChannelConfig>,
    ) -> CommunicationResult<Arc<dyn CommunicationChannel>> {
        let strategy = self
            .strategies
            .get(&instance_id.runtime_type)
            .ok_or_else(|| CommunicationError::UnsupportedRuntime {
                runtime_type: instance_id.runtime_type.as_str().to_string(),
            })?;

        // Use custom config or create default config for this instance
        // 使用自定义配置或为此实例创建默认配置
        let config = custom_config.unwrap_or_else(|| {
            let mut default_config = strategy.default_config.clone();
            default_config.instance_id = instance_id.clone();
            // Generate instance-specific address
            // 生成实例特定的地址
            default_config.address = self.generate_instance_address(&instance_id, &default_config);
            default_config
        });

        // Try preferred channel first
        // 首先尝试首选通道
        if let Ok(channel) = self
            .create_channel_by_type(
                instance_id.clone(),
                &strategy.preferred_channel,
                config.clone(),
            )
            .await
        {
            return Ok(Arc::from(channel));
        }

        // Try fallback channels
        // 尝试备用通道
        for fallback_type in &strategy.fallback_channels {
            if let Ok(channel) = self
                .create_channel_by_type(instance_id.clone(), fallback_type, config.clone())
                .await
            {
                return Ok(Arc::from(channel));
            }
        }

        // All attempts failed
        // 所有尝试都失败了
        let mut attempted_types = vec![strategy.preferred_channel.clone()];
        attempted_types.extend(strategy.fallback_channels.clone());

        Err(CommunicationError::ChannelCreationFailed {
            runtime_type: instance_id.runtime_type.as_str().to_string(),
            attempted_types,
        })
    }

    /// Generate instance-specific address
    /// 生成实例特定的地址
    fn generate_instance_address(
        &self,
        instance_id: &RuntimeInstanceId,
        config: &ChannelConfig,
    ) -> String {
        match config.channel_type.as_str() {
            "unix" => {
                format!(
                    "/tmp/spear-{}-{}.sock",
                    instance_id.runtime_type.as_str(),
                    instance_id.instance_id
                )
            }
            "tcp" => {
                // For TCP, we might need to use different ports for different instances
                // 对于 TCP，我们可能需要为不同实例使用不同端口
                let base_port = 50051;
                let instance_hash = instance_id
                    .instance_id
                    .chars()
                    .map(|c| c as u32)
                    .sum::<u32>()
                    % 1000;
                format!("127.0.0.1:{}", base_port + instance_hash)
            }
            "grpc" => {
                let base_port = 50051;
                let instance_hash = instance_id
                    .instance_id
                    .chars()
                    .map(|c| c as u32)
                    .sum::<u32>()
                    % 1000;
                format!("http://127.0.0.1:{}", base_port + instance_hash)
            }
            _ => config.address.clone(),
        }
    }

    /// Create a channel by specific type
    /// 按特定类型创建通道
    async fn create_channel_by_type(
        &self,
        instance_id: RuntimeInstanceId,
        channel_type: &str,
        config: ChannelConfig,
    ) -> CommunicationResult<Box<dyn CommunicationChannel>> {
        match channel_type {
            "unix" => {
                let channel = UnixSocketChannel::new(instance_id, config)?;
                Ok(Box::new(channel) as Box<dyn CommunicationChannel>)
            }
            "tcp" => {
                let channel = TcpChannel::new(instance_id, config)?;
                Ok(Box::new(channel) as Box<dyn CommunicationChannel>)
            }
            "grpc" => {
                let channel = GrpcChannel::new(instance_id, config)?;
                Ok(Box::new(channel) as Box<dyn CommunicationChannel>)
            }
            "memory" => {
                // For WASM runtime, we might use a different in-memory channel
                // 对于 WASM 运行时，我们可能使用不同的内存通道
                // TODO: Implement MemoryChannel for WASM runtime
                // TODO: 为 WASM 运行时实现 MemoryChannel
                Err(CommunicationError::UnsupportedChannel {
                    channel_type: channel_type.to_string(),
                })
            }
            _ => Err(CommunicationError::UnsupportedChannel {
                channel_type: channel_type.to_string(),
            }),
        }
    }

    /// Close and remove a channel from the pool
    /// 关闭并从池中移除通道
    pub async fn close_channel(
        &mut self,
        instance_id: &RuntimeInstanceId,
    ) -> CommunicationResult<()> {
        if let Some(channel) = self.active_channels.remove(instance_id) {
            channel.close().await?;
        }
        Ok(())
    }

    /// Close all channels and clear the pool
    /// 关闭所有通道并清空池
    pub async fn close_all_channels(&mut self) -> CommunicationResult<()> {
        for (_, channel) in self.active_channels.drain() {
            let _ = channel.close().await; // Ignore individual close errors
        }
        Ok(())
    }

    /// Get the number of active channels
    /// 获取活跃通道数量
    pub fn active_channel_count(&self) -> usize {
        self.active_channels.len()
    }

    /// Get active channel instance IDs
    /// 获取活跃通道的实例 ID
    pub fn active_instances(&self) -> Vec<RuntimeInstanceId> {
        self.active_channels.keys().cloned().collect()
    }

    /// Check if a channel exists for the given instance
    /// 检查给定实例是否存在通道
    pub fn has_channel(&self, instance_id: &RuntimeInstanceId) -> bool {
        self.active_channels.contains_key(instance_id)
    }

    /// Register a custom communication strategy
    /// 注册自定义通信策略
    pub fn register_strategy(&mut self, strategy: CommunicationStrategy) {
        self.strategies.insert(strategy.runtime_type, strategy);
    }

    /// Get the communication strategy for a runtime type
    /// 获取运行时类型的通信策略
    pub fn get_strategy(&self, runtime_type: &RuntimeType) -> Option<&CommunicationStrategy> {
        self.strategies.get(runtime_type)
    }

    /// Get all supported runtime types
    /// 获取所有支持的运行时类型
    pub fn supported_runtimes(&self) -> Vec<RuntimeType> {
        self.strategies.keys().cloned().collect()
    }

    /// Validate if a channel type is supported for a runtime
    /// 验证运行时是否支持通道类型
    pub fn is_channel_supported(&self, runtime_type: &RuntimeType, channel_type: &str) -> bool {
        if let Some(strategy) = self.strategies.get(runtime_type) {
            strategy
                .get_all_channels()
                .contains(&channel_type.to_string())
        } else {
            false
        }
    }
}

impl Default for CommunicationFactory {
    fn default() -> Self {
        Self::new()
    }
}

/// Builder for creating custom communication strategies
/// 用于创建自定义通信策略的构建器
pub struct CommunicationStrategyBuilder {
    runtime_type: Option<RuntimeType>,
    preferred_channel: Option<String>,
    fallback_channels: Vec<String>,
    config: ChannelConfig,
}

impl CommunicationStrategyBuilder {
    /// Create a new strategy builder
    /// 创建新的策略构建器
    pub fn new() -> Self {
        Self {
            runtime_type: None,
            preferred_channel: None,
            fallback_channels: Vec::new(),
            config: ChannelConfig::default(),
        }
    }

    /// Set the runtime type
    /// 设置运行时类型
    pub fn runtime_type(mut self, runtime_type: RuntimeType) -> Self {
        self.runtime_type = Some(runtime_type);
        self
    }

    /// Set the preferred channel type
    /// 设置首选通道类型
    pub fn preferred_channel(mut self, channel_type: String) -> Self {
        self.preferred_channel = Some(channel_type);
        self
    }

    /// Add a fallback channel type
    /// 添加备用通道类型
    pub fn add_fallback_channel(mut self, channel_type: String) -> Self {
        self.fallback_channels.push(channel_type);
        self
    }

    /// Set the default configuration
    /// 设置默认配置
    pub fn config(mut self, config: ChannelConfig) -> Self {
        self.config = config;
        self
    }

    /// Build the communication strategy
    /// 构建通信策略
    pub fn build(self) -> CommunicationResult<CommunicationStrategy> {
        let runtime_type =
            self.runtime_type
                .ok_or_else(|| CommunicationError::InvalidConfiguration {
                    message: "Runtime type is required".to_string(),
                })?;

        let preferred_channel =
            self.preferred_channel
                .ok_or_else(|| CommunicationError::InvalidConfiguration {
                    message: "Preferred channel type is required".to_string(),
                })?;

        Ok(CommunicationStrategy::new(
            runtime_type,
            preferred_channel,
            self.fallback_channels,
            self.config,
        ))
    }
}

impl Default for CommunicationStrategyBuilder {
    fn default() -> Self {
        Self::new()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_communication_factory_creation() {
        let factory = CommunicationFactory::new();

        // Should have default strategies for all runtime types
        // 应该为所有运行时类型提供默认策略
        assert!(factory.get_strategy(&RuntimeType::Process).is_some());
        assert!(factory.get_strategy(&RuntimeType::Kubernetes).is_some());
        assert!(factory.get_strategy(&RuntimeType::Wasm).is_some());

        // Should start with empty channel pool
        // 应该从空的通道池开始
        assert_eq!(factory.active_channel_count(), 0);
        assert!(factory.active_instances().is_empty());
    }

    #[test]
    fn test_strategy_builder() {
        let strategy = CommunicationStrategyBuilder::new()
            .runtime_type(RuntimeType::Process)
            .preferred_channel("unix".to_string())
            .add_fallback_channel("tcp".to_string())
            .build()
            .unwrap();

        assert_eq!(strategy.runtime_type, RuntimeType::Process);
        assert_eq!(strategy.preferred_channel, "unix");
        assert_eq!(strategy.fallback_channels, vec!["tcp"]);
    }

    #[test]
    fn test_channel_support_validation() {
        let factory = CommunicationFactory::new();

        // Process runtime should support unix and tcp
        // Process 运行时应该支持 unix 和 tcp
        assert!(factory.is_channel_supported(&RuntimeType::Process, "unix"));
        assert!(factory.is_channel_supported(&RuntimeType::Process, "tcp"));
        assert!(!factory.is_channel_supported(&RuntimeType::Process, "grpc"));

        // Kubernetes runtime should support grpc and tcp
        // Kubernetes 运行时应该支持 grpc 和 tcp
        assert!(factory.is_channel_supported(&RuntimeType::Kubernetes, "grpc"));
        assert!(factory.is_channel_supported(&RuntimeType::Kubernetes, "tcp"));
        assert!(!factory.is_channel_supported(&RuntimeType::Kubernetes, "unix"));
    }

    #[test]
    fn test_supported_runtimes() {
        let factory = CommunicationFactory::new();
        let runtimes = factory.supported_runtimes();

        assert!(runtimes.contains(&RuntimeType::Process));
        assert!(runtimes.contains(&RuntimeType::Kubernetes));
        assert!(runtimes.contains(&RuntimeType::Wasm));
    }

    #[test]
    fn test_custom_strategy_registration() {
        let mut factory = CommunicationFactory::new();

        let custom_strategy = CommunicationStrategy::new(
            RuntimeType::Process,
            "custom".to_string(),
            vec!["fallback".to_string()],
            ChannelConfig::default(),
        );

        factory.register_strategy(custom_strategy);

        let strategy = factory.get_strategy(&RuntimeType::Process).unwrap();
        assert_eq!(strategy.preferred_channel, "custom");
        assert_eq!(strategy.fallback_channels, vec!["fallback"]);
    }

    #[test]
    fn test_instance_id_creation() {
        let instance_id =
            RuntimeInstanceId::new(RuntimeType::Process, "test-instance-1".to_string());

        assert_eq!(instance_id.runtime_type, RuntimeType::Process);
        assert_eq!(instance_id.instance_id, "test-instance-1");
        assert_eq!(instance_id.namespace, None);
        assert_eq!(instance_id.full_id(), "process:test-instance-1");
    }

    #[test]
    fn test_instance_id_with_namespace() {
        let instance_id = RuntimeInstanceId::with_namespace(
            RuntimeType::Kubernetes,
            "pod-123".to_string(),
            "default".to_string(),
        );

        assert_eq!(instance_id.runtime_type, RuntimeType::Kubernetes);
        assert_eq!(instance_id.instance_id, "pod-123");
        assert_eq!(instance_id.namespace, Some("default".to_string()));
        assert_eq!(instance_id.full_id(), "default:kubernetes:pod-123");
    }

    #[test]
    fn test_instance_address_generation() {
        let factory = CommunicationFactory::new();

        let instance_id = RuntimeInstanceId::new(RuntimeType::Process, "test-123".to_string());

        let config = ChannelConfig {
            instance_id: instance_id.clone(),
            channel_type: "unix".to_string(),
            address: "".to_string(),
            connect_timeout_ms: 5000,
            request_timeout_ms: 30000,
            keepalive_interval_ms: 10000,
            max_retries: 3,
            extra_config: HashMap::new(),
        };

        let address = factory.generate_instance_address(&instance_id, &config);
        assert_eq!(address, "/tmp/spear-process-test-123.sock");
    }

    #[test]
    fn test_channel_pool_management() {
        let mut factory = CommunicationFactory::new();

        let instance_id1 = RuntimeInstanceId::new(RuntimeType::Process, "instance-1".to_string());

        let instance_id2 = RuntimeInstanceId::new(RuntimeType::Process, "instance-2".to_string());

        // Initially no channels
        // 初始时没有通道
        assert_eq!(factory.active_channel_count(), 0);
        assert!(!factory.has_channel(&instance_id1));
        assert!(!factory.has_channel(&instance_id2));

        // Test instance tracking
        // 测试实例跟踪
        let instances = factory.active_instances();
        assert!(instances.is_empty());
    }
}
