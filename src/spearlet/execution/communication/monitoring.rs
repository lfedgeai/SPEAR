//! Monitoring and diagnostics module for runtime communication
//! 运行时通信的监控和诊断模块

use crate::spearlet::execution::communication::protocol::ConnectionStatus;
use crate::spearlet::execution::communication::{ConnectionEvent, MessageType, SpearMessage};
use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::sync::Arc;
use std::time::{Duration, SystemTime};
use tokio::sync::{mpsc, RwLock};

/// Monitoring configuration / 监控配置
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct MonitoringConfig {
    /// Enable monitoring / 启用监控
    pub enabled: bool,
    /// Metrics collection interval / 指标收集间隔
    pub collection_interval_secs: u64,
    /// Maximum metrics history size / 最大指标历史大小
    pub max_history_size: usize,
    /// Enable detailed message tracking / 启用详细消息跟踪
    pub enable_message_tracking: bool,
    /// Enable connection tracking / 启用连接跟踪
    pub enable_connection_tracking: bool,
    /// Enable performance profiling / 启用性能分析
    pub enable_performance_profiling: bool,
}

impl Default for MonitoringConfig {
    fn default() -> Self {
        Self {
            enabled: true,
            collection_interval_secs: 30,
            max_history_size: 1000,
            enable_message_tracking: true,
            enable_connection_tracking: true,
            enable_performance_profiling: false,
        }
    }
}

/// Connection metrics / 连接指标
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ConnectionMetrics {
    /// Connection ID / 连接ID
    pub connection_id: String,
    /// Connection state / 连接状态
    pub state: ConnectionStatus,
    /// Connection start time / 连接开始时间
    pub connected_at: SystemTime,
    /// Last activity time / 最后活动时间
    pub last_activity: SystemTime,
    /// Total messages sent / 发送消息总数
    pub messages_sent: u64,
    /// Total messages received / 接收消息总数
    pub messages_received: u64,
    /// Total bytes sent / 发送字节总数
    pub bytes_sent: u64,
    /// Total bytes received / 接收字节总数
    pub bytes_received: u64,
    /// Connection errors / 连接错误数
    pub error_count: u64,
    /// Average response time (ms) / 平均响应时间（毫秒）
    pub avg_response_time_ms: f64,
}

/// Message metrics / 消息指标
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct MessageMetrics {
    /// Message type / 消息类型
    pub message_type: MessageType,
    /// Total count / 总数
    pub total_count: u64,
    /// Success count / 成功数
    pub success_count: u64,
    /// Error count / 错误数
    pub error_count: u64,
    /// Average processing time (ms) / 平均处理时间（毫秒）
    pub avg_processing_time_ms: f64,
    /// Maximum processing time (ms) / 最大处理时间（毫秒）
    pub max_processing_time_ms: f64,
    /// Minimum processing time (ms) / 最小处理时间（毫秒）
    pub min_processing_time_ms: f64,
}

/// System metrics / 系统指标
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SystemMetrics {
    /// Timestamp / 时间戳
    pub timestamp: SystemTime,
    /// Active connections / 活跃连接数
    pub active_connections: usize,
    /// Total connections / 总连接数
    pub total_connections: u64,
    /// Messages per second / 每秒消息数
    pub messages_per_second: f64,
    /// Bytes per second / 每秒字节数
    pub bytes_per_second: f64,
    /// Memory usage (bytes) / 内存使用量（字节）
    pub memory_usage_bytes: u64,
    /// CPU usage percentage / CPU使用率
    pub cpu_usage_percent: f64,
    /// Error rate / 错误率
    pub error_rate: f64,
}

/// Performance event / 性能事件
#[derive(Debug, Clone)]
pub struct PerformanceEvent {
    /// Event ID / 事件ID
    pub event_id: String,
    /// Event type / 事件类型
    pub event_type: String,
    /// Start time / 开始时间
    pub start_time: SystemTime,
    /// Duration / 持续时间
    pub duration: Option<Duration>,
    /// Metadata / 元数据
    pub metadata: HashMap<String, String>,
}

/// Monitoring event / 监控事件
#[derive(Debug, Clone)]
pub enum MonitoringEvent {
    /// Connection event / 连接事件
    Connection {
        connection_id: String,
        event: ConnectionEvent,
        timestamp: SystemTime,
    },
    /// Message event / 消息事件
    Message {
        connection_id: String,
        message_type: MessageType,
        direction: MessageDirection,
        size_bytes: usize,
        processing_time_ms: Option<f64>,
        timestamp: SystemTime,
    },
    /// Error event / 错误事件
    Error {
        connection_id: Option<String>,
        error_type: String,
        error_message: String,
        timestamp: SystemTime,
    },
    /// Performance event / 性能事件
    Performance {
        event_type: String,
        duration_ms: f64,
        metadata: HashMap<String, String>,
        timestamp: SystemTime,
    },
}

/// Message direction / 消息方向
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum MessageDirection {
    /// Incoming / 传入
    Incoming,
    /// Outgoing / 传出
    Outgoing,
}

/// Monitoring service / 监控服务
pub struct MonitoringService {
    /// Configuration / 配置
    config: MonitoringConfig,
    /// Connection metrics / 连接指标
    connection_metrics: Arc<RwLock<HashMap<String, ConnectionMetrics>>>,
    /// Message metrics / 消息指标
    message_metrics: Arc<RwLock<HashMap<MessageType, MessageMetrics>>>,
    /// System metrics history / 系统指标历史
    system_metrics_history: Arc<RwLock<Vec<SystemMetrics>>>,
    /// Event sender / 事件发送器
    event_sender: mpsc::UnboundedSender<MonitoringEvent>,
    /// Event receiver / 事件接收器
    event_receiver: Arc<RwLock<Option<mpsc::UnboundedReceiver<MonitoringEvent>>>>,
    /// Performance events / 性能事件
    performance_events: Arc<RwLock<HashMap<String, PerformanceEvent>>>,
}

impl MonitoringService {
    /// Create new monitoring service / 创建新的监控服务
    pub fn new(config: MonitoringConfig) -> Self {
        let (event_sender, event_receiver) = mpsc::unbounded_channel();

        Self {
            config,
            connection_metrics: Arc::new(RwLock::new(HashMap::new())),
            message_metrics: Arc::new(RwLock::new(HashMap::new())),
            system_metrics_history: Arc::new(RwLock::new(Vec::new())),
            event_sender,
            event_receiver: Arc::new(RwLock::new(Some(event_receiver))),
            performance_events: Arc::new(RwLock::new(HashMap::new())),
        }
    }

    /// Start monitoring service / 启动监控服务
    pub async fn start(&self) -> Result<(), Box<dyn std::error::Error + Send + Sync>> {
        if !self.config.enabled {
            return Ok(());
        }

        let mut receiver = self
            .event_receiver
            .write()
            .await
            .take()
            .ok_or("Monitoring service already started")?;

        let connection_metrics = Arc::clone(&self.connection_metrics);
        let message_metrics = Arc::clone(&self.message_metrics);
        let system_metrics_history = Arc::clone(&self.system_metrics_history);
        let performance_events = Arc::clone(&self.performance_events);
        let config = self.config.clone();

        // Start event processing task / 启动事件处理任务
        tokio::spawn(async move {
            while let Some(event) = receiver.recv().await {
                Self::process_event(
                    &event,
                    &connection_metrics,
                    &message_metrics,
                    &system_metrics_history,
                    &performance_events,
                    &config,
                )
                .await;
            }
        });

        // Start metrics collection task / 启动指标收集任务
        let system_metrics_history_clone = Arc::clone(&self.system_metrics_history);
        let connection_metrics_clone = Arc::clone(&self.connection_metrics);
        let collection_interval = Duration::from_secs(self.config.collection_interval_secs);
        let max_history_size = self.config.max_history_size;

        tokio::spawn(async move {
            let mut interval = tokio::time::interval(collection_interval);
            loop {
                interval.tick().await;
                Self::collect_system_metrics(
                    &system_metrics_history_clone,
                    &connection_metrics_clone,
                    max_history_size,
                )
                .await;
            }
        });

        Ok(())
    }

    /// Record connection event / 记录连接事件
    pub async fn record_connection_event(&self, connection_id: String, event: ConnectionEvent) {
        if !self.config.enabled || !self.config.enable_connection_tracking {
            return;
        }

        let monitoring_event = MonitoringEvent::Connection {
            connection_id,
            event,
            timestamp: SystemTime::now(),
        };

        let _ = self.event_sender.send(monitoring_event);
    }

    /// Record message event / 记录消息事件
    pub async fn record_message_event(
        &self,
        connection_id: String,
        message: &SpearMessage,
        direction: MessageDirection,
        processing_time_ms: Option<f64>,
    ) {
        if !self.config.enabled || !self.config.enable_message_tracking {
            return;
        }

        let size_bytes = serde_json::to_vec(message).unwrap_or_default().len();

        let monitoring_event = MonitoringEvent::Message {
            connection_id,
            message_type: message.message_type.clone(),
            direction,
            size_bytes,
            processing_time_ms,
            timestamp: SystemTime::now(),
        };

        let _ = self.event_sender.send(monitoring_event);
    }

    /// Record error event / 记录错误事件
    pub async fn record_error(
        &self,
        connection_id: Option<String>,
        error_type: String,
        error_message: String,
    ) {
        if !self.config.enabled {
            return;
        }

        let monitoring_event = MonitoringEvent::Error {
            connection_id,
            error_type,
            error_message,
            timestamp: SystemTime::now(),
        };

        let _ = self.event_sender.send(monitoring_event);
    }

    /// Start performance tracking / 开始性能跟踪
    pub async fn start_performance_tracking(
        &self,
        event_id: String,
        event_type: String,
        metadata: HashMap<String, String>,
    ) {
        if !self.config.enabled || !self.config.enable_performance_profiling {
            return;
        }

        let event = PerformanceEvent {
            event_id: event_id.clone(),
            event_type,
            start_time: SystemTime::now(),
            duration: None,
            metadata,
        };

        self.performance_events
            .write()
            .await
            .insert(event_id, event);
    }

    /// End performance tracking / 结束性能跟踪
    pub async fn end_performance_tracking(&self, event_id: &str) {
        if !self.config.enabled || !self.config.enable_performance_profiling {
            return;
        }

        let mut events = self.performance_events.write().await;
        if let Some(mut event) = events.remove(event_id) {
            let duration = SystemTime::now()
                .duration_since(event.start_time)
                .unwrap_or_default();
            event.duration = Some(duration);

            let monitoring_event = MonitoringEvent::Performance {
                event_type: event.event_type,
                duration_ms: duration.as_secs_f64() * 1000.0,
                metadata: event.metadata,
                timestamp: SystemTime::now(),
            };

            let _ = self.event_sender.send(monitoring_event);
        }
    }

    /// Get connection metrics / 获取连接指标
    pub async fn get_connection_metrics(&self) -> HashMap<String, ConnectionMetrics> {
        self.connection_metrics.read().await.clone()
    }

    /// Get message metrics / 获取消息指标
    pub async fn get_message_metrics(&self) -> HashMap<MessageType, MessageMetrics> {
        self.message_metrics.read().await.clone()
    }

    /// Get system metrics history / 获取系统指标历史
    pub async fn get_system_metrics_history(&self) -> Vec<SystemMetrics> {
        self.system_metrics_history.read().await.clone()
    }

    /// Get latest system metrics / 获取最新系统指标
    pub async fn get_latest_system_metrics(&self) -> Option<SystemMetrics> {
        self.system_metrics_history.read().await.last().cloned()
    }

    /// Process monitoring event / 处理监控事件
    async fn process_event(
        event: &MonitoringEvent,
        connection_metrics: &Arc<RwLock<HashMap<String, ConnectionMetrics>>>,
        message_metrics: &Arc<RwLock<HashMap<MessageType, MessageMetrics>>>,
        _system_metrics_history: &Arc<RwLock<Vec<SystemMetrics>>>,
        _performance_events: &Arc<RwLock<HashMap<String, PerformanceEvent>>>,
        _config: &MonitoringConfig,
    ) {
        match event {
            MonitoringEvent::Connection {
                connection_id,
                event,
                timestamp,
            } => {
                Self::update_connection_metrics(
                    connection_metrics,
                    connection_id,
                    event,
                    *timestamp,
                )
                .await;
            }
            MonitoringEvent::Message {
                connection_id,
                message_type,
                direction,
                size_bytes,
                processing_time_ms,
                timestamp,
            } => {
                Self::update_message_metrics(message_metrics, message_type, *processing_time_ms)
                    .await;

                Self::update_connection_message_metrics(
                    connection_metrics,
                    connection_id,
                    *direction,
                    *size_bytes,
                    *timestamp,
                )
                .await;
            }
            MonitoringEvent::Error { connection_id, .. } => {
                if let Some(conn_id) = connection_id {
                    Self::increment_connection_error_count(connection_metrics, conn_id).await;
                }
            }
            MonitoringEvent::Performance { .. } => {
                // Performance events are already processed
                // 性能事件已经被处理
            }
        }
    }

    /// Update connection metrics / 更新连接指标
    async fn update_connection_metrics(
        connection_metrics: &Arc<RwLock<HashMap<String, ConnectionMetrics>>>,
        connection_id: &str,
        event: &ConnectionEvent,
        timestamp: SystemTime,
    ) {
        let mut metrics = connection_metrics.write().await;

        match event {
            ConnectionEvent::Connected { .. } => {
                let new_metrics = ConnectionMetrics {
                    connection_id: connection_id.to_string(),
                    state: ConnectionStatus::Active,
                    connected_at: timestamp,
                    last_activity: timestamp,
                    messages_sent: 0,
                    messages_received: 0,
                    bytes_sent: 0,
                    bytes_received: 0,
                    error_count: 0,
                    avg_response_time_ms: 0.0,
                };
                metrics.insert(connection_id.to_string(), new_metrics);
            }
            ConnectionEvent::Disconnected { .. } => {
                if let Some(metric) = metrics.get_mut(connection_id) {
                    metric.state = ConnectionStatus::Idle;
                    metric.last_activity = timestamp;
                }
            }
            ConnectionEvent::Error { .. } => {
                if let Some(metric) = metrics.get_mut(connection_id) {
                    metric.error_count += 1;
                    metric.last_activity = timestamp;
                }
            }
            _ => {
                if let Some(metric) = metrics.get_mut(connection_id) {
                    metric.last_activity = timestamp;
                }
            }
        }
    }

    /// Update message metrics / 更新消息指标
    async fn update_message_metrics(
        message_metrics: &Arc<RwLock<HashMap<MessageType, MessageMetrics>>>,
        message_type: &MessageType,
        processing_time_ms: Option<f64>,
    ) {
        let mut metrics = message_metrics.write().await;
        let metric = metrics
            .entry(message_type.clone())
            .or_insert_with(|| MessageMetrics {
                message_type: message_type.clone(),
                total_count: 0,
                success_count: 0,
                error_count: 0,
                avg_processing_time_ms: 0.0,
                max_processing_time_ms: 0.0,
                min_processing_time_ms: f64::MAX,
            });

        metric.total_count += 1;

        if let Some(time_ms) = processing_time_ms {
            metric.success_count += 1;

            // Update processing time statistics / 更新处理时间统计
            let total_time =
                metric.avg_processing_time_ms * (metric.success_count - 1) as f64 + time_ms;
            metric.avg_processing_time_ms = total_time / metric.success_count as f64;
            metric.max_processing_time_ms = metric.max_processing_time_ms.max(time_ms);
            metric.min_processing_time_ms = metric.min_processing_time_ms.min(time_ms);
        } else {
            metric.error_count += 1;
        }
    }

    /// Update connection message metrics / 更新连接消息指标
    async fn update_connection_message_metrics(
        connection_metrics: &Arc<RwLock<HashMap<String, ConnectionMetrics>>>,
        connection_id: &str,
        direction: MessageDirection,
        size_bytes: usize,
        timestamp: SystemTime,
    ) {
        let mut metrics = connection_metrics.write().await;
        if let Some(metric) = metrics.get_mut(connection_id) {
            match direction {
                MessageDirection::Incoming => {
                    metric.messages_received += 1;
                    metric.bytes_received += size_bytes as u64;
                }
                MessageDirection::Outgoing => {
                    metric.messages_sent += 1;
                    metric.bytes_sent += size_bytes as u64;
                }
            }
            metric.last_activity = timestamp;
        }
    }

    /// Increment connection error count / 增加连接错误计数
    async fn increment_connection_error_count(
        connection_metrics: &Arc<RwLock<HashMap<String, ConnectionMetrics>>>,
        connection_id: &str,
    ) {
        let mut metrics = connection_metrics.write().await;
        if let Some(metric) = metrics.get_mut(connection_id) {
            metric.error_count += 1;
        }
    }

    /// Collect system metrics / 收集系统指标
    async fn collect_system_metrics(
        system_metrics_history: &Arc<RwLock<Vec<SystemMetrics>>>,
        connection_metrics: &Arc<RwLock<HashMap<String, ConnectionMetrics>>>,
        max_history_size: usize,
    ) {
        let connections = connection_metrics.read().await;
        let active_connections = connections
            .values()
            .filter(|m| m.state == ConnectionStatus::Active)
            .count();

        let total_connections = connections.len() as u64;
        let total_messages: u64 = connections
            .values()
            .map(|m| m.messages_sent + m.messages_received)
            .sum();
        let total_bytes: u64 = connections
            .values()
            .map(|m| m.bytes_sent + m.bytes_received)
            .sum();
        let total_errors: u64 = connections.values().map(|m| m.error_count).sum();

        drop(connections);

        let metrics = SystemMetrics {
            timestamp: SystemTime::now(),
            active_connections,
            total_connections,
            messages_per_second: total_messages as f64 / 60.0, // Approximate
            bytes_per_second: total_bytes as f64 / 60.0,       // Approximate
            memory_usage_bytes: 0,  // TODO: Implement actual memory tracking
            cpu_usage_percent: 0.0, // TODO: Implement actual CPU tracking
            error_rate: if total_messages > 0 {
                total_errors as f64 / total_messages as f64
            } else {
                0.0
            },
        };

        let mut history = system_metrics_history.write().await;
        history.push(metrics);

        // Keep history size under limit / 保持历史大小在限制内
        if history.len() > max_history_size {
            history.remove(0);
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::spearlet::execution::communication::protocol::AuthRequest;

    #[tokio::test]
    async fn test_monitoring_service_creation() {
        let config = MonitoringConfig::default();
        let service = MonitoringService::new(config);

        assert!(service.get_connection_metrics().await.is_empty());
        assert!(service.get_message_metrics().await.is_empty());
        assert!(service.get_system_metrics_history().await.is_empty());
    }

    #[tokio::test]
    async fn test_connection_event_recording() {
        let config = MonitoringConfig::default();
        let service = MonitoringService::new(config);

        service.start().await.unwrap();

        service
            .record_connection_event(
                "test-conn-1".to_string(),
                ConnectionEvent::Connected {
                    connection_id: "test-conn-1".to_string(),
                    remote_addr: "127.0.0.1:8080".parse().unwrap(),
                },
            )
            .await;

        // Give some time for event processing / 给事件处理一些时间
        tokio::time::sleep(Duration::from_millis(100)).await;

        let metrics = service.get_connection_metrics().await;
        assert!(metrics.contains_key("test-conn-1"));
    }

    #[tokio::test]
    async fn test_message_event_recording() {
        let config = MonitoringConfig::default();
        let service = MonitoringService::new(config);

        service.start().await.unwrap();

        let message = SpearMessage {
            message_type: MessageType::AuthRequest,
            request_id: 12345,
            timestamp: SystemTime::now(),
            payload: serde_json::to_vec(&AuthRequest {
                instance_id: "test-instance".to_string(),
                token: "test-token".to_string(),
                client_version: "1.0.0".to_string(),
                client_type: "process".to_string(),
                extra_params: std::collections::HashMap::new(),
            })
            .unwrap(),
            version: 1,
        };

        service
            .record_message_event(
                "test-conn-1".to_string(),
                &message,
                MessageDirection::Incoming,
                Some(10.5),
            )
            .await;

        // Give some time for event processing / 给事件处理一些时间
        tokio::time::sleep(Duration::from_millis(100)).await;

        let metrics = service.get_message_metrics().await;
        assert!(metrics.contains_key(&MessageType::AuthRequest));
    }
}
