//! Node management handler for SPEAR Metadata Server / SPEAR元数据服务器的节点管理处理器
//!
//! This module provides the NodeHandler which implements all node management operations
//! including registration, status updates, heartbeat monitoring, and cluster statistics.
//!
//! 此模块提供NodeHandler，实现所有节点管理操作，包括注册、状态更新、心跳监控和集群统计。

use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::sync::Arc;
use uuid::Uuid;
use crate::services::resource::{NodeResourceInfo, ResourceService};
use crate::storage::{KvStore, serialization, MemoryKvStore};
use crate::services::error::SmsError;

/// Namespace prefixes for KV storage / KV存储的命名空间前缀
pub mod namespace {
    /// Prefix for node data / 节点数据前缀
    pub const NODE_PREFIX: &str = "node:";
    /// Prefix for resource data / 资源数据前缀  
    pub const RESOURCE_PREFIX: &str = "resource:";
}

/// Helper functions for KV key generation / KV键生成的辅助函数
pub mod keys {
    use super::*;
    
    /// Generate node key / 生成节点键
    pub fn node_key(uuid: &Uuid) -> String {
        format!("{}{}", namespace::NODE_PREFIX, uuid)
    }
    
    /// Generate resource key / 生成资源键
    pub fn resource_key(uuid: &Uuid) -> String {
        format!("{}{}", namespace::RESOURCE_PREFIX, uuid)
    }
    
    /// Extract UUID from node key / 从节点键提取UUID
    pub fn extract_uuid_from_node_key(key: &str) -> Result<Uuid, SmsError> {
        if let Some(uuid_str) = key.strip_prefix(namespace::NODE_PREFIX) {
            Uuid::parse_str(uuid_str).map_err(|e| SmsError::Serialization(format!("Invalid UUID in node key: {}", e)))
        } else {
            Err(SmsError::Serialization("Invalid node key format".to_string()))
        }
    }
    
    /// Extract UUID from resource key / 从资源键提取UUID
    pub fn extract_uuid_from_resource_key(key: &str) -> Result<Uuid, SmsError> {
        if let Some(uuid_str) = key.strip_prefix(namespace::RESOURCE_PREFIX) {
            Uuid::parse_str(uuid_str).map_err(|e| SmsError::Serialization(format!("Invalid UUID in resource key: {}", e)))
        } else {
            Err(SmsError::Serialization("Invalid resource key format".to_string()))
        }
    }
}

/// Node status enumeration / 节点状态枚举
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub enum NodeStatus {
    /// Node is active and healthy / 节点活跃且健康
    Active,
    /// Node is inactive / 节点不活跃
    Inactive,
    /// Node is unhealthy / 节点不健康
    Unhealthy,
    /// Node is being decommissioned / 节点正在退役
    Decommissioning,
}

impl Default for NodeStatus {
    fn default() -> Self {
        NodeStatus::Active
    }
}

impl std::fmt::Display for NodeStatus {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            NodeStatus::Active => write!(f, "active"),
            NodeStatus::Inactive => write!(f, "inactive"),
            NodeStatus::Unhealthy => write!(f, "unhealthy"),
            NodeStatus::Decommissioning => write!(f, "decommissioning"),
        }
    }
}

impl std::str::FromStr for NodeStatus {
    type Err = String;
    
    fn from_str(s: &str) -> Result<Self, Self::Err> {
        match s.to_lowercase().as_str() {
            "active" => Ok(NodeStatus::Active),
            "inactive" => Ok(NodeStatus::Inactive),
            "unhealthy" => Ok(NodeStatus::Unhealthy),
            "decommissioning" => Ok(NodeStatus::Decommissioning),
            _ => Err(format!("Invalid node status: {}", s)),
        }
    }
}

/// Node information structure / 节点信息结构
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct NodeInfo {
    /// Unique identifier for the node / 节点唯一标识符
    pub uuid: Uuid,
    /// IP address of the node / 节点IP地址
    pub ip_address: String,
    /// Port number of the node / 节点端口号
    pub port: u16,
    /// Current status of the node / 节点当前状态
    pub status: NodeStatus,
    /// Last heartbeat timestamp / 最后心跳时间戳
    pub last_heartbeat: DateTime<Utc>,
    /// Registration timestamp / 注册时间戳
    pub registered_at: DateTime<Utc>,
    /// Additional metadata / 额外元数据
    pub metadata: HashMap<String, String>,
    /// Health information / 健康信息
    pub health_info: HashMap<String, String>,
}

impl NodeInfo {
    /// Create a new node info / 创建新的节点信息
    pub fn new(ip_address: String, port: u16) -> Self {
        let now = Utc::now();
        Self {
            uuid: Uuid::new_v4(),
            ip_address,
            port,
            status: NodeStatus::Active,
            last_heartbeat: now,
            registered_at: now,
            metadata: HashMap::new(),
            health_info: HashMap::new(),
        }
    }
    
    /// Update heartbeat timestamp / 更新心跳时间戳
    pub fn update_heartbeat(&mut self) {
        self.last_heartbeat = Utc::now();
    }
    
    /// Check if node is healthy based on timeout / 基于超时检查节点是否健康
    pub fn is_healthy(&self, timeout_seconds: u64) -> bool {
        let now = Utc::now();
        let timeout_duration = chrono::Duration::seconds(timeout_seconds as i64);
        now.signed_duration_since(self.last_heartbeat) <= timeout_duration
    }
    
    /// Get node address / 获取节点地址
    pub fn address(&self) -> String {
        format!("{}:{}", self.ip_address, self.port)
    }
    
    /// Update node metadata / 更新节点元数据
    pub fn update_metadata(&mut self, key: String, value: String) {
        self.metadata.insert(key, value);
    }
    
    /// Update health information / 更新健康信息
    pub fn update_health_info(&mut self, health_info: HashMap<String, String>) {
        self.health_info = health_info;
    }
}

/// Cluster statistics / 集群统计信息
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ClusterStats {
    /// Total number of nodes / 节点总数
    pub total_nodes: usize,
    /// Number of active nodes / 活跃节点数
    pub active_nodes: usize,
    /// Number of inactive nodes / 不活跃节点数
    pub inactive_nodes: usize,
    /// Number of unhealthy nodes / 不健康节点数
    pub unhealthy_nodes: usize,
    /// Number of nodes with resource information / 有资源信息的节点数
    pub nodes_with_resources: usize,
    /// Average CPU usage across all nodes / 所有节点的平均CPU使用率
    pub average_cpu_usage: f64,
    /// Average memory usage across all nodes / 所有节点的平均内存使用率
    pub average_memory_usage: f64,
    /// Total memory across all nodes / 所有节点的总内存
    pub total_memory_bytes: i64,
    /// Total used memory across all nodes / 所有节点的总已用内存
    pub total_used_memory_bytes: i64,
    /// Number of high load nodes / 高负载节点数
    pub high_load_nodes: usize,
}

/// Node management service / 节点管理服务
#[derive(Debug)]
pub struct NodeService {
    kv_store: Arc<dyn KvStore>,
    resource_service: ResourceService,
}

impl Default for NodeService {
    fn default() -> Self {
        Self::new_with_memory()
    }
}

impl NodeService {
    /// Create a new node service with memory backend / 创建使用内存后端的新节点服务
    pub fn new() -> Self {
        Self::new_with_memory()
    }
    
    /// Create a new node service with memory backend / 创建使用内存后端的新节点服务
    pub fn new_with_memory() -> Self {
        let kv_store = Arc::new(MemoryKvStore::new());
        Self {
            kv_store: kv_store.clone(),
            resource_service: ResourceService::with_kv_store(kv_store),
        }
    }
    
    /// Create a new node service with custom KV store / 创建使用自定义KV存储的新节点服务
    pub fn new_with_kv_store(kv_store: Box<dyn KvStore>) -> Self {
        let shared_kv_store: Arc<dyn KvStore> = Arc::from(kv_store);
        Self {
            kv_store: shared_kv_store.clone(),
            resource_service: ResourceService::with_kv_store(shared_kv_store),
        }
    }
    
    /// Register a new node / 注册新节点
    pub async fn register_node(&mut self, mut node: NodeInfo) -> Result<Uuid, SmsError> {
        let key = keys::node_key(&node.uuid);
        
        // Check if node already exists / 检查节点是否已存在
        if self.kv_store.get(&key).await?.is_some() {
            return Err(SmsError::Conflict { 
                message: format!("Node with UUID {} already exists", node.uuid) 
            });
        }
        
        node.update_heartbeat();
        let serialized = serialization::serialize(&node)?;
        self.kv_store.put(&key, &serialized).await?;
        
        Ok(node.uuid)
    }
    
    /// Update an existing node / 更新现有节点
    pub async fn update_node(&mut self, uuid: Uuid, updated_node: NodeInfo) -> Result<(), SmsError> {
        let key = keys::node_key(&uuid);
        
        // Check if node exists / 检查节点是否存在
        if self.kv_store.get(&key).await?.is_none() {
            return Err(SmsError::NodeNotFound { uuid: uuid.to_string() });
        }
        
        let serialized = serialization::serialize(&updated_node)?;
        self.kv_store.put(&key, &serialized).await?;
        
        Ok(())
    }
    
    /// Remove a node / 移除节点
    pub async fn remove_node(&mut self, uuid: &Uuid) -> Result<NodeInfo, SmsError> {
        let key = keys::node_key(uuid);
        
        let data = self.kv_store.get(&key).await?
            .ok_or_else(|| SmsError::NodeNotFound { uuid: uuid.to_string() })?;
        
        let node: NodeInfo = serialization::deserialize(&data)?;
        
        // Remove node data / 移除节点数据
        self.kv_store.delete(&key).await?;
        
        // Remove associated resource data / 移除关联的资源数据
        let _ = self.resource_service.remove_resource(uuid).await;
        
        Ok(node)
    }
    
    /// Get a node by UUID / 根据UUID获取节点
    pub async fn get_node(&self, uuid: &Uuid) -> Result<Option<NodeInfo>, SmsError> {
        let key = keys::node_key(uuid);
        
        if let Some(data) = self.kv_store.get(&key).await? {
            let node: NodeInfo = serialization::deserialize(&data)?;
            Ok(Some(node))
        } else {
            Ok(None)
        }
    }
    
    /// List all nodes / 列出所有节点
    pub async fn list_nodes(&self) -> Result<Vec<NodeInfo>, SmsError> {
        let keys = self.kv_store.keys_with_prefix(namespace::NODE_PREFIX).await?;
        let mut nodes = Vec::new();
        
        for key in keys {
            if let Some(data) = self.kv_store.get(&key).await? {
                let node: NodeInfo = serialization::deserialize(&data)?;
                nodes.push(node);
            }
        }
        
        Ok(nodes)
    }
    
    /// List nodes by status / 根据状态列出节点
    pub async fn list_nodes_by_status(&self, status: &NodeStatus) -> Result<Vec<NodeInfo>, SmsError> {
        let all_nodes = self.list_nodes().await?;
        Ok(all_nodes.into_iter().filter(|node| &node.status == status).collect())
    }
    
    /// Update node heartbeat / 更新节点心跳
    pub async fn update_heartbeat(&mut self, uuid: &Uuid, health_info: Option<HashMap<String, String>>) -> Result<(), SmsError> {
        if let Some(mut node) = self.get_node(uuid).await? {
            node.update_heartbeat();
            if let Some(health_info) = health_info {
                node.update_health_info(health_info);
            }
            self.update_node(*uuid, node).await?;
            Ok(())
        } else {
            Err(SmsError::NodeNotFound { uuid: uuid.to_string() })
        }
    }
    
    /// Cleanup unhealthy nodes / 清理不健康的节点
    pub async fn cleanup_unhealthy_nodes(&mut self, timeout_seconds: u64) -> Result<Vec<Uuid>, SmsError> {
        let all_nodes = self.list_nodes().await?;
        let mut removed_uuids = Vec::new();
        
        for node in all_nodes {
            if !node.is_healthy(timeout_seconds) {
                self.remove_node(&node.uuid).await?;
                removed_uuids.push(node.uuid);
            }
        }
        
        Ok(removed_uuids)
    }
    
    /// Get node count / 获取节点数量
    pub async fn node_count(&self) -> Result<usize, SmsError> {
        let keys = self.kv_store.keys_with_prefix(namespace::NODE_PREFIX).await?;
        Ok(keys.len())
    }
    
    /// Check if registry is empty / 检查注册表是否为空
    pub async fn is_empty(&self) -> Result<bool, SmsError> {
        Ok(self.node_count().await? == 0)
    }

    // Resource management methods / 资源管理方法
    
    /// Update node resource information / 更新节点资源信息
    pub async fn update_node_resource(&mut self, resource: NodeResourceInfo) -> Result<(), SmsError> {
        // Verify node exists / 验证节点存在
        if self.get_node(&resource.node_uuid).await?.is_none() {
            return Err(SmsError::NodeNotFound { uuid: resource.node_uuid.to_string() });
        }
        
        self.resource_service.update_resource(resource).await
    }

    /// Get node resource information / 获取节点资源信息
    pub async fn get_node_resource(&self, node_uuid: &Uuid) -> Result<Option<NodeResourceInfo>, SmsError> {
        self.resource_service.get_resource(node_uuid).await
    }

    /// Remove node resource information / 移除节点资源信息
    pub async fn remove_node_resource(&mut self, node_uuid: &Uuid) -> Result<Option<NodeResourceInfo>, SmsError> {
        self.resource_service.remove_resource(node_uuid).await
    }

    /// List all node resources / 列出所有节点资源
    pub async fn list_node_resources(&self) -> Result<Vec<NodeResourceInfo>, SmsError> {
        self.resource_service.list_resources().await
    }

    /// List resources for specific nodes / 列出特定节点的资源
    pub async fn list_resources_by_nodes(&self, node_uuids: &[Uuid]) -> Result<Vec<NodeResourceInfo>, SmsError> {
        self.resource_service.list_resources_by_nodes(node_uuids).await
    }

    /// List high load nodes / 列出高负载节点
    pub async fn list_high_load_nodes(&self) -> Result<Vec<NodeResourceInfo>, SmsError> {
        self.resource_service.list_high_load_nodes().await
    }

    /// Get node with its resource information / 获取节点及其资源信息
    pub async fn get_node_with_resource(&self, uuid: &Uuid) -> Result<Option<(NodeInfo, Option<NodeResourceInfo>)>, SmsError> {
        if let Some(node) = self.get_node(uuid).await? {
            let resource = self.get_node_resource(uuid).await?;
            Ok(Some((node, resource)))
        } else {
            Ok(None)
        }
    }

    /// Cleanup stale resource information / 清理过期的资源信息
    pub async fn cleanup_stale_resources(&mut self, max_age_seconds: u64) -> Result<Vec<Uuid>, SmsError> {
        self.resource_service.cleanup_stale_resources(max_age_seconds).await
    }

    /// Get resource count / 获取资源数量
    pub async fn resource_count(&self) -> Result<usize, SmsError> {
        self.resource_service.resource_count().await
    }

    /// Get cluster statistics / 获取集群统计信息
    pub async fn get_cluster_stats(&self) -> Result<ClusterStats, SmsError> {
        let nodes = self.list_nodes().await?;
        let resources = self.list_node_resources().await?;
        
        let total_nodes = nodes.len();
        let active_nodes = nodes.iter().filter(|n| n.status == NodeStatus::Active).count();
        let inactive_nodes = nodes.iter().filter(|n| n.status == NodeStatus::Inactive).count();
        let unhealthy_nodes = nodes.iter().filter(|n| n.status == NodeStatus::Unhealthy).count();
        
        let nodes_with_resources = resources.len();
        let average_cpu_usage = self.resource_service.get_average_cpu_usage().await?;
        let average_memory_usage = self.resource_service.get_average_memory_usage().await?;
        let total_memory_bytes = self.resource_service.get_total_memory_bytes().await?;
        let total_used_memory_bytes = self.resource_service.get_total_used_memory_bytes().await?;
        let high_load_nodes = self.list_high_load_nodes().await?.len();
        
        Ok(ClusterStats {
            total_nodes,
            active_nodes,
            inactive_nodes,
            unhealthy_nodes,
            nodes_with_resources,
            average_cpu_usage,
            average_memory_usage,
            total_memory_bytes,
            total_used_memory_bytes,
            high_load_nodes,
        })
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::thread;
    use std::time::Duration;

    #[test]
    fn test_node_status_default() {
        let status = NodeStatus::default();
        assert_eq!(status, NodeStatus::Active);
    }

    #[test]
    fn test_node_status_display() {
        assert_eq!(NodeStatus::Active.to_string(), "active");
        assert_eq!(NodeStatus::Inactive.to_string(), "inactive");
        assert_eq!(NodeStatus::Unhealthy.to_string(), "unhealthy");
        assert_eq!(NodeStatus::Decommissioning.to_string(), "decommissioning");
    }

    #[test]
    fn test_node_status_from_str() {
        assert_eq!("active".parse::<NodeStatus>().unwrap(), NodeStatus::Active);
        assert_eq!("inactive".parse::<NodeStatus>().unwrap(), NodeStatus::Inactive);
        assert_eq!("unhealthy".parse::<NodeStatus>().unwrap(), NodeStatus::Unhealthy);
        assert_eq!("decommissioning".parse::<NodeStatus>().unwrap(), NodeStatus::Decommissioning);
        assert!("invalid".parse::<NodeStatus>().is_err());
    }

    #[test]
    fn test_node_info_creation() {
        let node = NodeInfo::new("192.168.1.1".to_string(), 8080);
        assert_eq!(node.ip_address, "192.168.1.1");
        assert_eq!(node.port, 8080);
        assert_eq!(node.status, NodeStatus::Active);
        assert_eq!(node.address(), "192.168.1.1:8080");
        assert!(node.metadata.is_empty());
        assert!(node.health_info.is_empty());
    }

    #[test]
    fn test_node_info_heartbeat_update() {
        let mut node = NodeInfo::new("192.168.1.1".to_string(), 8080);
        let original_heartbeat = node.last_heartbeat;
        
        thread::sleep(Duration::from_millis(10));
        node.update_heartbeat();
        
        assert!(node.last_heartbeat > original_heartbeat);
    }

    #[test]
    fn test_node_info_health_check() {
        let mut node = NodeInfo::new("192.168.1.1".to_string(), 8080);
        
        // Fresh node should be healthy / 新节点应该是健康的
        assert!(node.is_healthy(60));
        
        // Simulate old heartbeat / 模拟旧心跳
        node.last_heartbeat = Utc::now() - chrono::Duration::seconds(120);
        assert!(!node.is_healthy(60));
    }

    #[test]
    fn test_node_info_metadata_update() {
        let mut node = NodeInfo::new("192.168.1.1".to_string(), 8080);
        
        node.update_metadata("region".to_string(), "us-west-1".to_string());
        node.update_metadata("zone".to_string(), "us-west-1a".to_string());
        
        assert_eq!(node.metadata.get("region"), Some(&"us-west-1".to_string()));
        assert_eq!(node.metadata.get("zone"), Some(&"us-west-1a".to_string()));
    }

    #[test]
    fn test_node_info_health_info_update() {
        let mut node = NodeInfo::new("192.168.1.1".to_string(), 8080);
        
        let mut health_info = HashMap::new();
        health_info.insert("cpu_usage".to_string(), "45.2".to_string());
        health_info.insert("memory_usage".to_string(), "67.8".to_string());
        
        node.update_health_info(health_info.clone());
        assert_eq!(node.health_info, health_info);
    }

    #[tokio::test]
    async fn test_node_service_creation() {
        let service = NodeService::new();
        assert!(service.is_empty().await.unwrap());
        assert_eq!(service.node_count().await.unwrap(), 0);
    }

    #[tokio::test]
    async fn test_node_service_register_node() {
        let mut service = NodeService::new();
        let node = NodeInfo::new("192.168.1.1".to_string(), 8080);
        let uuid = node.uuid;
        
        let result = service.register_node(node).await;
        assert!(result.is_ok());
        assert_eq!(result.unwrap(), uuid);
        assert_eq!(service.node_count().await.unwrap(), 1);
    }

    #[tokio::test]
    async fn test_node_service_duplicate_registration() {
        let mut service = NodeService::new();
        let node1 = NodeInfo::new("192.168.1.1".to_string(), 8080);
        let node2 = NodeInfo {
            uuid: node1.uuid, // Same UUID / 相同UUID
            ..NodeInfo::new("192.168.1.2".to_string(), 8081)
        };
        
        service.register_node(node1).await.unwrap();
        let result = service.register_node(node2).await;
        assert!(result.is_err());
        assert!(matches!(result.unwrap_err(), SmsError::Conflict { .. }));
    }

    #[tokio::test]
    async fn test_node_service_get_node() {
        let mut service = NodeService::new();
        let node = NodeInfo::new("192.168.1.1".to_string(), 8080);
        let uuid = node.uuid;
        
        // Node doesn't exist yet / 节点尚不存在
        assert!(service.get_node(&uuid).await.unwrap().is_none());
        
        // Register and retrieve / 注册并检索
        service.register_node(node.clone()).await.unwrap();
        let retrieved = service.get_node(&uuid).await.unwrap();
        assert!(retrieved.is_some());
        let retrieved_node = retrieved.unwrap();
        assert_eq!(retrieved_node.uuid, uuid);
        assert_eq!(retrieved_node.ip_address, "192.168.1.1");
    }

    #[tokio::test]
    async fn test_node_service_update_node() {
        let mut service = NodeService::new();
        let mut node = NodeInfo::new("192.168.1.1".to_string(), 8080);
        let uuid = node.uuid;
        
        service.register_node(node.clone()).await.unwrap();
        
        // Update node / 更新节点
        node.status = NodeStatus::Inactive;
        node.update_metadata("region".to_string(), "us-east-1".to_string());
        
        let result = service.update_node(uuid, node.clone()).await;
        assert!(result.is_ok());
        
        // Verify update / 验证更新
        let updated = service.get_node(&uuid).await.unwrap().unwrap();
        assert_eq!(updated.status, NodeStatus::Inactive);
        assert_eq!(updated.metadata.get("region"), Some(&"us-east-1".to_string()));
    }

    #[tokio::test]
    async fn test_node_service_remove_node() {
        let mut service = NodeService::new();
        let node = NodeInfo::new("192.168.1.1".to_string(), 8080);
        let uuid = node.uuid;
        
        service.register_node(node).await.unwrap();
        assert_eq!(service.node_count().await.unwrap(), 1);
        
        let removed = service.remove_node(&uuid).await.unwrap();
        assert_eq!(removed.uuid, uuid);
        assert_eq!(service.node_count().await.unwrap(), 0);
        assert!(service.get_node(&uuid).await.unwrap().is_none());
    }

    #[tokio::test]
    async fn test_node_service_list_nodes() {
        let mut service = NodeService::new();
        
        let node1 = NodeInfo::new("192.168.1.1".to_string(), 8080);
        let node2 = NodeInfo::new("192.168.1.2".to_string(), 8081);
        
        service.register_node(node1).await.unwrap();
        service.register_node(node2).await.unwrap();
        
        let nodes = service.list_nodes().await.unwrap();
        assert_eq!(nodes.len(), 2);
    }

    #[tokio::test]
    async fn test_node_service_list_nodes_by_status() {
        let mut service = NodeService::new();
        
        let mut node1 = NodeInfo::new("192.168.1.1".to_string(), 8080);
        let mut node2 = NodeInfo::new("192.168.1.2".to_string(), 8081);
        
        node1.status = NodeStatus::Active;
        node2.status = NodeStatus::Inactive;
        
        service.register_node(node1).await.unwrap();
        service.register_node(node2).await.unwrap();
        
        let active_nodes = service.list_nodes_by_status(&NodeStatus::Active).await.unwrap();
        let inactive_nodes = service.list_nodes_by_status(&NodeStatus::Inactive).await.unwrap();
        
        assert_eq!(active_nodes.len(), 1);
        assert_eq!(inactive_nodes.len(), 1);
        assert_eq!(active_nodes[0].status, NodeStatus::Active);
        assert_eq!(inactive_nodes[0].status, NodeStatus::Inactive);
    }

    #[tokio::test]
    async fn test_node_service_update_heartbeat() {
        let mut service = NodeService::new();
        let node = NodeInfo::new("192.168.1.1".to_string(), 8080);
        let uuid = node.uuid;
        let original_heartbeat = node.last_heartbeat;
        
        service.register_node(node).await.unwrap();
        
        // Wait a bit and update heartbeat / 等待一段时间并更新心跳
        tokio::time::sleep(tokio::time::Duration::from_millis(10)).await;
        
        let mut health_info = HashMap::new();
        health_info.insert("status".to_string(), "healthy".to_string());
        
        service.update_heartbeat(&uuid, Some(health_info.clone())).await.unwrap();
        
        let updated_node = service.get_node(&uuid).await.unwrap().unwrap();
        assert!(updated_node.last_heartbeat > original_heartbeat);
        assert_eq!(updated_node.health_info, health_info);
    }

    #[tokio::test]
    async fn test_node_service_cleanup_unhealthy_nodes() {
        let mut service = NodeService::new();
        
        let node1 = NodeInfo::new("192.168.1.1".to_string(), 8080);
        let node2 = NodeInfo::new("192.168.1.2".to_string(), 8081);
        
        let uuid1 = node1.uuid;
        let uuid2 = node2.uuid;
        
        // Register both nodes first / 先注册两个节点
        service.register_node(node1).await.unwrap();
        service.register_node(node2).await.unwrap();
        
        // Now manually update node2 to have an old heartbeat / 现在手动更新node2使其有过期的心跳
        let mut updated_node2 = service.get_node(&uuid2).await.unwrap().unwrap();
        updated_node2.last_heartbeat = Utc::now() - chrono::Duration::seconds(120);
        service.update_node(uuid2, updated_node2).await.unwrap();
        
        let removed = service.cleanup_unhealthy_nodes(60).await.unwrap();
        
        assert_eq!(removed.len(), 1);
        assert_eq!(removed[0], uuid2);
        assert!(service.get_node(&uuid1).await.unwrap().is_some());
        assert!(service.get_node(&uuid2).await.unwrap().is_none());
    }

    #[tokio::test]
    async fn test_node_service_resource_management() {
        let mut service = NodeService::new();
        let node = NodeInfo::new("192.168.1.1".to_string(), 8080);
        let uuid = node.uuid;
        
        service.register_node(node).await.unwrap();
        
        // Create resource info / 创建资源信息
        let resource = NodeResourceInfo::new(uuid);
        
        // Update resource / 更新资源
        service.update_node_resource(resource.clone()).await.unwrap();
        
        // Get resource / 获取资源
        let retrieved = service.get_node_resource(&uuid).await.unwrap();
        assert!(retrieved.is_some());
        assert_eq!(retrieved.unwrap().node_uuid, uuid);
        
        // Get node with resource / 获取节点及其资源
        let (node_info, resource_info) = service.get_node_with_resource(&uuid).await.unwrap().unwrap();
        assert_eq!(node_info.uuid, uuid);
        assert!(resource_info.is_some());
        assert_eq!(resource_info.unwrap().node_uuid, uuid);
        
        // Remove resource / 移除资源
        let removed = service.remove_node_resource(&uuid).await.unwrap();
        assert!(removed.is_some());
        assert_eq!(removed.unwrap().node_uuid, uuid);
        
        // Verify removal / 验证移除
        assert!(service.get_node_resource(&uuid).await.unwrap().is_none());
    }

    #[tokio::test]
    async fn test_node_service_get_node_with_resource() {
        let mut service = NodeService::new();
        let node = NodeInfo::new("192.168.1.1".to_string(), 8080);
        let uuid = node.uuid;
        
        service.register_node(node).await.unwrap();
        
        // Without resource / 没有资源
        let result = service.get_node_with_resource(&uuid).await.unwrap();
        assert!(result.is_some());
        let (node_info, resource_info) = result.unwrap();
        assert_eq!(node_info.uuid, uuid);
        assert!(resource_info.is_none());
        
        // With resource / 有资源
        let resource = NodeResourceInfo::new(uuid);
        service.update_node_resource(resource).await.unwrap();
        
        let result = service.get_node_with_resource(&uuid).await.unwrap();
        assert!(result.is_some());
        let (node_info, resource_info) = result.unwrap();
        assert_eq!(node_info.uuid, uuid);
        assert!(resource_info.is_some());
        assert_eq!(resource_info.unwrap().node_uuid, uuid);
    }

    #[tokio::test]
    async fn test_node_service_cluster_stats() {
        let mut service = NodeService::new();
        
        let mut node1 = NodeInfo::new("192.168.1.1".to_string(), 8080);
        let mut node2 = NodeInfo::new("192.168.1.2".to_string(), 8081);
        let mut node3 = NodeInfo::new("192.168.1.3".to_string(), 8082);
        
        node1.status = NodeStatus::Active;
        node2.status = NodeStatus::Inactive;
        node3.status = NodeStatus::Unhealthy;
        
        let uuid1 = node1.uuid;
        let uuid2 = node2.uuid;
        
        service.register_node(node1).await.unwrap();
        service.register_node(node2).await.unwrap();
        service.register_node(node3).await.unwrap();
        
        // Add some resource info / 添加一些资源信息
        let mut resource1 = NodeResourceInfo::new(uuid1);
        resource1.cpu_usage_percent = 50.0;
        resource1.memory_usage_percent = 60.0;
        
        let mut resource2 = NodeResourceInfo::new(uuid2);
        resource2.cpu_usage_percent = 30.0;
        resource2.memory_usage_percent = 40.0;
        
        service.update_node_resource(resource1).await.unwrap();
        service.update_node_resource(resource2).await.unwrap();
        
        let stats = service.get_cluster_stats().await.unwrap();
        
        assert_eq!(stats.total_nodes, 3);
        assert_eq!(stats.active_nodes, 1);
        assert_eq!(stats.inactive_nodes, 1);
        assert_eq!(stats.unhealthy_nodes, 1);
        assert_eq!(stats.nodes_with_resources, 2);
        assert_eq!(stats.average_cpu_usage, 40.0); // (50 + 30) / 2
        assert_eq!(stats.average_memory_usage, 50.0); // (60 + 40) / 2
    }

    #[tokio::test]
    async fn test_node_service_remove_node_with_resources() {
        let mut service = NodeService::new();
        let node = NodeInfo::new("192.168.1.1".to_string(), 8080);
        let uuid = node.uuid;
        
        service.register_node(node).await.unwrap();
        
        // Add resource / 添加资源
        let resource = NodeResourceInfo::new(uuid);
        service.update_node_resource(resource).await.unwrap();
        
        // Verify resource exists / 验证资源存在
        assert!(service.get_node_resource(&uuid).await.unwrap().is_some());
        
        // Remove node (should also remove resource) / 移除节点（也应该移除资源）
        service.remove_node(&uuid).await.unwrap();
        
        // Verify both node and resource are removed / 验证节点和资源都被移除
        assert!(service.get_node(&uuid).await.unwrap().is_none());
        assert!(service.get_node_resource(&uuid).await.unwrap().is_none());
    }

    #[tokio::test]
    async fn test_shared_kv_store_between_node_and_resource() {
        let mut service = NodeService::new();
        let node = NodeInfo::new("192.168.1.1".to_string(), 8080);
        let uuid = node.uuid;
        
        service.register_node(node).await.unwrap();
        
        // Add resource through service / 通过服务添加资源
        let resource = NodeResourceInfo::new(uuid);
        service.update_node_resource(resource).await.unwrap();
        
        // Verify resource can be retrieved / 验证可以检索资源
        let retrieved_resource = service.get_node_resource(&uuid).await.unwrap();
        assert!(retrieved_resource.is_some());
        assert_eq!(retrieved_resource.unwrap().node_uuid, uuid);
        
        // Verify resource count / 验证资源数量
        assert_eq!(service.resource_count().await.unwrap(), 1);
    }
}