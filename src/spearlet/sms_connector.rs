use crate::spearlet::config::SpearletConfig;
use std::fmt::{Display, Formatter};
use std::time::Duration;
use tokio::time::{timeout, Instant};
use tonic::transport::{Channel, Endpoint};

#[derive(Debug, Clone)]
pub enum SmsConnectError {
    EmptyAddr,
    InvalidUrl(String),
    Timeout { attempts: u32 },
    Transport(String),
}

impl Display for SmsConnectError {
    fn fmt(&self, f: &mut Formatter<'_>) -> std::fmt::Result {
        match self {
            SmsConnectError::EmptyAddr => write!(f, "sms_grpc_addr is empty"),
            SmsConnectError::InvalidUrl(e) => write!(f, "invalid sms url: {}", e),
            SmsConnectError::Timeout { attempts } => {
                write!(f, "connect sms timeout (attempts={})", attempts)
            }
            SmsConnectError::Transport(e) => write!(f, "connect sms failed: {}", e),
        }
    }
}

impl std::error::Error for SmsConnectError {}

fn sms_endpoint(config: &SpearletConfig) -> Result<Endpoint, SmsConnectError> {
    let addr = config.sms_grpc_addr.trim();
    if addr.is_empty() {
        return Err(SmsConnectError::EmptyAddr);
    }
    let url = format!("http://{}", addr);
    let connect_timeout =
        Duration::from_millis(config.sms_connect_timeout_ms).min(Duration::from_secs(5));
    let endpoint = Endpoint::from_shared(url)
        .map_err(|e| SmsConnectError::InvalidUrl(e.to_string()))?
        .connect_timeout(connect_timeout)
        .tcp_keepalive(Some(Duration::from_secs(30)))
        .keep_alive_while_idle(true)
        .http2_keep_alive_interval(Duration::from_secs(30))
        .keep_alive_timeout(Duration::from_secs(10));
    Ok(endpoint)
}

pub fn sms_channel_lazy(config: &SpearletConfig) -> Result<Channel, SmsConnectError> {
    sms_endpoint(config).map(|e| e.connect_lazy())
}

pub async fn connect_sms_channel_once(config: &SpearletConfig) -> Result<Channel, SmsConnectError> {
    let endpoint = sms_endpoint(config)?;
    let fut = endpoint.connect();
    timeout(Duration::from_millis(config.sms_connect_timeout_ms), fut)
        .await
        .map_err(|_| SmsConnectError::Timeout { attempts: 1 })?
        .map_err(|e| SmsConnectError::Transport(e.to_string()))
}

pub async fn connect_sms_channel_retry(
    config: &SpearletConfig,
) -> Result<Channel, SmsConnectError> {
    let endpoint = sms_endpoint(config)?;
    let deadline = Instant::now() + Duration::from_millis(config.sms_connect_timeout_ms);
    let mut attempts: u32 = 0;
    let mut last_err: Option<String> = None;
    while Instant::now() < deadline {
        attempts = attempts.saturating_add(1);
        let remaining = deadline.saturating_duration_since(Instant::now());
        let attempt_timeout = remaining
            .min(Duration::from_secs(5))
            .max(Duration::from_millis(1));
        let ep = endpoint.clone();
        let fut = ep.connect();
        match timeout(attempt_timeout, fut).await {
            Ok(Ok(ch)) => return Ok(ch),
            Ok(Err(e)) => last_err = Some(e.to_string()),
            Err(_) => last_err = Some("connect attempt timeout".to_string()),
        }
        let sleep_ms = config.sms_connect_retry_ms.max(1);
        tokio::time::sleep(Duration::from_millis(sleep_ms)).await;
    }
    if let Some(e) = last_err {
        Err(SmsConnectError::Transport(e))
    } else {
        Err(SmsConnectError::Timeout { attempts })
    }
}

pub async fn connect_sms_channel_status_once(
    config: &SpearletConfig,
) -> Result<Channel, tonic::Status> {
    connect_sms_channel_once(config).await.map_err(|e| match e {
        SmsConnectError::Timeout { .. } => tonic::Status::deadline_exceeded(e.to_string()),
        _ => tonic::Status::unavailable(e.to_string()),
    })
}

pub async fn connect_sms_channel_status_retry(
    config: &SpearletConfig,
) -> Result<Channel, tonic::Status> {
    connect_sms_channel_retry(config)
        .await
        .map_err(|e| match e {
            SmsConnectError::Timeout { .. } => tonic::Status::deadline_exceeded(e.to_string()),
            _ => tonic::Status::unavailable(e.to_string()),
        })
}
