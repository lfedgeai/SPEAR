use crate::spearlet::config::SpearletConfig;
use std::fmt::{Display, Formatter};
use std::time::Duration;
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
