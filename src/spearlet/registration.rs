//! Spearlet registration service / Spearlet注册服务
//!
//! This module handles Spearlet node registration with the SMS (SPEAR Metadata Server).
//! 此模块处理Spearlet节点向SMS（SPEAR元数据服务器）的注册。

use std::sync::Arc;
use std::time::Duration;
use tokio::sync::RwLock;
use tokio::time::{interval, Instant};
use tokio_util::sync::CancellationToken;
use tonic::transport::Channel;
use tracing::{debug, error, info, warn};

use crate::proto::sms::{
    node_service_client::NodeServiceClient, HeartbeatRequest, Node, NodeResource,
    RegisterNodeRequest, UpdateNodeResourceRequest,
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
    /// Disconnection start time / 断线开始时间
    disconnect_since: Arc<RwLock<Option<Instant>>>,
    cancel_token: CancellationToken,
}

impl RegistrationService {
    fn compute_node_uuid(config: &SpearletConfig) -> String {
        if let Ok(u) = uuid::Uuid::parse_str(&config.node_name) {
            return u.to_string();
        }
        let base = format!(
            "{}:{}:{}",
            config.grpc.addr.ip(),
            config.grpc.addr.port(),
            config.node_name
        );
        uuid::Uuid::new_v5(&uuid::Uuid::NAMESPACE_OID, base.as_bytes()).to_string()
    }
    /// Create new registration service / 创建新的注册服务
    pub fn new(config: Arc<SpearletConfig>) -> Self {
        Self {
            config,
            node_client: Arc::new(RwLock::new(None)),
            state: Arc::new(RwLock::new(RegistrationState::NotRegistered)),
            disconnect_since: Arc::new(RwLock::new(None)),
            cancel_token: CancellationToken::new(),
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
    pub async fn connect_to_sms(&self) -> Result<(), Box<dyn std::error::Error + Send + Sync>> {
        let sms_url = format!("http://{}", self.config.sms_grpc_addr);
        debug!("Connecting to SMS at: {}", sms_url);

        let mut last_err: Option<Box<dyn std::error::Error + Send + Sync>> = None;
        let deadline = Instant::now() + Duration::from_millis(self.config.sms_connect_timeout_ms);
        while Instant::now() < deadline {
            match Channel::from_shared(sms_url.clone())?.connect().await {
                Ok(channel) => {
                    let client = NodeServiceClient::new(channel);
                    *self.node_client.write().await = Some(client);
                    info!("Connected to SMS successfully");
                    return Ok(());
                }
                Err(e) => {
                    last_err = Some(Box::new(e));
                    warn!("Retrying SMS connection...");
                    tokio::time::sleep(Duration::from_millis(self.config.sms_connect_retry_ms))
                        .await;
                }
            }
        }
        Err(last_err.unwrap_or_else(|| Box::new(std::io::Error::other("unknown error"))))
    }

    // kept for clarity: start() already calls connect_to_sms() / start()已调用connect_to_sms()

    /// Start registration task / 启动注册任务
    async fn start_registration_task(&self) {
        let config = self.config.clone();
        let node_client = self.node_client.clone();
        let state = self.state.clone();
        let disconnect_since = self.disconnect_since.clone();
        let cancel_token = self.cancel_token.clone();

        tokio::spawn(async move {
            let mut heartbeat_interval = interval(Duration::from_secs(config.heartbeat_interval));

            loop {
                tokio::select! {
                    _ = cancel_token.cancelled() => {
                        break;
                    }
                    _ = heartbeat_interval.tick() => {}
                }

                // Exit if reconnection exceeded total timeout / 若重连超过总超时则退出
                if let Some(start) = *disconnect_since.read().await {
                    let elapsed = Instant::now().duration_since(start).as_millis() as u64;
                    if elapsed >= config.reconnect_total_timeout_ms {
                        error!("Reconnect timed out after {} ms; exiting", elapsed);
                        std::process::exit(1);
                    }
                }

                let current_state = state.read().await.clone();
                match current_state {
                    RegistrationState::NotRegistered | RegistrationState::Failed { .. } => {
                        // Attempt registration / 尝试注册
                        // Ensure client is connected / 确保客户端已连接
                        if node_client.read().await.is_none() {
                            if let Err(e) = Self::attempt_reconnect(&config, &node_client).await {
                                warn!("Reconnect to SMS failed: {}", e);
                                *state.write().await = RegistrationState::Failed {
                                    error: e.to_string(),
                                    last_attempt: Instant::now(),
                                };
                                // mark disconnect start if not set / 若未设置则标记断线开始
                                if disconnect_since.read().await.is_none() {
                                    *disconnect_since.write().await = Some(Instant::now());
                                }
                                continue;
                            }
                            // reconnected, clear disconnect_since / 重连成功，清除断线标记
                            *disconnect_since.write().await = None;
                        }
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
                        debug!(
                            "Heartbeat tick: interval={}s, node_name={}, sms_grpc_addr={}",
                            config.heartbeat_interval, config.node_name, config.sms_grpc_addr
                        );
                        if let Err(e) = Self::send_heartbeat(&config, &node_client, &state).await {
                            warn!("Heartbeat failed: {}", e);
                            // Try reconnect immediately / 立即尝试重连
                            if let Err(re) = Self::attempt_reconnect(&config, &node_client).await {
                                warn!("Reconnect to SMS failed after heartbeat error: {}", re);
                                *state.write().await = RegistrationState::Failed {
                                    error: re.to_string(),
                                    last_attempt: Instant::now(),
                                };
                                if disconnect_since.read().await.is_none() {
                                    *disconnect_since.write().await = Some(Instant::now());
                                }
                            } else {
                                // Re-register immediately after reconnect / 重连成功后立即重新注册
                                *disconnect_since.write().await = None;
                                if let Err(e) =
                                    Self::register_node(&config, &node_client, &state).await
                                {
                                    error!("Re-registration failed after reconnect: {}", e);
                                    *state.write().await = RegistrationState::Failed {
                                        error: e.to_string(),
                                        last_attempt: Instant::now(),
                                    };
                                } else {
                                    info!("Re-registered successfully after reconnect");
                                }
                            }
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
        let client = client_guard.as_mut().ok_or("Node client not connected")?;

        let node_addr = config.grpc.addr;
        let node_uuid = Self::compute_node_uuid(config);
        let node = Node {
            uuid: node_uuid,
            ip_address: node_addr.ip().to_string(),
            port: node_addr.port() as i32,
            status: "online".to_string(),
            last_heartbeat: chrono::Utc::now().timestamp(),
            registered_at: chrono::Utc::now().timestamp(),
            metadata: {
                let mut m = std::collections::HashMap::new();
                m.insert("name".to_string(), config.node_name.clone());
                m
            },
        };

        let request = tonic::Request::new(RegisterNodeRequest { node: Some(node) });

        client.register_node(request).await?;

        if let Err(e) = Self::send_resource_report(client, config).await {
            warn!("Resource report failed: {}", e);
        }

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
        let client = client_guard.as_mut().ok_or("Node client not connected")?;

        let node_uuid = Self::compute_node_uuid(config);
        let ts = chrono::Utc::now().timestamp();
        debug!(
            "Sending heartbeat: uuid={}, node_name={}, ts={}, sms_grpc_addr={}",
            node_uuid, config.node_name, ts, config.sms_grpc_addr
        );
        let request = tonic::Request::new(HeartbeatRequest {
            uuid: node_uuid.clone(),
            timestamp: ts,
            health_info: std::collections::HashMap::new(),
        });

        let resp = client.heartbeat(request).await?;
        let server_ts = resp.get_ref().server_timestamp;
        debug!("Heartbeat ACK: uuid={}, server_ts={}", node_uuid, server_ts);

        if let Err(e) = Self::send_resource_report(client, config).await {
            warn!("Resource report failed: {}", e);
        }

        // Update last heartbeat time / 更新最后心跳时间
        let mut st = state.write().await;
        if let RegistrationState::Registered { registered_at, .. } = &*st {
            let registered_at = *registered_at;
            *st = RegistrationState::Registered {
                registered_at,
                last_heartbeat: Instant::now(),
            };
        }

        debug!("Heartbeat sent successfully");
        Ok(())
    }

    async fn send_resource_report(
        client: &mut NodeServiceClient<Channel>,
        config: &SpearletConfig,
    ) -> Result<(), Box<dyn std::error::Error + Send + Sync>> {
        let node_uuid = Self::compute_node_uuid(config);
        let resource = Self::collect_node_resource(&node_uuid);
        let req = tonic::Request::new(UpdateNodeResourceRequest {
            resource: Some(resource),
        });
        let _ = client.update_node_resource(req).await?;
        Ok(())
    }

    fn collect_node_resource(node_uuid: &str) -> NodeResource {
        let ts = chrono::Utc::now().timestamp();
        let (load1, load5, load15) = Self::get_load_averages();
        let (total_mem, avail_mem, used_mem, mem_percent) = Self::get_memory_snapshot();
        let (total_disk, used_disk, disk_percent) = Self::get_disk_snapshot();
        let cpu_percent = Self::estimate_cpu_percent(load1);

        NodeResource {
            node_uuid: node_uuid.to_string(),
            cpu_usage_percent: cpu_percent,
            memory_usage_percent: mem_percent,
            total_memory_bytes: total_mem,
            used_memory_bytes: used_mem,
            available_memory_bytes: avail_mem,
            disk_usage_percent: disk_percent,
            total_disk_bytes: total_disk,
            used_disk_bytes: used_disk,
            network_rx_bytes_per_sec: 0,
            network_tx_bytes_per_sec: 0,
            load_average_1m: load1,
            load_average_5m: load5,
            load_average_15m: load15,
            updated_at: ts,
            resource_metadata: std::collections::HashMap::new(),
        }
    }

    fn get_load_averages() -> (f64, f64, f64) {
        let mut loads = [0f64; 3];
        let n = unsafe { libc::getloadavg(loads.as_mut_ptr(), 3) };
        if n <= 0 {
            return (0.0, 0.0, 0.0);
        }
        (
            loads.get(0).copied().unwrap_or(0.0),
            loads.get(1).copied().unwrap_or(0.0),
            loads.get(2).copied().unwrap_or(0.0),
        )
    }

    fn estimate_cpu_percent(load1: f64) -> f64 {
        let ncpu = unsafe { libc::sysconf(libc::_SC_NPROCESSORS_ONLN) };
        if ncpu <= 0 {
            return 0.0;
        }
        ((load1 / (ncpu as f64)) * 100.0).clamp(0.0, 100.0)
    }

    #[cfg(target_os = "linux")]
    fn get_memory_snapshot() -> (i64, i64, i64, f64) {
        let page_size = unsafe { libc::sysconf(libc::_SC_PAGESIZE) };
        let phys_pages = unsafe { libc::sysconf(libc::_SC_PHYS_PAGES) };
        let avail_pages = unsafe { libc::sysconf(libc::_SC_AVPHYS_PAGES) };
        if page_size <= 0 || phys_pages <= 0 || avail_pages < 0 {
            return (0, 0, 0, 0.0);
        }
        let total = (phys_pages as i128) * (page_size as i128);
        let avail = (avail_pages as i128) * (page_size as i128);
        let used = (total - avail).max(0);
        let percent = if total > 0 {
            ((used as f64) / (total as f64) * 100.0).clamp(0.0, 100.0)
        } else {
            0.0
        };
        (
            total.min(i64::MAX as i128) as i64,
            avail.min(i64::MAX as i128) as i64,
            used.min(i64::MAX as i128) as i64,
            percent,
        )
    }

    #[cfg(target_os = "macos")]
    fn get_memory_snapshot() -> (i64, i64, i64, f64) {
        fn sysctl_u64(name: &str) -> Option<u64> {
            let c = std::ffi::CString::new(name).ok()?;
            let mut v: u64 = 0;
            let mut len = std::mem::size_of::<u64>();
            let rc = unsafe {
                libc::sysctlbyname(
                    c.as_ptr(),
                    &mut v as *mut _ as *mut _,
                    &mut len,
                    std::ptr::null_mut(),
                    0,
                )
            };
            if rc == 0 {
                Some(v)
            } else {
                None
            }
        }

        fn sysctl_u32(name: &str) -> Option<u32> {
            let c = std::ffi::CString::new(name).ok()?;
            let mut v: u32 = 0;
            let mut len = std::mem::size_of::<u32>();
            let rc = unsafe {
                libc::sysctlbyname(
                    c.as_ptr(),
                    &mut v as *mut _ as *mut _,
                    &mut len,
                    std::ptr::null_mut(),
                    0,
                )
            };
            if rc == 0 {
                Some(v)
            } else {
                None
            }
        }

        let total = sysctl_u64("hw.memsize").unwrap_or(0) as i128;
        let page_size = sysctl_u64("hw.pagesize").unwrap_or(0) as i128;
        let free_pages = sysctl_u32("vm.page_free_count").unwrap_or(0) as i128;
        if total <= 0 || page_size <= 0 {
            return (0, 0, 0, 0.0);
        }
        let avail = (free_pages * page_size).max(0);
        let used = (total - avail).max(0);
        let percent = ((used as f64) / (total as f64) * 100.0).clamp(0.0, 100.0);
        (
            total.min(i64::MAX as i128) as i64,
            avail.min(i64::MAX as i128) as i64,
            used.min(i64::MAX as i128) as i64,
            percent,
        )
    }

    #[cfg(not(any(target_os = "linux", target_os = "macos")))]
    fn get_memory_snapshot() -> (i64, i64, i64, f64) {
        let page_size = unsafe { libc::sysconf(libc::_SC_PAGESIZE) };
        let phys_pages = unsafe { libc::sysconf(libc::_SC_PHYS_PAGES) };
        if page_size <= 0 || phys_pages <= 0 {
            return (0, 0, 0, 0.0);
        }
        let total = (phys_pages as i128) * (page_size as i128);
        (total.min(i64::MAX as i128) as i64, 0, 0, 0.0)
    }

    fn get_disk_snapshot() -> (i64, i64, f64) {
        let path = match std::ffi::CString::new("/") {
            Ok(p) => p,
            Err(_) => return (0, 0, 0.0),
        };
        let mut stat: libc::statvfs = unsafe { std::mem::zeroed() };
        let rc = unsafe { libc::statvfs(path.as_ptr(), &mut stat) };
        if rc != 0 {
            return (0, 0, 0.0);
        }
        let total = (stat.f_blocks as i128) * (stat.f_frsize as i128);
        let avail = (stat.f_bavail as i128) * (stat.f_frsize as i128);
        let used = (total - avail).max(0);
        let percent = if total > 0 {
            ((used as f64) / (total as f64) * 100.0).clamp(0.0, 100.0)
        } else {
            0.0
        };
        (
            total.min(i64::MAX as i128) as i64,
            used.min(i64::MAX as i128) as i64,
            percent,
        )
    }

    /// Attempt to reconnect to SMS / 尝试重新连接SMS
    async fn attempt_reconnect(
        config: &SpearletConfig,
        node_client: &Arc<RwLock<Option<NodeServiceClient<Channel>>>>,
    ) -> Result<(), Box<dyn std::error::Error + Send + Sync>> {
        let sms_url = format!("http://{}", config.sms_grpc_addr);
        let deadline = Instant::now() + Duration::from_millis(config.sms_connect_timeout_ms);
        let mut last_err: Option<Box<dyn std::error::Error + Send + Sync>> = None;
        while Instant::now() < deadline {
            match Channel::from_shared(sms_url.clone())?.connect().await {
                Ok(channel) => {
                    let client = NodeServiceClient::new(channel);
                    *node_client.write().await = Some(client);
                    info!("Reconnected to SMS successfully");
                    return Ok(());
                }
                Err(e) => {
                    last_err = Some(Box::new(e));
                    tokio::time::sleep(Duration::from_millis(config.sms_connect_retry_ms)).await;
                }
            }
        }
        Err(last_err.unwrap_or_else(|| Box::new(std::io::Error::other("reconnect failed"))))
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

    pub fn shutdown(&self) {
        self.cancel_token.cancel();
    }
}
