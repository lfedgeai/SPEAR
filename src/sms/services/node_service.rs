//! Node service implementation / 节点服务实现

use std::collections::HashMap;
use std::sync::Arc;
use tokio::sync::RwLock;

use crate::proto::sms::Node;
use crate::sms::error::{SmsError, SmsResult};

#[derive(Debug, Clone)]
pub struct NodeOfflineMark {
    pub uuid: String,
    pub last_heartbeat: i64,
    pub age_seconds: i64,
    pub previous_status: String,
    pub node: Node,
}

/// Node service for managing cluster nodes / 管理集群节点的服务
#[derive(Debug, Clone)]
pub struct NodeService {
    /// In-memory storage for nodes / 节点的内存存储
    nodes: Arc<RwLock<HashMap<String, Node>>>,
}

impl NodeService {
    /// Create a new node service / 创建新的节点服务
    pub fn new() -> Self {
        Self {
            nodes: Arc::new(RwLock::new(HashMap::new())),
        }
    }

    /// Register a node / 注册节点
    pub async fn register_node(&mut self, node: Node) -> SmsResult<()> {
        let mut nodes = self.nodes.write().await;
        nodes.insert(node.uuid.clone(), node);
        Ok(())
    }

    /// Get a node by UUID / 根据UUID获取节点
    pub async fn get_node(&self, uuid: &str) -> SmsResult<Option<Node>> {
        let nodes = self.nodes.read().await;
        Ok(nodes.get(uuid).cloned())
    }

    /// List all nodes / 列出所有节点
    pub async fn list_nodes(&self) -> SmsResult<Vec<Node>> {
        let nodes = self.nodes.read().await;
        Ok(nodes.values().cloned().collect())
    }

    /// Update node heartbeat / 更新节点心跳
    pub async fn update_heartbeat(
        &mut self,
        uuid: &str,
        timestamp: i64,
    ) -> SmsResult<Option<Node>> {
        let mut nodes = self.nodes.write().await;
        if let Some(node) = nodes.get_mut(uuid) {
            node.last_heartbeat = timestamp;
            if node.status.to_ascii_lowercase() != "online" {
                node.status = "online".to_string();
                return Ok(Some(node.clone()));
            }
            Ok(None)
        } else {
            Err(SmsError::NotFound(format!(
                "Node with UUID {} not found",
                uuid
            )))
        }
    }

    pub async fn mark_unhealthy_nodes_offline(
        &mut self,
        timeout_seconds: u64,
    ) -> SmsResult<Vec<NodeOfflineMark>> {
        let mut nodes = self.nodes.write().await;
        let current_time = chrono::Utc::now().timestamp();
        let timeout_threshold = current_time - timeout_seconds as i64;

        let mut updated = Vec::new();
        for (uuid, node) in nodes.iter_mut() {
            if node.last_heartbeat < timeout_threshold {
                let previous_status = node.status.clone();
                if previous_status.to_ascii_lowercase() != "offline" {
                    node.status = "offline".to_string();
                }
                updated.push(NodeOfflineMark {
                    uuid: uuid.clone(),
                    last_heartbeat: node.last_heartbeat,
                    age_seconds: current_time - node.last_heartbeat,
                    previous_status,
                    node: node.clone(),
                });
            }
        }

        Ok(updated)
    }

    /// Update an existing node / 更新现有节点
    pub async fn update_node(&mut self, node: Node) -> SmsResult<()> {
        let mut nodes = self.nodes.write().await;
        if nodes.contains_key(&node.uuid) {
            nodes.insert(node.uuid.clone(), node);
            Ok(())
        } else {
            Err(SmsError::NotFound(format!(
                "Node with UUID {} not found",
                node.uuid
            )))
        }
    }

    /// Remove a node / 移除节点
    pub async fn remove_node(&mut self, uuid: &str) -> SmsResult<()> {
        let mut nodes = self.nodes.write().await;
        if nodes.remove(uuid).is_some() {
            Ok(())
        } else {
            Err(SmsError::NotFound(format!(
                "Node with UUID {} not found",
                uuid
            )))
        }
    }

    /// Get node count / 获取节点数量
    pub async fn node_count(&self) -> SmsResult<usize> {
        let nodes = self.nodes.read().await;
        Ok(nodes.len())
    }

    /// Cleanup unhealthy nodes / 清理不健康的节点
    pub async fn cleanup_unhealthy_nodes(
        &mut self,
        timeout_seconds: u64,
    ) -> SmsResult<Vec<String>> {
        let mut nodes = self.nodes.write().await;
        let current_time = chrono::Utc::now().timestamp();
        let timeout_threshold = current_time - timeout_seconds as i64;

        let mut removed_nodes = Vec::new();
        nodes.retain(|uuid, node| {
            if node.last_heartbeat < timeout_threshold {
                removed_nodes.push(uuid.clone());
                false
            } else {
                true
            }
        });

        Ok(removed_nodes)
    }
}

impl Default for NodeService {
    fn default() -> Self {
        Self::new()
    }
}

/// Node information for registration / 节点注册信息
#[derive(Debug, Clone, serde::Serialize, serde::Deserialize)]
pub struct NodeInfo {
    pub uuid: String,
    pub name: String,
    pub address: String,
    pub port: u16,
    pub capabilities: Vec<String>,
}

impl NodeInfo {
    /// Get the full address (address:port) / 获取完整地址（地址:端口）
    pub fn address(&self) -> String {
        format!("{}:{}", self.address, self.port)
    }
}

/// Node status enumeration / 节点状态枚举
#[derive(Debug, Clone, PartialEq)]
pub enum NodeStatus {
    Online,
    Offline,
    Maintenance,
}
