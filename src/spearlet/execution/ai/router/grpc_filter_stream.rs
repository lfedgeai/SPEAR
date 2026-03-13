//! Router gRPC filter stream integration (hub + service).
//! Router gRPC 过滤 stream 集成（Hub + Service）。

use std::collections::{HashMap, HashSet, VecDeque};
use std::pin::Pin;
use std::sync::atomic::{AtomicI64, AtomicU64, Ordering};
use std::sync::{Arc, OnceLock};
use std::time::{Duration, SystemTime, UNIX_EPOCH};

use tokio::sync::{mpsc, Semaphore};
use tokio_stream::wrappers::ReceiverStream;
use tokio_stream::Stream;
use tokio_stream::StreamExt;
use tonic::{Request, Response, Status};

use crate::proto::spearlet::{
    router_filter_stream_service_server::RouterFilterStreamService, Candidate, CandidateDecision,
    CandidateRuntimeHints, DecisionAction, FilterRequest, FilterResponse, FinalAction, Heartbeat,
    Operation as ProtoOperation, Ping, RegisterRequest, RegisterResponse, Reject,
    RequestFetchRequest, RequestFetchResponse, RequestSignals, Requirements, RoutingHints,
    StreamClientMessage, StreamServerMessage,
};
use crate::spearlet::config::RouterGrpcFilterStreamConfig;
use crate::spearlet::execution::ai::ir::{CanonicalError, CanonicalRequestEnvelope, Operation};
use crate::spearlet::execution::ai::router::registry::{BackendInstance, Hosting};

static GLOBAL_HUB: OnceLock<Arc<RouterFilterStreamHub>> = OnceLock::new();

fn now_ms() -> i64 {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .unwrap_or_default()
        .as_millis() as i64
}

fn to_proto_operation(op: &Operation) -> i32 {
    match op {
        Operation::ChatCompletions => ProtoOperation::ChatCompletions as i32,
        Operation::Embeddings => ProtoOperation::Embeddings as i32,
        Operation::ImageGeneration => ProtoOperation::ImageGeneration as i32,
        Operation::SpeechToText => ProtoOperation::SpeechToText as i32,
        Operation::TextToSpeech => ProtoOperation::TextToSpeech as i32,
        Operation::RealtimeVoice => ProtoOperation::RealtimeVoice as i32,
    }
}

fn to_operation_name(op: &Operation) -> &'static str {
    match op {
        Operation::ChatCompletions => "chat_completions",
        Operation::Embeddings => "embeddings",
        Operation::ImageGeneration => "image_generation",
        Operation::SpeechToText => "speech_to_text",
        Operation::TextToSpeech => "text_to_speech",
        Operation::RealtimeVoice => "realtime_voice",
    }
}

fn requested_model(req: &CanonicalRequestEnvelope) -> Option<&str> {
    match &req.payload {
        crate::spearlet::execution::ai::ir::Payload::ChatCompletions(p) => Some(p.model.as_str()),
        crate::spearlet::execution::ai::ir::Payload::Embeddings(p) => p.model.as_deref(),
        crate::spearlet::execution::ai::ir::Payload::ImageGeneration(p) => p.model.as_deref(),
        crate::spearlet::execution::ai::ir::Payload::SpeechToText(p) => p.model.as_deref(),
        crate::spearlet::execution::ai::ir::Payload::TextToSpeech(p) => p.model.as_deref(),
        crate::spearlet::execution::ai::ir::Payload::RealtimeVoice(p) => p.model.as_deref(),
    }
}

fn build_signals(req: &CanonicalRequestEnvelope) -> RequestSignals {
    match &req.payload {
        crate::spearlet::execution::ai::ir::Payload::ChatCompletions(p) => {
            let mut approx: u32 = 0;
            for m in p.messages.iter() {
                if let Some(s) = m.content.as_str() {
                    approx = approx.saturating_add(s.len().min(u32::MAX as usize) as u32);
                }
            }
            let uses_tools = !p.tools.is_empty();
            let uses_json_schema = p
                .params
                .get("response_format")
                .and_then(|v| v.get("json_schema"))
                .is_some();
            RequestSignals {
                model: p.model.clone(),
                message_count: p.messages.len().min(u32::MAX as usize) as u32,
                approx_text_bytes: approx,
                uses_tools,
                uses_json_schema,
                ..Default::default()
            }
        }
        crate::spearlet::execution::ai::ir::Payload::SpeechToText(p) => RequestSignals {
            model: p.model.clone().unwrap_or_default(),
            ..Default::default()
        },
        _ => RequestSignals {
            model: requested_model(req).unwrap_or_default().to_string(),
            ..Default::default()
        },
    }
}

/// RouterFilterStreamHub is a process-local broker for filter agents.
/// RouterFilterStreamHub 是进程内的 filter agent broker。
pub struct RouterFilterStreamHub {
    pub config: RouterGrpcFilterStreamConfig,
    agents: parking_lot::RwLock<HashMap<String, AgentHandle>>,
    rr: AtomicU64,
    inflight: parking_lot::Mutex<HashMap<String, std::sync::mpsc::Sender<FilterResponse>>>,
    total_inflight: Arc<Semaphore>,
    tokens: parking_lot::RwLock<HashMap<String, TokenRecord>>,
    request_cache: parking_lot::Mutex<HashMap<String, CachedRequest>>,
    request_order: parking_lot::Mutex<VecDeque<String>>,
}

#[derive(Debug, Clone)]
struct TokenRecord {
    agent_id: String,
    expire_at_ms: i64,
}

#[derive(Debug, Clone)]
struct CachedRequest {
    req: CanonicalRequestEnvelope,
    expire_at_ms: i64,
}

/// AgentHandle is a registered filter agent connection.
/// AgentHandle 是已注册的 filter agent 连接句柄。
#[derive(Clone)]
pub struct AgentHandle {
    pub agent_id: String,
    pub tx: mpsc::Sender<StreamServerMessage>,
    pub inflight: Arc<Semaphore>,
    pub supported_operations: HashSet<i32>,
    pub max_candidates: usize,
    pub last_heartbeat_ms: Arc<AtomicI64>,
}

/// FilterTrace describes the applied filter result for logging/debugging.
/// FilterTrace 描述已应用的 filter 结果（用于日志/排障）。
#[derive(Debug, Clone, Default)]
pub struct FilterTrace {
    pub decision_id: Option<String>,
    pub dropped: Vec<String>,
    pub weight_overrides: Vec<(String, u32)>,
    pub priority_overrides: Vec<(String, i32)>,
    pub reason_codes_by_candidate: HashMap<String, Vec<String>>,
    pub final_action: Option<FinalActionTrace>,
    pub failed: bool,
    pub failure_message: Option<String>,
}

/// FinalActionTrace is the normalized final action.
/// FinalActionTrace 是归一化后的 final action。
#[derive(Debug, Clone, Default)]
pub struct FinalActionTrace {
    pub reject_request: bool,
    pub reject_code: Option<String>,
    pub force_backend: Option<String>,
}

impl RouterFilterStreamHub {
    /// Initialize global hub (idempotent).
    /// 初始化全局 hub（幂等）。
    pub fn init_global(config: RouterGrpcFilterStreamConfig) -> Arc<Self> {
        GLOBAL_HUB
            .get_or_init(|| Arc::new(Self::new(config)))
            .clone()
    }

    /// Get global hub if already initialized.
    /// 获取已初始化的全局 hub。
    pub fn global() -> Option<Arc<Self>> {
        GLOBAL_HUB.get().cloned()
    }

    pub fn new(config: RouterGrpcFilterStreamConfig) -> Self {
        let max_total = config.max_inflight_total.max(1);
        Self {
            config,
            agents: parking_lot::RwLock::new(HashMap::new()),
            rr: AtomicU64::new(0),
            inflight: parking_lot::Mutex::new(HashMap::new()),
            total_inflight: Arc::new(Semaphore::new(max_total)),
            tokens: parking_lot::RwLock::new(HashMap::new()),
            request_cache: parking_lot::Mutex::new(HashMap::new()),
            request_order: parking_lot::Mutex::new(VecDeque::new()),
        }
    }

    pub fn register_agent(&self, h: AgentHandle) {
        self.agents.write().insert(h.agent_id.clone(), h);
    }

    pub fn unregister_agent(&self, agent_id: &str) {
        self.agents.write().remove(agent_id);
        self.tokens
            .write()
            .retain(|_, rec| rec.agent_id.as_str() != agent_id);
    }

    pub fn update_heartbeat(&self, agent_id: &str, now_ms: i64) {
        if let Some(h) = self.agents.read().get(agent_id) {
            h.last_heartbeat_ms.store(now_ms, Ordering::Relaxed);
        }
    }

    pub fn handle_filter_response(&self, resp: FilterResponse) {
        let cid = resp.correlation_id.clone();
        let tx = self.inflight.lock().remove(&cid);
        if let Some(tx) = tx {
            let _ = tx.send(resp);
        }
    }

    pub fn issue_session_token(&self, agent_id: &str) -> (String, i64) {
        let token = uuid::Uuid::new_v4().to_string();
        let expire_at_ms = now_ms() + self.config.session_token_ttl_ms as i64;
        self.tokens.write().insert(
            token.clone(),
            TokenRecord {
                agent_id: agent_id.to_string(),
                expire_at_ms,
            },
        );
        (token, expire_at_ms)
    }

    pub fn validate_session_token(&self, token: &str) -> Option<String> {
        let now = now_ms();
        let mut m = self.tokens.write();
        let rec = m.get(token).cloned()?;
        if rec.expire_at_ms <= now {
            m.remove(token);
            return None;
        }
        Some(rec.agent_id)
    }

    pub fn cache_request_for_fetch(&self, req: CanonicalRequestEnvelope) {
        if !self.config.content_fetch_enabled {
            return;
        }
        let now = now_ms();
        let expire_at_ms = now + self.config.content_fetch_cache_ttl_ms as i64;
        let id = req.request_id.clone();
        self.request_cache
            .lock()
            .insert(id.clone(), CachedRequest { req, expire_at_ms });
        let mut order = self.request_order.lock();
        order.push_back(id);
        let max_entries = self.config.content_fetch_cache_max_entries.max(1);
        while order.len() > max_entries {
            if let Some(old) = order.pop_front() {
                self.request_cache.lock().remove(&old);
            }
        }
    }

    pub fn get_cached_request(&self, request_id: &str) -> Option<CanonicalRequestEnvelope> {
        let now = now_ms();
        let mut cache = self.request_cache.lock();
        let Some(rec) = cache.get(request_id).cloned() else {
            return None;
        };
        if rec.expire_at_ms <= now {
            cache.remove(request_id);
            return None;
        }
        Some(rec.req)
    }

    fn select_agent(&self, op: i32) -> Option<AgentHandle> {
        let agents = self.agents.read();
        if agents.is_empty() {
            return None;
        }
        let mut v: Vec<AgentHandle> = agents.values().cloned().collect();
        v.sort_by(|a, b| a.agent_id.cmp(&b.agent_id));
        let start = (self.rr.fetch_add(1, Ordering::Relaxed) as usize) % v.len();
        for i in 0..v.len() {
            let idx = (start + i) % v.len();
            let h = &v[idx];
            if !h.supported_operations.is_empty() && !h.supported_operations.contains(&op) {
                continue;
            }
            return Some(h.clone());
        }
        None
    }

    /// try_filter_candidates_blocking sends a filter request and waits for response.
    /// try_filter_candidates_blocking 发送过滤请求并阻塞等待响应。
    pub fn try_filter_candidates_blocking(
        &self,
        req: &CanonicalRequestEnvelope,
        candidates: &[&BackendInstance],
        decision_timeout_ms: u64,
    ) -> Result<(FilterResponse, FilterTrace), CanonicalError> {
        if !self.config.enabled {
            return Err(CanonicalError {
                code: "router_filter_disabled".to_string(),
                message: "router filter disabled".to_string(),
                retryable: false,
                operation: Some(req.operation.clone()),
            });
        }

        let op = to_proto_operation(&req.operation);
        let Some(agent) = self.select_agent(op) else {
            return Err(CanonicalError {
                code: "router_filter_unavailable".to_string(),
                message: "no router filter agent connected".to_string(),
                retryable: true,
                operation: Some(req.operation.clone()),
            });
        };

        let total_permit = self
            .total_inflight
            .try_acquire()
            .map_err(|_| CanonicalError {
                code: "router_filter_busy".to_string(),
                message: "router filter inflight limit reached".to_string(),
                retryable: true,
                operation: Some(req.operation.clone()),
            })?;

        let agent_permit = agent.inflight.try_acquire().map_err(|_| CanonicalError {
            code: "router_filter_busy".to_string(),
            message: "router filter agent inflight limit reached".to_string(),
            retryable: true,
            operation: Some(req.operation.clone()),
        })?;

        let correlation_id = uuid::Uuid::new_v4().to_string();

        if self.config.content_fetch_enabled {
            self.cache_request_for_fetch(req.clone());
        }

        let (wait_tx, wait_rx) = std::sync::mpsc::channel::<FilterResponse>();
        self.inflight.lock().insert(correlation_id.clone(), wait_tx);

        let max_candidates = self
            .config
            .max_candidates_sent
            .min(agent.max_candidates.max(1))
            .max(1);
        let mut proto_candidates: Vec<Candidate> = Vec::new();
        for c in candidates.iter().take(max_candidates) {
            let ops = c
                .capabilities
                .ops
                .iter()
                .map(to_operation_name)
                .map(|s| s.to_string())
                .collect();
            proto_candidates.push(Candidate {
                name: c.name.clone(),
                kind: c.kind.clone(),
                base_url: c.base_url.clone(),
                model: c.model.clone().unwrap_or_default(),
                weight: c.weight,
                priority: c.priority,
                ops,
                features: c.capabilities.features.clone(),
                transports: c.capabilities.transports.clone(),
                is_local: c.hosting == Hosting::Local,
                runtime: Some(CandidateRuntimeHints::default()),
            });
        }

        let routing = RoutingHints {
            backend: req.routing.backend.clone().unwrap_or_default(),
            allowlist: req.routing.allowlist.clone(),
            denylist: req.routing.denylist.clone(),
            requested_model: requested_model(req).unwrap_or_default().to_string(),
        };

        let requirements = Requirements {
            required_features: req.requirements.required_features.clone(),
            required_transports: req.requirements.required_transports.clone(),
        };

        let msg = StreamServerMessage {
            msg: Some(
                crate::proto::spearlet::stream_server_message::Msg::FilterRequest(FilterRequest {
                    correlation_id: correlation_id.clone(),
                    request_id: req.request_id.clone(),
                    operation: op,
                    decision_timeout_ms: decision_timeout_ms.min(u32::MAX as u64) as u32,
                    meta: req.meta.clone(),
                    routing: Some(routing),
                    requirements: Some(requirements),
                    signals: Some(build_signals(req)),
                    candidates: proto_candidates,
                }),
            ),
        };

        if agent.tx.try_send(msg).is_err() {
            self.inflight.lock().remove(&correlation_id);
            drop(agent_permit);
            drop(total_permit);
            return Err(CanonicalError {
                code: "router_filter_unavailable".to_string(),
                message: "failed to send filter request to agent".to_string(),
                retryable: true,
                operation: Some(req.operation.clone()),
            });
        }

        let timeout = Duration::from_millis(decision_timeout_ms.max(1));
        let resp = wait_rx.recv_timeout(timeout).map_err(|_| {
            self.inflight.lock().remove(&correlation_id);
            CanonicalError {
                code: "router_filter_timeout".to_string(),
                message: "router filter decision timed out".to_string(),
                retryable: true,
                operation: Some(req.operation.clone()),
            }
        })?;
        self.inflight.lock().remove(&correlation_id);
        drop(agent_permit);
        drop(total_permit);

        let trace = trace_from_response(&resp, self.config.max_debug_kv);
        Ok((resp, trace))
    }
}

fn trace_from_response(resp: &FilterResponse, max_debug_kv: usize) -> FilterTrace {
    let mut trace = FilterTrace::default();
    if !resp.decision_id.trim().is_empty() {
        trace.decision_id = Some(resp.decision_id.clone());
    }
    let mut reason_map: HashMap<String, Vec<String>> = HashMap::new();
    for d in resp.decisions.iter() {
        if d.action == DecisionAction::Drop as i32 {
            trace.dropped.push(d.name.clone());
        }
        if let Some(w) = d.weight_override {
            trace.weight_overrides.push((d.name.clone(), w));
        }
        if let Some(p) = d.priority_override {
            trace.priority_overrides.push((d.name.clone(), p));
        }
        if !d.reason_codes.is_empty() {
            reason_map.insert(d.name.clone(), d.reason_codes.clone());
        }
    }
    trace.reason_codes_by_candidate = reason_map;
    if let Some(fa) = resp.final_action.as_ref() {
        let mut fat = FinalActionTrace::default();
        fat.reject_request = fa.reject_request;
        if !fa.reject_code.trim().is_empty() {
            fat.reject_code = Some(fa.reject_code.clone());
        }
        if !fa.force_backend.trim().is_empty() {
            fat.force_backend = Some(fa.force_backend.clone());
        }
        trace.final_action = Some(fat);
    }
    let _ = max_debug_kv;
    trace
}

/// RouterFilterStreamServiceImpl implements the gRPC service and bridges connections to the hub.
/// RouterFilterStreamServiceImpl 实现 gRPC service 并把连接桥接到 hub。
#[derive(Clone)]
pub struct RouterFilterStreamServiceImpl {
    hub: Arc<RouterFilterStreamHub>,
    protocol_version: u32,
}

impl RouterFilterStreamServiceImpl {
    pub fn new(hub: Arc<RouterFilterStreamHub>) -> Self {
        Self {
            hub,
            protocol_version: 1,
        }
    }
}

#[tonic::async_trait]
impl RouterFilterStreamService for RouterFilterStreamServiceImpl {
    type OpenStream = Pin<Box<dyn Stream<Item = Result<StreamServerMessage, Status>> + Send>>;

    async fn open(
        &self,
        request: Request<tonic::Streaming<StreamClientMessage>>,
    ) -> Result<Response<Self::OpenStream>, Status> {
        let mut inbound = request.into_inner();
        let (out_tx, out_rx) = mpsc::channel::<StreamServerMessage>(256);
        let hub = self.hub.clone();
        let protocol_version = self.protocol_version;

        tokio::spawn(async move {
            let mut agent_id: Option<String> = None;
            let mut accepted = false;
            while let Some(item) = inbound.next().await {
                let msg = match item {
                    Ok(v) => v,
                    Err(_) => break,
                };
                match msg.msg {
                    Some(crate::proto::spearlet::stream_client_message::Msg::Register(r)) => {
                        if accepted {
                            let _ = out_tx
                                .send(StreamServerMessage {
                                    msg: Some(
                                        crate::proto::spearlet::stream_server_message::Msg::Reject(
                                            Reject {
                                                code: "duplicated_register".to_string(),
                                                message: "register request already accepted"
                                                    .to_string(),
                                            },
                                        ),
                                    ),
                                })
                                .await;
                            break;
                        }
                        if !hub.config.enabled {
                            let _ = out_tx
                                .send(StreamServerMessage {
                                    msg: Some(
                                        crate::proto::spearlet::stream_server_message::Msg::RegisterOk(
                                            RegisterResponse {
                                                protocol_version,
                                                accepted: false,
                                                message: "router filter disabled".to_string(),
                                                session_token: String::new(),
                                                token_expire_at_ms: 0,
                                            },
                                        ),
                                    ),
                                })
                                .await;
                            break;
                        }
                        let id = r.agent_id.trim().to_string();
                        if id.is_empty() {
                            let _ = out_tx
                                .send(StreamServerMessage {
                                    msg: Some(
                                        crate::proto::spearlet::stream_server_message::Msg::RegisterOk(
                                            RegisterResponse {
                                                protocol_version,
                                                accepted: false,
                                                message: "agent_id is required".to_string(),
                                                session_token: String::new(),
                                                token_expire_at_ms: 0,
                                            },
                                        ),
                                    ),
                                })
                                .await;
                            break;
                        }

                        let supported_operations: HashSet<i32> =
                            r.supported_operations.iter().cloned().collect();
                        let max_inflight = if r.max_inflight == 0 {
                            hub.config.per_agent_max_inflight.max(1) as u32
                        } else {
                            r.max_inflight
                        };
                        let max_candidates = if r.max_candidates == 0 {
                            hub.config.max_candidates_sent.max(1) as u32
                        } else {
                            r.max_candidates
                        };

                        let handle = AgentHandle {
                            agent_id: id.clone(),
                            tx: out_tx.clone(),
                            inflight: Arc::new(Semaphore::new(max_inflight as usize)),
                            supported_operations,
                            max_candidates: max_candidates as usize,
                            last_heartbeat_ms: Arc::new(AtomicI64::new(now_ms())),
                        };
                        hub.register_agent(handle);
                        agent_id = Some(id);
                        accepted = true;

                        let (token, token_expire_at_ms) =
                            hub.issue_session_token(agent_id.as_ref().unwrap());
                        let _ = out_tx
                            .send(StreamServerMessage {
                                msg: Some(
                                    crate::proto::spearlet::stream_server_message::Msg::RegisterOk(
                                        RegisterResponse {
                                            protocol_version,
                                            accepted: true,
                                            message: "ok".to_string(),
                                            session_token: token,
                                            token_expire_at_ms,
                                        },
                                    ),
                                ),
                            })
                            .await;
                    }
                    Some(crate::proto::spearlet::stream_client_message::Msg::Heartbeat(hb)) => {
                        let id = match agent_id.as_ref() {
                            Some(v) => v,
                            None => continue,
                        };
                        hub.update_heartbeat(id, hb.now_ms.max(0));
                        let _ = hb;
                    }
                    Some(crate::proto::spearlet::stream_client_message::Msg::FilterResponse(r)) => {
                        hub.handle_filter_response(r);
                    }
                    None => {}
                }
            }

            if let Some(id) = agent_id.as_ref() {
                hub.unregister_agent(id);
            }
        });

        let stream = ReceiverStream::new(out_rx).map(Ok);
        Ok(Response::new(Box::pin(stream) as Self::OpenStream))
    }

    async fn fetch_request_by_id(
        &self,
        request: Request<RequestFetchRequest>,
    ) -> Result<Response<RequestFetchResponse>, Status> {
        if !self.hub.config.content_fetch_enabled {
            return Err(Status::permission_denied("content fetch disabled"));
        }
        let req = request.into_inner();
        if req.request_id.trim().is_empty() {
            return Err(Status::invalid_argument("request_id is required"));
        }
        if req.session_token.trim().is_empty() {
            return Err(Status::unauthenticated("session_token is required"));
        }

        let agent_id = self
            .hub
            .validate_session_token(req.session_token.trim())
            .ok_or_else(|| Status::unauthenticated("invalid or expired session_token"))?;
        if !self.hub.agents.read().contains_key(&agent_id) {
            return Err(Status::unauthenticated("agent not connected"));
        }

        let cached = self
            .hub
            .get_cached_request(req.request_id.trim())
            .ok_or_else(|| Status::not_found("request not found"))?;
        let bytes = serde_json::to_vec(&cached)
            .map_err(|_| Status::internal("failed to serialize request"))?;
        let cap = self.hub.config.content_fetch_max_bytes.max(1);
        let req_cap = if req.max_bytes == 0 {
            cap
        } else {
            (req.max_bytes as usize).min(cap)
        };
        if bytes.len() > req_cap {
            return Err(Status::resource_exhausted("payload too large"));
        }

        Ok(Response::new(RequestFetchResponse {
            request_id: cached.request_id,
            content_type: "application/json".to_string(),
            payload: bytes,
        }))
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::spearlet::execution::ai::backends::stub::StubBackendAdapter;
    use crate::spearlet::execution::ai::ir::{
        ChatCompletionsPayload, ChatMessage, Payload, ResultPayload, RoutingHints,
    };
    use crate::spearlet::execution::ai::router::capabilities::Capabilities;
    use crate::spearlet::execution::ai::router::policy::SelectionPolicy;
    use crate::spearlet::execution::ai::router::registry::BackendRegistry;
    use crate::spearlet::execution::ai::router::Router;
    use serde_json::Value;
    use std::sync::Arc;
    use tonic::Request;

    fn chat_req(model: &str) -> CanonicalRequestEnvelope {
        CanonicalRequestEnvelope {
            version: 1,
            request_id: "r1".to_string(),
            operation: Operation::ChatCompletions,
            meta: HashMap::new(),
            routing: RoutingHints::default(),
            requirements: Default::default(),
            timeout_ms: Some(50),
            payload: Payload::ChatCompletions(ChatCompletionsPayload {
                model: model.to_string(),
                messages: vec![ChatMessage {
                    role: "user".to_string(),
                    content: Value::String("hi".to_string()),
                    tool_call_id: None,
                    tool_calls: None,
                    name: None,
                }],
                tools: vec![],
                params: HashMap::new(),
            }),
            extra: HashMap::new(),
        }
    }

    #[test]
    fn test_filter_hub_drops_candidate() {
        let hub = Arc::new(RouterFilterStreamHub::new(RouterGrpcFilterStreamConfig {
            enabled: true,
            decision_timeout_ms: 100,
            ..Default::default()
        }));

        let (tx, mut rx) = mpsc::channel::<StreamServerMessage>(8);
        let agent = AgentHandle {
            agent_id: "a1".to_string(),
            tx,
            inflight: Arc::new(Semaphore::new(16)),
            supported_operations: HashSet::new(),
            max_candidates: 64,
            last_heartbeat_ms: Arc::new(AtomicI64::new(now_ms())),
        };
        hub.register_agent(agent);

        let openai = crate::spearlet::execution::ai::router::registry::BackendInstance {
            name: "openai".to_string(),
            kind: "openai_chat_completion".to_string(),
            base_url: "https://api.openai.com/v1".to_string(),
            hosting: crate::spearlet::execution::ai::router::registry::Hosting::Remote,
            model: Some("gpt".to_string()),
            weight: 100,
            priority: 0,
            capabilities: Capabilities {
                ops: vec![Operation::ChatCompletions],
                features: vec![],
                transports: vec!["http".to_string()],
            },
            adapter: Arc::new(StubBackendAdapter::new("openai")),
        };
        let ollama = crate::spearlet::execution::ai::router::registry::BackendInstance {
            name: "ollama".to_string(),
            kind: "ollama_chat".to_string(),
            base_url: "http://127.0.0.1:11434".to_string(),
            hosting: crate::spearlet::execution::ai::router::registry::Hosting::Local,
            model: Some("gpt".to_string()),
            weight: 100,
            priority: 0,
            capabilities: Capabilities {
                ops: vec![Operation::ChatCompletions],
                features: vec![],
                transports: vec!["http".to_string()],
            },
            adapter: Arc::new(StubBackendAdapter::new("ollama")),
        };

        let router = Router::new_with_filter(
            BackendRegistry::new(vec![openai, ollama]),
            SelectionPolicy::WeightedRandom,
            Some(hub.clone()),
        );

        std::thread::spawn(move || {
            let msg = rx.blocking_recv().unwrap();
            let req = match msg.msg.unwrap() {
                crate::proto::spearlet::stream_server_message::Msg::FilterRequest(r) => r,
                _ => return,
            };
            let resp = FilterResponse {
                correlation_id: req.correlation_id,
                decision_id: "d1".to_string(),
                decisions: vec![CandidateDecision {
                    name: "openai".to_string(),
                    action: DecisionAction::Drop as i32,
                    weight_override: None,
                    priority_override: None,
                    score: None,
                    reason_codes: vec!["test_drop".to_string()],
                }],
                final_action: Some(FinalAction {
                    reject_request: false,
                    reject_code: String::new(),
                    reject_message: String::new(),
                    force_backend: String::new(),
                }),
                debug: HashMap::new(),
            };
            hub.handle_filter_response(resp);
        });

        let req = chat_req("gpt");
        let inst = router.route(&req).unwrap();
        assert_eq!(inst.name, "ollama");
        let resp = inst.adapter.invoke(&req).unwrap();
        match resp.result {
            ResultPayload::Payload(_) => {}
            _ => panic!("unexpected"),
        }
    }

    #[test]
    fn test_filter_hub_force_backend() {
        let hub = Arc::new(RouterFilterStreamHub::new(RouterGrpcFilterStreamConfig {
            enabled: true,
            decision_timeout_ms: 100,
            ..Default::default()
        }));

        let (tx, mut rx) = mpsc::channel::<StreamServerMessage>(8);
        let agent = AgentHandle {
            agent_id: "a1".to_string(),
            tx,
            inflight: Arc::new(Semaphore::new(16)),
            supported_operations: HashSet::new(),
            max_candidates: 64,
            last_heartbeat_ms: Arc::new(AtomicI64::new(now_ms())),
        };
        hub.register_agent(agent);

        let openai = crate::spearlet::execution::ai::router::registry::BackendInstance {
            name: "openai".to_string(),
            kind: "openai_chat_completion".to_string(),
            base_url: "https://api.openai.com/v1".to_string(),
            hosting: crate::spearlet::execution::ai::router::registry::Hosting::Remote,
            model: Some("gpt".to_string()),
            weight: 100,
            priority: 0,
            capabilities: Capabilities {
                ops: vec![Operation::ChatCompletions],
                features: vec![],
                transports: vec!["http".to_string()],
            },
            adapter: Arc::new(StubBackendAdapter::new("openai")),
        };
        let ollama = crate::spearlet::execution::ai::router::registry::BackendInstance {
            name: "ollama".to_string(),
            kind: "ollama_chat".to_string(),
            base_url: "http://127.0.0.1:11434".to_string(),
            hosting: crate::spearlet::execution::ai::router::registry::Hosting::Local,
            model: Some("gpt".to_string()),
            weight: 100,
            priority: 0,
            capabilities: Capabilities {
                ops: vec![Operation::ChatCompletions],
                features: vec![],
                transports: vec!["http".to_string()],
            },
            adapter: Arc::new(StubBackendAdapter::new("ollama")),
        };

        let router = Router::new_with_filter(
            BackendRegistry::new(vec![openai, ollama]),
            SelectionPolicy::WeightedRandom,
            Some(hub.clone()),
        );

        std::thread::spawn(move || {
            let msg = rx.blocking_recv().unwrap();
            let req = match msg.msg.unwrap() {
                crate::proto::spearlet::stream_server_message::Msg::FilterRequest(r) => r,
                _ => return,
            };
            let resp = FilterResponse {
                correlation_id: req.correlation_id,
                decision_id: "d1".to_string(),
                decisions: vec![],
                final_action: Some(FinalAction {
                    reject_request: false,
                    reject_code: String::new(),
                    reject_message: String::new(),
                    force_backend: "openai".to_string(),
                }),
                debug: HashMap::new(),
            };
            hub.handle_filter_response(resp);
        });

        let req = chat_req("gpt");
        let inst = router.route(&req).unwrap();
        assert_eq!(inst.name, "openai");
    }

    #[test]
    fn test_filter_hub_rejects_request() {
        let hub = Arc::new(RouterFilterStreamHub::new(RouterGrpcFilterStreamConfig {
            enabled: true,
            decision_timeout_ms: 100,
            ..Default::default()
        }));

        let (tx, mut rx) = mpsc::channel::<StreamServerMessage>(8);
        let agent = AgentHandle {
            agent_id: "a1".to_string(),
            tx,
            inflight: Arc::new(Semaphore::new(16)),
            supported_operations: HashSet::new(),
            max_candidates: 64,
            last_heartbeat_ms: Arc::new(AtomicI64::new(now_ms())),
        };
        hub.register_agent(agent);

        let openai = crate::spearlet::execution::ai::router::registry::BackendInstance {
            name: "openai".to_string(),
            kind: "openai_chat_completion".to_string(),
            base_url: "https://api.openai.com/v1".to_string(),
            hosting: crate::spearlet::execution::ai::router::registry::Hosting::Remote,
            model: Some("gpt".to_string()),
            weight: 100,
            priority: 0,
            capabilities: Capabilities {
                ops: vec![Operation::ChatCompletions],
                features: vec![],
                transports: vec!["http".to_string()],
            },
            adapter: Arc::new(StubBackendAdapter::new("openai")),
        };

        let router = Router::new_with_filter(
            BackendRegistry::new(vec![openai]),
            SelectionPolicy::WeightedRandom,
            Some(hub.clone()),
        );

        std::thread::spawn(move || {
            let msg = rx.blocking_recv().unwrap();
            let req = match msg.msg.unwrap() {
                crate::proto::spearlet::stream_server_message::Msg::FilterRequest(r) => r,
                _ => return,
            };
            let resp = FilterResponse {
                correlation_id: req.correlation_id,
                decision_id: "d1".to_string(),
                decisions: vec![],
                final_action: Some(FinalAction {
                    reject_request: true,
                    reject_code: "policy_denied".to_string(),
                    reject_message: "blocked".to_string(),
                    force_backend: String::new(),
                }),
                debug: HashMap::new(),
            };
            hub.handle_filter_response(resp);
        });

        let req = chat_req("gpt");
        let err = match router.route(&req) {
            Ok(_) => panic!("expected error"),
            Err(e) => e,
        };
        assert_eq!(err.code, "policy_denied");
    }

    #[tokio::test]
    async fn test_fetch_request_by_id_ok() {
        let hub = Arc::new(RouterFilterStreamHub::new(RouterGrpcFilterStreamConfig {
            enabled: true,
            content_fetch_enabled: true,
            content_fetch_max_bytes: 1024 * 1024,
            ..Default::default()
        }));
        let svc = RouterFilterStreamServiceImpl::new(hub.clone());

        hub.register_agent(AgentHandle {
            agent_id: "a1".to_string(),
            tx: mpsc::channel::<StreamServerMessage>(1).0,
            inflight: Arc::new(Semaphore::new(1)),
            supported_operations: HashSet::new(),
            max_candidates: 1,
            last_heartbeat_ms: Arc::new(AtomicI64::new(now_ms())),
        });
        let (token, _exp) = hub.issue_session_token("a1");
        hub.cache_request_for_fetch(chat_req("gpt"));

        let resp = svc
            .fetch_request_by_id(Request::new(RequestFetchRequest {
                request_id: "r1".to_string(),
                session_token: token,
                max_bytes: 0,
            }))
            .await
            .unwrap()
            .into_inner();
        assert_eq!(resp.request_id, "r1");
        assert_eq!(resp.content_type, "application/json");
        assert!(!resp.payload.is_empty());
    }

    #[tokio::test]
    async fn test_fetch_request_by_id_invalid_token() {
        let hub = Arc::new(RouterFilterStreamHub::new(RouterGrpcFilterStreamConfig {
            enabled: true,
            content_fetch_enabled: true,
            ..Default::default()
        }));
        let svc = RouterFilterStreamServiceImpl::new(hub.clone());

        hub.register_agent(AgentHandle {
            agent_id: "a1".to_string(),
            tx: mpsc::channel::<StreamServerMessage>(1).0,
            inflight: Arc::new(Semaphore::new(1)),
            supported_operations: HashSet::new(),
            max_candidates: 1,
            last_heartbeat_ms: Arc::new(AtomicI64::new(now_ms())),
        });
        hub.cache_request_for_fetch(chat_req("gpt"));

        let err = svc
            .fetch_request_by_id(Request::new(RequestFetchRequest {
                request_id: "r1".to_string(),
                session_token: "bad".to_string(),
                max_bytes: 0,
            }))
            .await
            .unwrap_err();
        assert_eq!(err.code(), tonic::Code::Unauthenticated);
    }

    #[tokio::test]
    async fn test_fetch_request_by_id_too_large() {
        let hub = Arc::new(RouterFilterStreamHub::new(RouterGrpcFilterStreamConfig {
            enabled: true,
            content_fetch_enabled: true,
            content_fetch_max_bytes: 16,
            ..Default::default()
        }));
        let svc = RouterFilterStreamServiceImpl::new(hub.clone());

        hub.register_agent(AgentHandle {
            agent_id: "a1".to_string(),
            tx: mpsc::channel::<StreamServerMessage>(1).0,
            inflight: Arc::new(Semaphore::new(1)),
            supported_operations: HashSet::new(),
            max_candidates: 1,
            last_heartbeat_ms: Arc::new(AtomicI64::new(now_ms())),
        });
        let (token, _exp) = hub.issue_session_token("a1");
        hub.cache_request_for_fetch(chat_req("gpt"));

        let err = svc
            .fetch_request_by_id(Request::new(RequestFetchRequest {
                request_id: "r1".to_string(),
                session_token: token,
                max_bytes: 0,
            }))
            .await
            .unwrap_err();
        assert_eq!(err.code(), tonic::Code::ResourceExhausted);
    }
}
