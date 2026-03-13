//! Keyword-based router filter agent (drops remote candidates when keyword hits).
//! 基于关键词的 Router filter agent（命中关键词时丢弃 remote 候选）。

use std::time::{Duration, SystemTime, UNIX_EPOCH};

use clap::Parser;
use serde_json::Value;
use tokio::sync::mpsc;
use tokio_stream::wrappers::ReceiverStream;
use tokio_stream::StreamExt;
use tonic::transport::Channel;
use tracing::{info, warn};
use tracing_subscriber::EnvFilter;

use spear_next::proto::spearlet::{
    router_filter_stream_service_client::RouterFilterStreamServiceClient, CandidateDecision,
    DecisionAction, FilterRequest, FilterResponse, Heartbeat, RegisterRequest, RequestFetchRequest,
    StreamClientMessage, StreamServerMessage,
};

#[derive(Parser, Debug, Clone)]
#[command(
    name = "keyword-filter-agent",
    about = "Keyword-based router filter agent\n基于关键词的 Router filter agent"
)]
struct Args {
    /// Spearlet gRPC endpoint (host:port) / Spearlet gRPC 地址（host:port）
    #[arg(long, default_value = "127.0.0.1:50052")]
    addr: String,

    /// Agent id / Agent 标识
    #[arg(long, default_value = "keyword-filter-agent-1")]
    agent_id: String,

    /// Max inflight / 最大并发
    #[arg(long, default_value_t = 256)]
    max_inflight: u32,

    /// Max candidates / 最大候选数
    #[arg(long, default_value_t = 64)]
    max_candidates: u32,
}

const BLACKLIST_KEYWORDS: &[&str] = &[
    "secret",
    "confidential",
    "apikey",
    "password",
    "机密",
    "敏感",
];

fn now_ms() -> i64 {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .unwrap_or_default()
        .as_millis() as i64
}

fn contains_blacklist_keyword(s: &str) -> Option<&'static str> {
    let lower = s.to_ascii_lowercase();
    for &kw in BLACKLIST_KEYWORDS {
        if kw.is_ascii() {
            if lower.contains(kw) {
                return Some(kw);
            }
        } else if s.contains(kw) {
            return Some(kw);
        }
    }
    None
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum FetchStatus {
    Ok,
    None,
    InvalidOrExpiredToken,
}

async fn fetch_request_text(
    fetch_client: &mut RouterFilterStreamServiceClient<Channel>,
    session_token: Option<&str>,
    request_id: &str,
) -> (FetchStatus, Option<String>) {
    let token = match session_token {
        Some(v) => v.trim(),
        None => {
            warn!(request_id = %request_id, "[DEBUG] missing session_token");
            return (FetchStatus::None, None);
        }
    };
    if token.is_empty() {
        warn!(request_id = %request_id, "[DEBUG] missing session_token");
        return (FetchStatus::None, None);
    }
    let id = request_id.trim();
    if id.is_empty() {
        warn!("[DEBUG] empty request_id");
        return (FetchStatus::None, None);
    }

    let resp = match tokio::time::timeout(
        Duration::from_millis(1500),
        fetch_client.fetch_request_by_id(RequestFetchRequest {
            request_id: id.to_string(),
            session_token: token.to_string(),
            max_bytes: 64 * 1024,
        }),
    )
    .await
    {
        Err(_) => {
            warn!(request_id = %id, "[DEBUG] fetch_request_by_id timeout");
            return (FetchStatus::None, None);
        }
        Ok(Err(e)) => {
            if e.code() == tonic::Code::Unauthenticated
                && e.message().contains("invalid or expired session_token")
            {
                warn!(
                    request_id = %id,
                    message = %e.message(),
                    "[DEBUG] session token expired"
                );
                return (FetchStatus::InvalidOrExpiredToken, None);
            }
            warn!(
                request_id = %id,
                code = ?e.code(),
                message = %e.message(),
                "[DEBUG] fetch_request_by_id error"
            );
            return (FetchStatus::None, None);
        }
        Ok(Ok(v)) => v.into_inner(),
    };

    let s = String::from_utf8_lossy(&resp.payload).to_string();
    if !resp.content_type.eq_ignore_ascii_case("application/json") {
        return (FetchStatus::Ok, Some(s));
    }
    let v: Value = match serde_json::from_slice(resp.payload.as_slice()) {
        Ok(v) => v,
        Err(_) => return (FetchStatus::Ok, Some(s)),
    };
    (FetchStatus::Ok, serde_json::to_string(&v).ok().or(Some(s)))
}

async fn build_response(
    fetch_client: &mut RouterFilterStreamServiceClient<Channel>,
    session_token: Option<&str>,
    r: FilterRequest,
) -> (FilterResponse, bool) {
    let mut haystack = String::new();

    for (k, v) in r.meta.iter() {
        haystack.push_str(k);
        haystack.push('=');
        haystack.push_str(v);
        haystack.push('\n');
    }

    if let Some(routing) = r.routing.as_ref() {
        if !routing.requested_model.trim().is_empty() {
            haystack.push_str("requested_model=");
            haystack.push_str(routing.requested_model.trim());
            haystack.push('\n');
        }
    }

    if let Some(signals) = r.signals.as_ref() {
        if !signals.model.trim().is_empty() {
            haystack.push_str("model=");
            haystack.push_str(signals.model.trim());
            haystack.push('\n');
        }
    }

    let mut need_reconnect = false;
    match fetch_request_text(fetch_client, session_token, &r.request_id).await {
        (FetchStatus::Ok, Some(s)) => {
            haystack.push_str(&s);
        }
        (FetchStatus::InvalidOrExpiredToken, _) => {
            need_reconnect = true;
        }
        _ => {}
    }

    let hit = contains_blacklist_keyword(&haystack);
    let mut decisions: Vec<CandidateDecision> = Vec::with_capacity(r.candidates.len());
    let mut drop_count: usize = 0;
    for c in r.candidates.iter() {
        let action = if hit.is_some() && !c.is_local {
            drop_count += 1;
            DecisionAction::Drop as i32
        } else {
            DecisionAction::Keep as i32
        };
        decisions.push(CandidateDecision {
            name: c.name.clone(),
            action,
            weight_override: None,
            priority_override: None,
            score: None,
            reason_codes: Vec::new(),
        });
    }

    let mut debug = std::collections::HashMap::new();
    if need_reconnect {
        debug.insert(
            "keyword_filter_fetch_error".to_string(),
            "invalid_or_expired_session_token".to_string(),
        );
    }
    if let Some(kw) = hit {
        debug.insert("keyword_filter_hit".to_string(), kw.to_string());
        info!(
            correlation_id = %r.correlation_id,
            request_id = %r.request_id,
            keyword = %kw,
            candidates = r.candidates.len(),
            dropped = drop_count,
            "[DEBUG] keyword hit"
        );
    } else {
        info!(
            correlation_id = %r.correlation_id,
            request_id = %r.request_id,
            candidates = r.candidates.len(),
            dropped = drop_count,
            "[DEBUG] no keyword hit"
        );
    }

    let resp = FilterResponse {
        correlation_id: r.correlation_id,
        decision_id: "keyword_filter".to_string(),
        decisions,
        final_action: None,
        debug,
    };

    (resp, need_reconnect)
}

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error + Send + Sync>> {
    tracing_subscriber::fmt()
        .with_env_filter(EnvFilter::from_default_env().add_directive("info".parse()?))
        .init();

    let args = Args::parse();
    let endpoint = format!("http://{}", args.addr);
    loop {
        info!(
            endpoint = %endpoint,
            agent_id = %args.agent_id,
            max_inflight = args.max_inflight,
            max_candidates = args.max_candidates,
            "[DEBUG] keyword-filter-agent start"
        );

        let channel: Channel = Channel::from_shared(endpoint.clone())?.connect().await?;
        let mut client = RouterFilterStreamServiceClient::new(channel);
        let fetch_channel: Channel = Channel::from_shared(endpoint.clone())?.connect().await?;
        let mut fetch_client = RouterFilterStreamServiceClient::new(fetch_channel);

        let (tx, rx) = mpsc::channel::<StreamClientMessage>(256);

        tx.send(StreamClientMessage {
            msg: Some(
                spear_next::proto::spearlet::stream_client_message::Msg::Register(RegisterRequest {
                    agent_id: args.agent_id.clone(),
                    supported_operations: Vec::new(),
                    max_inflight: args.max_inflight,
                    max_candidates: args.max_candidates,
                    protocol_version: 1,
                }),
            ),
        })
        .await?;

        let response = client.open(ReceiverStream::new(rx)).await?;
        let mut inbound = response.into_inner();

        let hb_tx = tx.clone();
        let resp_tx = tx.clone();
        let hb_handle = tokio::spawn(async move {
            let mut interval = tokio::time::interval(Duration::from_secs(5));
            loop {
                interval.tick().await;
                let _ = hb_tx
                    .send(StreamClientMessage {
                        msg: Some(
                            spear_next::proto::spearlet::stream_client_message::Msg::Heartbeat(
                                Heartbeat { now_ms: now_ms() },
                            ),
                        ),
                    })
                    .await;
            }
        });

        let mut session_token: Option<String> = None;
        let mut token_expire_at_ms: i64 = 0;
        let mut reconnect = false;
        while let Some(msg) = inbound.next().await {
            let msg: StreamServerMessage = msg?;
            match msg.msg {
                Some(spear_next::proto::spearlet::stream_server_message::Msg::RegisterOk(r)) => {
                    if !r.accepted {
                        warn!(message = %r.message, "[DEBUG] register rejected");
                        reconnect = true;
                        break;
                    }
                    if !r.session_token.trim().is_empty() {
                        session_token = Some(r.session_token);
                    }
                    token_expire_at_ms = r.token_expire_at_ms;
                    info!(
                        token_expire_at_ms = r.token_expire_at_ms,
                        has_session_token = session_token
                            .as_deref()
                            .map(|s| !s.trim().is_empty())
                            .unwrap_or(false),
                        "[DEBUG] register ok"
                    );
                }
                Some(spear_next::proto::spearlet::stream_server_message::Msg::FilterRequest(r)) => {
                    if token_expire_at_ms > 0 && now_ms() + 5_000 >= token_expire_at_ms {
                        warn!(
                            token_expire_at_ms,
                            now_ms = now_ms(),
                            "[DEBUG] session token near expiry; reconnect"
                        );
                        reconnect = true;
                    }
                    info!(
                        correlation_id = %r.correlation_id,
                        request_id = %r.request_id,
                        candidates = r.candidates.len(),
                        has_session_token = session_token
                            .as_deref()
                            .map(|s| !s.trim().is_empty())
                            .unwrap_or(false),
                        "[DEBUG] filter request"
                    );
                    let (resp, need_reconnect) =
                        build_response(&mut fetch_client, session_token.as_deref(), r).await;
                    let _ = resp_tx
                        .send(StreamClientMessage {
                            msg: Some(
                                spear_next::proto::spearlet::stream_client_message::Msg::FilterResponse(
                                    resp,
                                ),
                            ),
                        })
                        .await;
                    if need_reconnect {
                        reconnect = true;
                        break;
                    }
                    if reconnect {
                        break;
                    }
                }
                Some(spear_next::proto::spearlet::stream_server_message::Msg::Ping(_)) => {}
                Some(spear_next::proto::spearlet::stream_server_message::Msg::Reject(r)) => {
                    warn!(code = %r.code, message = %r.message, "[DEBUG] rejected");
                    reconnect = true;
                    break;
                }
                None => {}
            }
        }

        hb_handle.abort();
        if reconnect {
            tokio::time::sleep(Duration::from_millis(200)).await;
            continue;
        }

        break;
    }

    Ok(())
}
