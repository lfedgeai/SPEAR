//! Spearlet registration service / Spearlet注册服务
//!
//! This module handles Spearlet node registration with the SMS (SPEAR Metadata Server).
//! 此模块处理Spearlet节点向SMS（SPEAR元数据服务器）的注册。

use std::sync::Arc;
use std::time::Duration;
use tokio::sync::RwLock;
use tokio::time::{interval, Instant};
use tonic::transport::Channel;
use tracing::{debug, error, info, warn};

use crate::proto::sms::{
    node_service_client::NodeServiceClient, HeartbeatRequest, Node,
    RegisterNodeRequest,
};
use crate::spearlet::config::SpearletConfig;

/// Registration state / 注册状态
#[derive(Debug, Clone)]
pub enum RegistrationState {
    /// Not registered / 未注册
    NotRegistered,
    /// Registration in progress / 注册进行中
    Registering,
    /// Successfully registered / 注册成功
    Registered {
        /// Registration timestamp / 注册时间戳
        registered_at: Instant,
        /// Last heartbeat timestamp / 最后心跳时间戳
        last_heartbeat: Instant,
    },
    /// Registration failed / 注册失败
    Failed {
        /// Error message / 错误信息
        error: String,
        /// Last attempt timestamp / 最后尝试时间戳
        last_attempt: Instant,
    },
}

impl RegistrationState {
    /// Check if currently registered / 检查是否已注册
    pub fn is_registered(&self) -> bool {
        matches!(self, RegistrationState::Registered { .. })
    }

    /// Check if registration failed / 检查注册是否失败
    pub fn is_failed(&self) -> bool {
        matches!(self, RegistrationState::Failed { .. })
    }

    /// Get status description / 获取状态描述
    pub fn status_description(&self) -> &'static str {
        match self {
            RegistrationState::NotRegistered => "Not registered",
            RegistrationState::Registering => "Registering",
            RegistrationState::Registered { .. } => "Registered",
            RegistrationState::Failed { .. } => "Failed",
        }
    }
}

/// Registration service for managing Spearlet node registration / 管理Spearlet节点注册的注册服务
pub struct RegistrationService {
    /// Configuration / 配置
    config: Arc<SpearletConfig>,
    /// Node service client / 节点服务客户端
    node_client: Arc<RwLock<Option<NodeServiceClient<Channel>>>>,
    /// Current registration state / 当前注册状态
    state: Arc<RwLock<RegistrationState>>,
}

impl RegistrationService {
    /// Create new registration service / 创建新的注册服务
    pub fn new(config: Arc<SpearletConfig>) -> Self {
        Self {
            config,
            node_client: Arc::new(RwLock::new(None)),
            state: Arc::new(RwLock::new(RegistrationState::NotRegistered)),
        }
    }

    /// Start registration service / 启动注册服务
    pub async fn start(&self) -> Result<(), Box<dyn std::error::Error + Send + Sync>> {
        info!("Starting Spearlet registration service");

        // Connect to SMS / 连接到SMS
        self.connect_to_sms().await?;

        // Start registration and heartbeat tasks / 启动注册和心跳任务
        if self.config.auto_register {
            self.start_registration_task().await;
        }

        Ok(())
    }

    /// Connect to SMS service / 连接到SMS服务
    async fn connect_to_sms(&self) -> Result<(), Box<dyn std::error::Error + Send + Sync>> {
        let sms_url = format!("http://{}", self.config.sms_addr);
        debug!("Connecting to SMS at: {}", sms_url);

        let channel = Channel::from_shared(sms_url)?
            .connect()
            .await?;

        let client = NodeServiceClient::new(channel);
        *self.node_client.write().await = Some(client);

        info!("Connected to SMS successfully");
        Ok(())
    }

    /// Start registration task / 启动注册任务
    async fn start_registration_task(&self) {
        let config = self.config.clone();
        let node_client = self.node_client.clone();
        let state = self.state.clone();

        tokio::spawn(async move {
            let mut heartbeat_interval = interval(Duration::from_secs(config.heartbeat_interval));

            loop {
                heartbeat_interval.tick().await;

                let current_state = state.read().await.clone();
                match current_state {
                    RegistrationState::NotRegistered | RegistrationState::Failed { .. } => {
                        // Attempt registration / 尝试注册
                        if let Err(e) = Self::register_node(&config, &node_client, &state).await {
                            error!("Registration failed: {}", e);
                            *state.write().await = RegistrationState::Failed {
                                error: e.to_string(),
                                last_attempt: Instant::now(),
                            };
                        }
                    }
                    RegistrationState::Registered { .. } => {
                        // Send heartbeat / 发送心跳
                        if let Err(e) = Self::send_heartbeat(&config, &node_client, &state).await {
                            warn!("Heartbeat failed: {}", e);
                            // Don't change state immediately, allow a few failures
                            // 不立即改变状态，允许几次失败
                        }
                    }
                    RegistrationState::Registering => {
                        // Wait for registration to complete / 等待注册完成
                        debug!("Registration in progress, waiting...");
                    }
                }
            }
        });
    }

    /// Register node with SMS / 向SMS注册节点
    async fn register_node(
        config: &SpearletConfig,
        node_client: &Arc<RwLock<Option<NodeServiceClient<Channel>>>>,
        state: &Arc<RwLock<RegistrationState>>,
    ) -> Result<(), Box<dyn std::error::Error + Send + Sync>> {
        *state.write().await = RegistrationState::Registering;

        let mut client_guard = node_client.write().await;
        let client = client_guard
            .as_mut()
            .ok_or("Node client not connected")?;

        let node_addr = config.grpc.addr;
        let node = Node {
            uuid: config.node_id.clone(),
            ip_address: node_addr.ip().to_string(),
            port: node_addr.port() as i32,
            status: "active".to_string(),
            last_heartbeat: chrono::Utc::now().timestamp(),
            registered_at: chrono::Utc::now().timestamp(),
            metadata: std::collections::HashMap::new(),
        };

        let request = tonic::Request::new(RegisterNodeRequest { node: Some(node) });

        client.register_node(request).await?;

        let now = Instant::now();
        *state.write().await = RegistrationState::Registered {
            registered_at: now,
            last_heartbeat: now,
        };

        info!("Successfully registered with SMS");
        Ok(())
    }

    /// Send heartbeat to SMS / 向SMS发送心跳
    async fn send_heartbeat(
        config: &SpearletConfig,
        node_client: &Arc<RwLock<Option<NodeServiceClient<Channel>>>>,
        state: &Arc<RwLock<RegistrationState>>,
    ) -> Result<(), Box<dyn std::error::Error + Send + Sync>> {
        let mut client_guard = node_client.write().await;
        let client = client_guard
            .as_mut()
            .ok_or("Node client not connected")?;

        let request = tonic::Request::new(HeartbeatRequest {
            uuid: config.node_id.clone(),
            timestamp: chrono::Utc::now().timestamp(),
            health_info: std::collections::HashMap::new(),
        });

        client.heartbeat(request).await?;

        // Update last heartbeat time / 更新最后心跳时间
        if let RegistrationState::Registered { registered_at, .. } = &*state.read().await {
            let registered_at = *registered_at;
            *state.write().await = RegistrationState::Registered {
                registered_at,
                last_heartbeat: Instant::now(),
            };
        }

        debug!("Heartbeat sent successfully");
        Ok(())
    }

    /// Get current registration state / 获取当前注册状态
    pub async fn get_state(&self) -> RegistrationState {
        self.state.read().await.clone()
    }

    /// Force registration / 强制注册
    pub async fn force_register(&self) -> Result<(), Box<dyn std::error::Error + Send + Sync>> {
        Self::register_node(&self.config, &self.node_client, &self.state).await
    }

    /// Disconnect from SMS / 断开与SMS的连接
    pub async fn disconnect(&self) {
        *self.node_client.write().await = None;
        *self.state.write().await = RegistrationState::NotRegistered;
        info!("Disconnected from SMS");
    }
}
