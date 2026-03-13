use std::net::SocketAddr;
use std::pin::Pin;
use std::process::Stdio;
use std::sync::Arc;

use tokio::process::Command;
use tokio::sync::{mpsc, oneshot, Mutex};
use tokio_stream::wrappers::ReceiverStream;
use tokio_stream::StreamExt;
use tonic::{Request, Response, Status};

use spear_next::proto::spearlet::router_filter_stream_service_server::{
    RouterFilterStreamService, RouterFilterStreamServiceServer,
};
use spear_next::proto::spearlet::{
    Candidate, CandidateRuntimeHints, DecisionAction, FilterRequest, FilterResponse, Heartbeat,
    Operation, RegisterRequest, RegisterResponse, RequestFetchRequest, RequestFetchResponse,
    RoutingHints, StreamClientMessage, StreamServerMessage,
};

type ServerStream =
    Pin<Box<dyn tokio_stream::Stream<Item = Result<StreamServerMessage, Status>> + Send + 'static>>;

#[derive(Clone)]
struct TestSvc {
    request_payload: Arc<Vec<u8>>,
    response_tx: Arc<Mutex<Option<oneshot::Sender<FilterResponse>>>>,
}

#[tonic::async_trait]
impl RouterFilterStreamService for TestSvc {
    type OpenStream = ServerStream;

    async fn open(
        &self,
        request: Request<tonic::Streaming<StreamClientMessage>>,
    ) -> Result<Response<Self::OpenStream>, Status> {
        let mut inbound = request.into_inner();
        let (tx, rx) = mpsc::channel::<Result<StreamServerMessage, Status>>(16);
        let svc = self.clone();

        tokio::spawn(async move {
            let first = inbound.next().await;
            let Some(Ok(StreamClientMessage { msg: Some(m) })) = first else {
                return;
            };
            let _register = match m {
                spear_next::proto::spearlet::stream_client_message::Msg::Register(r) => r,
                spear_next::proto::spearlet::stream_client_message::Msg::Heartbeat(Heartbeat {
                    ..
                }) => return,
                spear_next::proto::spearlet::stream_client_message::Msg::FilterResponse(_) => {
                    return
                }
            };

            let _ = tx
                .send(Ok(StreamServerMessage {
                    msg: Some(
                        spear_next::proto::spearlet::stream_server_message::Msg::RegisterOk(
                            RegisterResponse {
                                protocol_version: 1,
                                accepted: true,
                                message: "ok".to_string(),
                                session_token: "t1".to_string(),
                                token_expire_at_ms: 0,
                            },
                        ),
                    ),
                }))
                .await;

            let candidates = vec![
                Candidate {
                    name: "stub".to_string(),
                    kind: "stub".to_string(),
                    base_url: "".to_string(),
                    model: "".to_string(),
                    weight: 100,
                    priority: 0,
                    ops: vec!["chat_completions".to_string()],
                    features: vec![],
                    transports: vec!["http".to_string()],
                    is_local: true,
                    runtime: Some(CandidateRuntimeHints::default()),
                },
                Candidate {
                    name: "openai".to_string(),
                    kind: "openai_chat_completion".to_string(),
                    base_url: "https://api.openai.com/v1".to_string(),
                    model: "".to_string(),
                    weight: 100,
                    priority: 0,
                    ops: vec!["chat_completions".to_string()],
                    features: vec![],
                    transports: vec!["http".to_string()],
                    is_local: false,
                    runtime: Some(CandidateRuntimeHints::default()),
                },
            ];

            let _ = tx
                .send(Ok(StreamServerMessage {
                    msg: Some(
                        spear_next::proto::spearlet::stream_server_message::Msg::FilterRequest(
                            FilterRequest {
                                correlation_id: "c1".to_string(),
                                request_id: "req-1".to_string(),
                                operation: Operation::ChatCompletions as i32,
                                decision_timeout_ms: 1000,
                                meta: std::collections::HashMap::new(),
                                routing: Some(RoutingHints {
                                    backend: "".to_string(),
                                    allowlist: vec![],
                                    denylist: vec![],
                                    requested_model: "".to_string(),
                                }),
                                requirements: None,
                                signals: None,
                                candidates,
                            },
                        ),
                    ),
                }))
                .await;

            while let Some(msg) = inbound.next().await {
                let Ok(StreamClientMessage { msg: Some(m) }) = msg else {
                    continue;
                };
                match m {
                    spear_next::proto::spearlet::stream_client_message::Msg::FilterResponse(r) => {
                        if let Some(tx) = svc.response_tx.lock().await.take() {
                            let _ = tx.send(r);
                        }
                        break;
                    }
                    spear_next::proto::spearlet::stream_client_message::Msg::Heartbeat(_) => {}
                    spear_next::proto::spearlet::stream_client_message::Msg::Register(
                        RegisterRequest { .. },
                    ) => {}
                }
            }
        });

        Ok(Response::new(
            Box::pin(ReceiverStream::new(rx)) as Self::OpenStream
        ))
    }

    async fn fetch_request_by_id(
        &self,
        request: Request<RequestFetchRequest>,
    ) -> Result<Response<RequestFetchResponse>, Status> {
        let r = request.into_inner();
        if r.session_token != "t1" {
            return Err(Status::permission_denied("bad token"));
        }
        if r.request_id != "req-1" {
            return Err(Status::not_found("bad request_id"));
        }
        Ok(Response::new(RequestFetchResponse {
            request_id: r.request_id,
            content_type: "application/json".to_string(),
            payload: self.request_payload.as_ref().clone(),
        }))
    }
}

#[tokio::test]
async fn protocol_keyword_filter_agent_drops_remote_candidates() {
    let payload = serde_json::to_vec(&serde_json::json!({
        "messages": [{"role": "user", "content": "my secret is 123"}]
    }))
    .unwrap();
    let (resp_tx, resp_rx) = oneshot::channel::<FilterResponse>();
    let svc = TestSvc {
        request_payload: Arc::new(payload),
        response_tx: Arc::new(Mutex::new(Some(resp_tx))),
    };

    let listener = tokio::net::TcpListener::bind("127.0.0.1:0").await.unwrap();
    let addr: SocketAddr = listener.local_addr().unwrap();
    let incoming = tokio_stream::wrappers::TcpListenerStream::new(listener);
    let server = tokio::spawn(async move {
        tonic::transport::Server::builder()
            .add_service(RouterFilterStreamServiceServer::new(svc))
            .serve_with_incoming(incoming)
            .await
            .unwrap();
    });

    let agent = env!("CARGO_BIN_EXE_keyword-filter-agent");
    let mut child = Command::new(agent)
        .arg("--addr")
        .arg(addr.to_string())
        .arg("--agent-id")
        .arg("protocol-agent-1")
        .arg("--max-inflight")
        .arg("8")
        .arg("--max-candidates")
        .arg("8")
        .stdout(Stdio::null())
        .stderr(Stdio::null())
        .spawn()
        .unwrap();

    let resp = tokio::time::timeout(std::time::Duration::from_secs(10), resp_rx)
        .await
        .unwrap()
        .unwrap();

    let mut map: std::collections::HashMap<String, i32> = std::collections::HashMap::new();
    for d in resp.decisions.iter() {
        map.insert(d.name.clone(), d.action);
    }
    assert_eq!(
        map.get("openai").copied(),
        Some(DecisionAction::Drop as i32)
    );
    assert_eq!(map.get("stub").copied(), Some(DecisionAction::Keep as i32));
    assert_eq!(
        resp.debug.get("keyword_filter_hit").map(|s| s.as_str()),
        Some("secret")
    );

    let _ = child.kill().await;
    server.abort();
}
