use anyhow::Result;
use axum::http::header::{CACHE_CONTROL, CONTENT_TYPE};
use axum::http::{HeaderMap, Request, StatusCode};
use axum::middleware;
use axum::middleware::Next;
use axum::response::IntoResponse;
use axum::{
    extract::{Path, Query},
    response::{Html, Json},
    routing::{delete, get, post},
    Router,
};
use futures::StreamExt;
use serde::Deserialize;
use serde_json::json;
use std::convert::Infallible;
use std::net::SocketAddr;
use std::pin::Pin;
use std::time::Duration;
use tokio_stream::wrappers::IntervalStream;
use tokio_util::sync::CancellationToken;
use uuid::Uuid;

use crate::proto::sms::{
    backend_registry_service_client::BackendRegistryServiceClient,
    mcp_registry_service_client::McpRegistryServiceClient, node_service_client::NodeServiceClient,
    placement_service_client::PlacementServiceClient, ListNodesRequest,
};
use crate::sms::gateway::GatewayState;
use crate::sms::handlers::{
    delete_file, download_file, get_file_meta, list_files, presign_upload, upload_file,
};

use crate::proto::spearlet::{
    invocation_service_client::InvocationServiceClient, ExecutionMode, ExecutionStatus,
    InvokeRequest, Payload,
};
use crate::spearlet::execution::DEFAULT_ENTRY_FUNCTION_NAME;

pub struct WebAdminServer {
    addr: SocketAddr,
    grpc_addr: SocketAddr,
    enabled: bool,
}

impl WebAdminServer {
    pub fn new(addr: SocketAddr, grpc_addr: SocketAddr, enabled: bool) -> Self {
        Self {
            addr,
            grpc_addr,
            enabled,
        }
    }

    pub async fn start_with_shutdown<F>(self, shutdown: F) -> Result<()>
    where
        F: std::future::Future<Output = ()> + Send + 'static,
    {
        if !self.enabled {
            return Ok(());
        }
        let cancel_token = CancellationToken::new();
        let (listener, app) = self.prepare_with_token(cancel_token.clone()).await?;
        let shutdown_and_cancel = async move {
            shutdown.await;
            cancel_token.cancel();
        };
        axum::serve(listener, app)
            .with_graceful_shutdown(shutdown_and_cancel)
            .await?;
        Ok(())
    }

    async fn prepare_with_token(
        &self,
        cancel_token: CancellationToken,
    ) -> Result<(tokio::net::TcpListener, Router)> {
        let grpc_url = format!("http://{}", self.grpc_addr);
        let channel = tonic::transport::Channel::from_shared(grpc_url)
            .expect("Invalid gRPC URL")
            .connect_lazy();
        let node_client = NodeServiceClient::new(channel.clone());
        let task_client =
            crate::proto::sms::task_service_client::TaskServiceClient::new(channel.clone());
        let placement_client = PlacementServiceClient::new(channel.clone());
        let mcp_registry_client = McpRegistryServiceClient::new(channel.clone());
        let backend_registry_client = BackendRegistryServiceClient::new(channel);
        let state = GatewayState {
            node_client,
            task_client,
            placement_client,
            mcp_registry_client,
            backend_registry_client,
            cancel_token: cancel_token.clone(),
            max_upload_bytes: 64 * 1024 * 1024,
        };
        let mut app = create_admin_router(state);
        if let Ok(token) = std::env::var("SMS_WEB_ADMIN_TOKEN") {
            let bearer = format!("Bearer {}", token);
            app = app.layer(middleware::from_fn(
                move |req: Request<axum::body::Body>, next: Next| {
                    let bearer = bearer.clone();
                    async move {
                        let authorized = req
                            .headers()
                            .get(axum::http::header::AUTHORIZATION)
                            .and_then(|h| h.to_str().ok())
                            .map(|v| v == bearer)
                            .unwrap_or(false);
                        if !authorized {
                            return StatusCode::UNAUTHORIZED.into_response();
                        }
                        next.run(req).await
                    }
                },
            ));
        }
        let listener = tokio::net::TcpListener::bind(self.addr).await?;
        Ok((listener, app))
    }
}

#[derive(Deserialize)]
struct ListQuery {
    status: Option<String>,
    limit: Option<usize>,
    offset: Option<usize>,
    sort: Option<String>,
    sort_by: Option<String>,
    order: Option<String>,
    q: Option<String>,
}

pub fn create_admin_router(state: GatewayState) -> Router {
    Router::new()
        .route("/", get(admin_index))
        .route("/admin", get(admin_index))
        .route("/admin/", get(admin_index))
        .route("/admin/static/{*path}", get(admin_static))
        .route(
            "/admin/api/nodes",
            get({
                let state = state.clone();
                move |q: Query<ListQuery>| list_nodes(state.clone(), q)
            }),
        )
        .route(
            "/admin/api/nodes/{uuid}",
            get({
                let state = state.clone();
                move |p: Path<String>| get_node_detail(state.clone(), p)
            }),
        )
        .route(
            "/admin/api/nodes/stream",
            get({
                let state = state.clone();
                move |q: Query<StreamQuery>| nodes_stream(state.clone(), q)
            }),
        )
        .route(
            "/admin/api/stats",
            get({
                let state = state.clone();
                move || get_stats(state.clone())
            }),
        )
        .route(
            "/admin/api/backends",
            get({
                let state = state.clone();
                move |q: Query<ListQuery>| list_backends(state.clone(), q)
            }),
        )
        .route(
            "/admin/api/nodes/{uuid}/backends",
            get({
                let state = state.clone();
                move |p: Path<String>| get_node_backends(state.clone(), p)
            }),
        )
        .route(
            "/admin/api/mcp/servers",
            get({
                let state = state.clone();
                move || list_mcp_servers(state.clone())
            }),
        )
        .route(
            "/admin/api/mcp/servers",
            post({
                let state = state.clone();
                move |body: Json<McpServerUpsertBody>| upsert_mcp_server(state.clone(), body)
            }),
        )
        .route(
            "/admin/api/mcp/servers/{server_id}",
            delete({
                let state = state.clone();
                move |p: Path<String>| delete_mcp_server(state.clone(), p)
            }),
        )
        .route(
            "/admin/api/tasks",
            get({
                let state = state.clone();
                move |q: Query<ListQuery>| list_tasks(state.clone(), q)
            }),
        )
        .route(
            "/admin/api/tasks",
            post({
                let state = state.clone();
                move |payload: axum::extract::Json<CreateTaskBody>| {
                    create_task(state.clone(), payload)
                }
            }),
        )
        .route(
            "/admin/api/tasks/{task_id}",
            get({
                let state = state.clone();
                move |p: Path<String>| get_task_detail(state.clone(), p)
            }),
        )
        .route(
            "/admin/api/invocations",
            post({
                let state = state.clone();
                move |payload: axum::extract::Json<CreateExecutionBody>| {
                    create_invocation(state.clone(), payload)
                }
            }),
        )
        .route(
            "/admin/api/executions",
            post({
                let state = state.clone();
                move |payload: axum::extract::Json<CreateExecutionBody>| {
                    create_invocation(state.clone(), payload)
                }
            }),
        )
        // Embedded file storage API / 内嵌文件存储API
        .route("/admin/api/files", get(list_files))
        .route("/admin/api/files/presign-upload", post(presign_upload))
        .route(
            "/admin/api/files",
            post({
                let state = state.clone();
                move |req: axum::http::Request<axum::body::Body>| {
                    upload_file(axum::extract::State(state.clone()), req)
                }
            }),
        )
        .route("/admin/api/files/{id}", get(download_file))
        .route("/admin/api/files/{id}", delete(delete_file))
        .route("/admin/api/files/{id}/meta", get(get_file_meta))
}

#[derive(Deserialize)]
struct McpServerUpsertBody {
    server_id: String,
    display_name: Option<String>,
    transport: String,
    stdio: Option<McpStdioBody>,
    http: Option<McpHttpBody>,
    tool_namespace: Option<String>,
    allowed_tools: Option<Vec<String>>,
    budgets: Option<McpBudgetsBody>,
    approval_policy: Option<McpApprovalPolicyBody>,
}

#[derive(Deserialize)]
struct McpStdioBody {
    command: String,
    args: Option<Vec<String>>,
    env: Option<std::collections::HashMap<String, String>>,
    cwd: Option<String>,
}

#[derive(Deserialize)]
struct McpHttpBody {
    url: String,
    headers: Option<std::collections::HashMap<String, String>>,
    auth_ref: Option<String>,
}

#[derive(Deserialize)]
struct McpBudgetsBody {
    tool_timeout_ms: Option<u64>,
    max_concurrency: Option<u64>,
    max_tool_output_bytes: Option<u64>,
}

#[derive(Deserialize)]
struct McpApprovalPolicyBody {
    default_policy: Option<String>,
    per_tool: Option<std::collections::HashMap<String, String>>,
}

async fn list_backends(state: GatewayState, Query(q): Query<ListQuery>) -> Json<serde_json::Value> {
    use crate::proto::sms::{BackendStatus, ListNodeBackendSnapshotsRequest};

    #[derive(Default)]
    struct Agg {
        name: String,
        kind: String,
        operations: std::collections::BTreeSet<String>,
        features: std::collections::BTreeSet<String>,
        transports: std::collections::BTreeSet<String>,
        available_nodes: i64,
        total_nodes: i64,
        nodes: Vec<serde_json::Value>,
    }

    let mut client = state.backend_registry_client.clone();
    let limit = q.limit.unwrap_or(500) as u32;
    let offset = q.offset.unwrap_or(0) as u32;
    let resp = match client
        .list_node_backend_snapshots(ListNodeBackendSnapshotsRequest { limit, offset })
        .await
    {
        Ok(r) => r.into_inner(),
        Err(e) => return Json(json!({"success": false, "message": e.to_string()})),
    };

    let status_filter = q.status.as_deref().map(|s| s.to_ascii_lowercase());
    let needle = q.q.as_deref().map(|s| s.to_ascii_lowercase());

    let mut agg: std::collections::HashMap<(String, String), Agg> =
        std::collections::HashMap::new();

    for snap in resp.snapshots.into_iter() {
        let node_uuid = snap.node_uuid.clone();
        for b in snap.backends.into_iter() {
            let status = if b.status == BackendStatus::Available as i32 {
                "available"
            } else {
                "unavailable"
            };
            if let Some(f) = status_filter.as_ref() {
                if f != status {
                    continue;
                }
            }
            if let Some(n) = needle.as_ref() {
                let hay = format!("{} {}", b.name, b.kind).to_ascii_lowercase();
                if !hay.contains(n) {
                    continue;
                }
            }

            let key = (b.name.clone(), b.kind.clone());
            let entry = agg.entry(key).or_insert_with(|| Agg {
                name: b.name.clone(),
                kind: b.kind.clone(),
                ..Default::default()
            });

            for op in b.operations.iter() {
                if !op.trim().is_empty() {
                    entry.operations.insert(op.clone());
                }
            }
            for f in b.features.iter() {
                if !f.trim().is_empty() {
                    entry.features.insert(f.clone());
                }
            }
            for t in b.transports.iter() {
                if !t.trim().is_empty() {
                    entry.transports.insert(t.clone());
                }
            }

            entry.nodes.push(json!({
                "node_uuid": node_uuid,
                "status": status,
                "status_reason": b.status_reason,
                "weight": b.weight,
                "priority": b.priority,
                "base_url": b.base_url,
            }));
            entry.total_nodes += 1;
            if status == "available" {
                entry.available_nodes += 1;
            }
        }
    }

    let mut list = agg
        .into_values()
        .map(|a| {
            json!({
                "name": a.name,
                "kind": a.kind,
                "operations": a.operations.into_iter().collect::<Vec<_>>(),
                "features": a.features.into_iter().collect::<Vec<_>>(),
                "transports": a.transports.into_iter().collect::<Vec<_>>(),
                "available_nodes": a.available_nodes,
                "total_nodes": a.total_nodes,
                "nodes": a.nodes,
            })
        })
        .collect::<Vec<_>>();

    list.sort_by(|a, b| {
        let av = a
            .get("available_nodes")
            .and_then(|v| v.as_i64())
            .unwrap_or(0);
        let bv = b
            .get("available_nodes")
            .and_then(|v| v.as_i64())
            .unwrap_or(0);
        let an = a.get("name").and_then(|v| v.as_str()).unwrap_or("");
        let bn = b.get("name").and_then(|v| v.as_str()).unwrap_or("");
        bv.cmp(&av).then_with(|| an.cmp(bn))
    });

    Json(json!({"success": true, "backends": list, "total_count": resp.total_count}))
}

async fn get_node_backends(state: GatewayState, p: Path<String>) -> Json<serde_json::Value> {
    use crate::proto::sms::GetNodeBackendsRequest;

    let node_uuid = p.0;
    if node_uuid.trim().is_empty() {
        return Json(json!({"found": false}));
    }
    let mut client = state.backend_registry_client.clone();
    match client
        .get_node_backends(GetNodeBackendsRequest {
            node_uuid: node_uuid.clone(),
        })
        .await
    {
        Ok(resp) => {
            let inner = resp.into_inner();
            let meta = inner.snapshot.as_ref().map(|s| {
                json!({
                    "revision": s.revision,
                    "reported_at_ms": s.reported_at_ms,
                })
            });
            let backends = inner
                .snapshot
                .as_ref()
                .map(|s| {
                    s.backends
                        .iter()
                        .map(|b| {
                            json!({
                                "name": b.name,
                                "kind": b.kind,
                                "operations": b.operations,
                                "features": b.features,
                                "transports": b.transports,
                                "weight": b.weight,
                                "priority": b.priority,
                                "base_url": b.base_url,
                                "status": b.status,
                                "status_reason": b.status_reason,
                            })
                        })
                        .collect::<Vec<_>>()
                })
                .unwrap_or_default();
            Json(json!({
                "found": inner.found,
                "node_uuid": node_uuid,
                "backends": backends,
                "snapshot": meta,
            }))
        }
        Err(e) => Json(json!({"found": false, "message": e.to_string()})),
    }
}

async fn list_mcp_servers(state: GatewayState) -> Json<serde_json::Value> {
    let mut client = state.mcp_registry_client.clone();
    match client
        .list_mcp_servers(crate::proto::sms::ListMcpServersRequest { since_revision: 0 })
        .await
    {
        Ok(resp) => {
            let inner = resp.into_inner();
            let servers = inner
                .servers
                .into_iter()
                .map(|s| {
                    let stdio = s.stdio.as_ref().map(|x| {
                        json!({
                            "command": x.command,
                            "args": x.args,
                            "env": x.env,
                            "cwd": x.cwd,
                        })
                    });
                    let http = s.http.as_ref().map(|x| {
                        json!({
                            "url": x.url,
                            "headers": x.headers,
                            "auth_ref": x.auth_ref,
                        })
                    });
                    let approval_policy = s.approval_policy.as_ref().map(|x| {
                        json!({
                            "default_policy": x.default_policy,
                            "per_tool": x.per_tool,
                        })
                    });
                    let budgets = s.budgets.as_ref().map(|x| {
                        json!({
                            "tool_timeout_ms": x.tool_timeout_ms,
                            "max_concurrency": x.max_concurrency,
                            "max_tool_output_bytes": x.max_tool_output_bytes,
                        })
                    });
                    json!({
                        "server_id": s.server_id,
                        "display_name": s.display_name,
                        "transport": s.transport,
                        "stdio": stdio,
                        "http": http,
                        "tool_namespace": s.tool_namespace,
                        "allowed_tools": s.allowed_tools,
                        "approval_policy": approval_policy,
                        "budgets": budgets,
                        "updated_at_ms": s.updated_at_ms,
                    })
                })
                .collect::<Vec<_>>();
            Json(json!({"success": true, "revision": inner.revision, "servers": servers}))
        }
        Err(e) => Json(json!({"success": false, "message": e.to_string()})),
    }
}

async fn upsert_mcp_server(
    state: GatewayState,
    body: Json<McpServerUpsertBody>,
) -> Json<serde_json::Value> {
    use crate::proto::sms::{
        McpApprovalPolicy, McpBudgets, McpHttpConfig, McpServerRecord, McpStdioConfig, McpTransport,
    };

    let body = body.0;
    if body.server_id.trim().is_empty() {
        return Json(json!({"success": false, "message": "server_id is required"}));
    }

    let transport = match body.transport.as_str() {
        "stdio" => McpTransport::Stdio as i32,
        "streamable_http" => McpTransport::StreamableHttp as i32,
        _ => {
            return Json(json!({"success": false, "message": "invalid transport"}));
        }
    };

    let stdio = body.stdio.map(|s| McpStdioConfig {
        command: s.command,
        args: s.args.unwrap_or_default(),
        env: s.env.unwrap_or_default(),
        cwd: s.cwd.unwrap_or_default(),
    });
    let http = body.http.map(|h| McpHttpConfig {
        url: h.url,
        headers: h.headers.unwrap_or_default(),
        auth_ref: h.auth_ref.unwrap_or_default(),
    });

    let budgets = body.budgets.map(|b| McpBudgets {
        tool_timeout_ms: b.tool_timeout_ms.unwrap_or(0),
        max_concurrency: b.max_concurrency.unwrap_or(0),
        max_tool_output_bytes: b.max_tool_output_bytes.unwrap_or(0),
    });

    let approval_policy = body.approval_policy.map(|p| McpApprovalPolicy {
        default_policy: p.default_policy.unwrap_or_default(),
        per_tool: p.per_tool.unwrap_or_default(),
    });

    let record = McpServerRecord {
        server_id: body.server_id.trim().to_string(),
        display_name: body.display_name.unwrap_or_default(),
        transport,
        stdio,
        http,
        tool_namespace: body.tool_namespace.unwrap_or_default(),
        allowed_tools: body.allowed_tools.unwrap_or_default(),
        approval_policy,
        budgets,
        updated_at_ms: 0,
    };

    let mut client = state.mcp_registry_client.clone();
    match client
        .upsert_mcp_server(crate::proto::sms::UpsertMcpServerRequest {
            record: Some(record),
        })
        .await
    {
        Ok(resp) => Json(json!({"success": true, "revision": resp.into_inner().revision})),
        Err(e) => Json(json!({"success": false, "message": e.to_string()})),
    }
}

async fn delete_mcp_server(state: GatewayState, p: Path<String>) -> Json<serde_json::Value> {
    let server_id = p.0;
    if server_id.trim().is_empty() {
        return Json(json!({"success": false, "message": "server_id is required"}));
    }
    let mut client = state.mcp_registry_client.clone();
    match client
        .delete_mcp_server(crate::proto::sms::DeleteMcpServerRequest {
            server_id: server_id.trim().to_string(),
        })
        .await
    {
        Ok(resp) => Json(json!({"success": true, "revision": resp.into_inner().revision})),
        Err(e) => Json(json!({"success": false, "message": e.to_string()})),
    }
}

#[derive(serde::Deserialize)]
struct CreateExecutionBody {
    task_id: String,
    node_uuid: Option<String>,
    request_id: Option<String>,
    execution_id: Option<String>,
    execution_mode: Option<String>,
    max_candidates: Option<u32>,
}

fn parse_execution_mode(v: Option<&str>) -> i32 {
    match v.map(|s| s.to_ascii_lowercase()) {
        Some(s) if s == "async" => ExecutionMode::Async as i32,
        Some(s) if s == "stream" => ExecutionMode::Stream as i32,
        Some(s) if s == "console" => ExecutionMode::Console as i32,
        _ => ExecutionMode::Sync as i32,
    }
}

async fn create_invocation(
    state: GatewayState,
    axum::extract::Json(body): axum::extract::Json<CreateExecutionBody>,
) -> Json<serde_json::Value> {
    // Admin BFF: one-shot execution submission with two-level scheduling.
    // Admin BFF：一次性提交执行请求，走“两层调度”（SMS placement → Spearlet execute）。
    if body.task_id.is_empty() {
        return Json(json!({ "success": false, "message": "task_id is required" }));
    }
    let request_id = body
        .request_id
        .filter(|s| !s.is_empty())
        .unwrap_or_else(|| Uuid::new_v4().to_string());
    let execution_id = body
        .execution_id
        .filter(|s| !s.is_empty())
        .unwrap_or_else(|| Uuid::new_v4().to_string());
    let mode = parse_execution_mode(body.execution_mode.as_deref());
    let max_candidates = body.max_candidates.unwrap_or(3);

    if let Some(node_uuid) = body.node_uuid.as_ref().filter(|s| !s.is_empty()) {
        use crate::proto::sms::GetNodeRequest;
        let mut node_client = state.node_client.clone();
        let node_resp = node_client
            .get_node(GetNodeRequest {
                uuid: node_uuid.clone(),
            })
            .await;
        let node_resp = match node_resp {
            Ok(r) => r.into_inner(),
            Err(e) => return Json(json!({ "success": false, "message": e.to_string() })),
        };
        if !node_resp.found {
            return Json(json!({ "success": false, "message": "node not found" }));
        }
        let Some(node) = node_resp.node else {
            return Json(json!({ "success": false, "message": "node not found" }));
        };

        let url = format!("http://{}:{}", node.ip_address, node.port);
        let channel = tonic::transport::Channel::from_shared(url.clone())
            .ok()
            .map(|ch| ch.connect_lazy());
        let Some(channel) = channel else {
            return Json(json!({ "success": false, "message": "invalid node url" }));
        };
        let mut invc = InvocationServiceClient::new(channel);
        let req = InvokeRequest {
            invocation_id: request_id.clone(),
            execution_id: execution_id.clone(),
            task_id: body.task_id.clone(),
            function_name: DEFAULT_ENTRY_FUNCTION_NAME.to_string(),
            input: Some(Payload {
                content_type: "application/octet-stream".to_string(),
                data: Vec::new(),
            }),
            headers: Default::default(),
            environment: Default::default(),
            timeout_ms: 0,
            session_id: String::new(),
            mode,
            force_new_instance: false,
            metadata: Default::default(),
        };
        return match invc.invoke(req).await {
            Ok(resp) => {
                let inner = resp.into_inner();
                let success = inner.status == ExecutionStatus::Completed as i32;
                let message = inner
                    .error
                    .as_ref()
                    .map(|e| e.message.clone())
                    .unwrap_or_else(|| "ok".to_string());
                Json(json!({
                    "success": success,
                    "node_uuid": node.uuid,
                    "invocation_id": inner.invocation_id,
                    "execution_id": inner.execution_id,
                    "message": message,
                }))
            }
            Err(e) => Json(json!({ "success": false, "message": e.to_string() })),
        };
    }

    let mut placement = state.placement_client.clone();
    // Step 1: ask SMS to return an ordered list of candidate nodes.
    // 第一步：调用 SMS placement，拿到有序候选节点列表。
    let placement_resp = placement
        .place_invocation(crate::proto::sms::PlaceInvocationRequest {
            request_id: request_id.clone(),
            task_id: body.task_id.clone(),
            max_candidates,
            labels: std::collections::HashMap::new(),
        })
        .await;
    let placement_resp = match placement_resp {
        Ok(r) => r.into_inner(),
        Err(e) => {
            return Json(json!({ "success": false, "message": e.to_string() }));
        }
    };
    if placement_resp.candidates.is_empty() {
        return Json(json!({ "success": false, "message": "no candidates" }));
    }

    for c in placement_resp.candidates.iter() {
        // Step 2: try candidates in order (spillback).
        // 第二步：按顺序尝试候选节点（spillback）。
        let url = format!("http://{}:{}", c.ip_address, c.port);
        let channel = tonic::transport::Channel::from_shared(url.clone())
            .ok()
            .map(|ch| ch.connect_lazy());
        let Some(channel) = channel else {
            // Node address is invalid: treat as unavailable and spillback.
            // 节点地址不可用：按 unavailable 处理并继续 spillback。
            let _ = placement
                .report_invocation_outcome(crate::proto::sms::ReportInvocationOutcomeRequest {
                    decision_id: placement_resp.decision_id.clone(),
                    request_id: request_id.clone(),
                    task_id: body.task_id.clone(),
                    node_uuid: c.node_uuid.clone(),
                    outcome_class: crate::proto::sms::InvocationOutcomeClass::Unavailable as i32,
                    error_message: "invalid node url".to_string(),
                })
                .await;
            continue;
        };
        let mut invc = InvocationServiceClient::new(channel);
        // Use ExistingTask invocation; Spearlet may fetch task from SMS when missing.
        // 使用 ExistingTask 调用；若节点本地缺 task，Spearlet 会从 SMS 拉取补齐后执行。
        let req = InvokeRequest {
            invocation_id: request_id.clone(),
            execution_id: execution_id.clone(),
            task_id: body.task_id.clone(),
            function_name: DEFAULT_ENTRY_FUNCTION_NAME.to_string(),
            input: Some(Payload {
                content_type: "application/octet-stream".to_string(),
                data: Vec::new(),
            }),
            headers: Default::default(),
            environment: Default::default(),
            timeout_ms: 0,
            session_id: String::new(),
            mode,
            force_new_instance: false,
            metadata: Default::default(),
        };
        match invc.invoke(req).await {
            Ok(resp) => {
                let inner = resp.into_inner();
                let success = inner.status == ExecutionStatus::Completed as i32;
                let message = inner
                    .error
                    .as_ref()
                    .map(|e| e.message.clone())
                    .unwrap_or_else(|| "ok".to_string());
                if success {
                    // Success: report feedback to SMS and return.
                    // 成功：回报给 SMS 用于后续 placement，然后直接返回。
                    let _ = placement
                        .report_invocation_outcome(
                            crate::proto::sms::ReportInvocationOutcomeRequest {
                                decision_id: placement_resp.decision_id.clone(),
                                request_id: request_id.clone(),
                                task_id: body.task_id.clone(),
                                node_uuid: c.node_uuid.clone(),
                                outcome_class: crate::proto::sms::InvocationOutcomeClass::Success
                                    as i32,
                                error_message: String::new(),
                            },
                        )
                        .await;
                    return Json(json!({
                        "success": true,
                        "decision_id": placement_resp.decision_id,
                        "node_uuid": c.node_uuid,
                        "invocation_id": inner.invocation_id,
                        "execution_id": inner.execution_id,
                        "message": message,
                    }));
                }
                // Function-level failure is not retryable here: return immediately.
                // Function 级失败在这里不做重试：直接返回给前端。
                let _ = placement
                    .report_invocation_outcome(crate::proto::sms::ReportInvocationOutcomeRequest {
                        decision_id: placement_resp.decision_id.clone(),
                        request_id: request_id.clone(),
                        task_id: body.task_id.clone(),
                        node_uuid: c.node_uuid.clone(),
                        outcome_class: crate::proto::sms::InvocationOutcomeClass::Internal as i32,
                        error_message: message.clone(),
                    })
                    .await;
                return Json(json!({ "success": false, "message": message }));
            }
            Err(e) => {
                // gRPC error classification decides whether to spillback.
                // gRPC 错误分类用于决定是否继续 spillback。
                let class = match e.code() {
                    tonic::Code::DeadlineExceeded => {
                        crate::proto::sms::InvocationOutcomeClass::Timeout as i32
                    }
                    tonic::Code::Unavailable => {
                        crate::proto::sms::InvocationOutcomeClass::Unavailable as i32
                    }
                    tonic::Code::ResourceExhausted => {
                        crate::proto::sms::InvocationOutcomeClass::Overloaded as i32
                    }
                    tonic::Code::InvalidArgument => {
                        crate::proto::sms::InvocationOutcomeClass::BadRequest as i32
                    }
                    tonic::Code::Unauthenticated | tonic::Code::PermissionDenied => {
                        crate::proto::sms::InvocationOutcomeClass::Rejected as i32
                    }
                    _ => crate::proto::sms::InvocationOutcomeClass::Internal as i32,
                };
                let _ = placement
                    .report_invocation_outcome(crate::proto::sms::ReportInvocationOutcomeRequest {
                        decision_id: placement_resp.decision_id.clone(),
                        request_id: request_id.clone(),
                        task_id: body.task_id.clone(),
                        node_uuid: c.node_uuid.clone(),
                        outcome_class: class,
                        error_message: e.to_string(),
                    })
                    .await;
                if class == crate::proto::sms::InvocationOutcomeClass::BadRequest as i32
                    || class == crate::proto::sms::InvocationOutcomeClass::Rejected as i32
                {
                    // Non-retryable: stop spillback.
                    // 不可重试：终止 spillback。
                    return Json(json!({ "success": false, "message": e.to_string() }));
                }
                continue;
            }
        }
    }
    // All candidates exhausted.
    // 所有候选均失败。
    Json(json!({ "success": false, "message": "all candidates failed" }))
}

async fn list_nodes(state: GatewayState, Query(q): Query<ListQuery>) -> Json<serde_json::Value> {
    let mut client = state.node_client.clone();
    let req = ListNodesRequest {
        status_filter: q.status.unwrap_or_default(),
    };
    let resp = client.list_nodes(req).await.unwrap().into_inner();
    let total = resp.nodes.len();
    let mut list = resp
        .nodes
        .into_iter()
        .map(|n| {
            let name = n.metadata.get("name").cloned().unwrap_or_default();
            json!({
                "uuid": n.uuid,
                "name": name,
                "ip_address": n.ip_address,
                "port": n.port,
                "status": n.status,
                "last_heartbeat": n.last_heartbeat,
                "registered_at": n.registered_at,
                "metadata": n.metadata,
            })
        })
        .collect::<Vec<_>>();
    if let Some(q) = q.q.as_ref().map(|s| s.to_lowercase()) {
        list.retain(|item| {
            let uuid = item["uuid"].as_str().unwrap_or("").to_lowercase();
            let ip = item["ip_address"].as_str().unwrap_or("").to_lowercase();
            let meta = item["metadata"].as_object();
            let meta_hit = meta
                .map(|m| {
                    m.iter().any(|(k, v)| {
                        k.to_lowercase().contains(&q)
                            || v.as_str().unwrap_or("").to_lowercase().contains(&q)
                    })
                })
                .unwrap_or(false);
            uuid.contains(&q) || ip.contains(&q) || meta_hit
        });
    }
    let (field, asc) = if let Some(sort) = &q.sort {
        let mut parts = sort.split(':');
        let field = parts.next().unwrap_or("").to_string();
        let order = parts.next().unwrap_or("asc");
        let asc = order != "desc";
        (field, asc)
    } else if let Some(field) = &q.sort_by {
        let asc = q.order.as_deref().unwrap_or("asc") != "desc";
        (field.clone(), asc)
    } else {
        (String::new(), true)
    };
    if !field.is_empty() {
        match field.as_str() {
            "last_heartbeat" => list.sort_by_key(|i| i["last_heartbeat"].as_i64().unwrap_or(0)),
            "registered_at" => list.sort_by_key(|i| i["registered_at"].as_i64().unwrap_or(0)),
            _ => {}
        }
        if !asc {
            list.reverse();
        }
    }
    if let Some(offset) = q.offset {
        if offset < list.len() {
            list = list.split_off(offset);
        } else {
            list.clear();
        }
    }
    if let Some(limit) = q.limit {
        if limit < list.len() {
            list.truncate(limit);
        }
    }
    Json(json!({ "nodes": list, "total_count": total }))
}

async fn list_tasks(state: GatewayState, Query(q): Query<ListQuery>) -> Json<serde_json::Value> {
    use crate::proto::sms::ListTasksRequest;
    let mut client = state.task_client.clone();
    let resp = client
        .list_tasks(ListTasksRequest {
            node_uuid: String::new(),
            status_filter: -1,
            priority_filter: -1,
            limit: 0,
            offset: 0,
        })
        .await
        .unwrap()
        .into_inner();
    let total = resp.total_count as usize;
    let mut list = resp
        .tasks
        .into_iter()
        .map(|t| {
            let status = match t.status {
                1 => "registered",
                2 => "active",
                3 => "inactive",
                4 => "unregistered",
                _ => "unknown",
            };
            let priority = match t.priority {
                1 => "low",
                2 => "normal",
                3 => "high",
                4 => "urgent",
                _ => "unknown",
            };
            let (exec_type, exec_uri, exec_name) = if let Some(exec) = t.executable {
                let et = match exec.r#type {
                    1 => "binary",
                    2 => "script",
                    3 => "container",
                    4 => "wasm",
                    5 => "process",
                    _ => "unknown",
                };
                (et.to_string(), exec.uri, exec.name)
            } else {
                (String::new(), String::new(), String::new())
            };
            let execution_kind = match t.execution_kind {
                x if x == crate::proto::sms::TaskExecutionKind::LongRunning as i32 => {
                    "long_running".to_string()
                }
                x if x == crate::proto::sms::TaskExecutionKind::ShortRunning as i32 => {
                    "short_running".to_string()
                }
                _ => "short_running".to_string(),
            };
            json!({
                "task_id": t.task_id,
                "name": t.name,
                "description": t.description,
                "status": status,
                "priority": priority,
                "node_uuid": t.node_uuid,
                "endpoint": t.endpoint,
                "version": t.version,
                "capabilities": t.capabilities,
                "registered_at": t.registered_at,
                "last_heartbeat": t.last_heartbeat,
                "metadata": t.metadata,
                "config": t.config,
                "execution_kind": execution_kind,
                "executable_type": exec_type,
                "executable_uri": exec_uri,
                "executable_name": exec_name,
                "result_uris": t.result_uris,
                "last_result_uri": t.last_result_uri,
                "last_result_status": t.last_result_status,
                "last_completed_at": t.last_completed_at,
                "last_result_metadata": t.last_result_metadata,
            })
        })
        .collect::<Vec<_>>();
    if let Some(qs) = q.q.as_ref().map(|s| s.to_lowercase()) {
        list.retain(|item| {
            let id = item["task_id"].as_str().unwrap_or("").to_lowercase();
            let name = item["name"].as_str().unwrap_or("").to_lowercase();
            let node = item["node_uuid"].as_str().unwrap_or("").to_lowercase();
            let endpoint = item["endpoint"].as_str().unwrap_or("").to_lowercase();
            let meta = item["metadata"].as_object();
            let meta_hit = meta
                .map(|m| {
                    m.iter().any(|(k, v)| {
                        k.to_lowercase().contains(&qs)
                            || v.as_str().unwrap_or("").to_lowercase().contains(&qs)
                    })
                })
                .unwrap_or(false);
            id.contains(&qs)
                || name.contains(&qs)
                || node.contains(&qs)
                || endpoint.contains(&qs)
                || meta_hit
        });
    }
    let (field, asc) = if let Some(sort) = &q.sort {
        let mut parts = sort.split(':');
        let field = parts.next().unwrap_or("").to_string();
        let order = parts.next().unwrap_or("asc");
        let asc = order != "desc";
        (field, asc)
    } else if let Some(field) = &q.sort_by {
        let asc = q.order.as_deref().unwrap_or("asc") != "desc";
        (field.clone(), asc)
    } else {
        (String::new(), true)
    };
    if !field.is_empty() {
        match field.as_str() {
            "last_heartbeat" => list.sort_by_key(|i| i["last_heartbeat"].as_i64().unwrap_or(0)),
            "registered_at" => list.sort_by_key(|i| i["registered_at"].as_i64().unwrap_or(0)),
            _ => {}
        }
        if !asc {
            list.reverse();
        }
    }
    if let Some(offset) = q.offset {
        if offset < list.len() {
            list = list.split_off(offset);
        } else {
            list.clear();
        }
    }
    if let Some(limit) = q.limit {
        if limit < list.len() {
            list.truncate(limit);
        }
    }
    Json(json!({ "tasks": list, "total_count": total }))
}

#[derive(serde::Deserialize)]
struct CreateExecutableBody {
    r#type: Option<String>,
    uri: Option<String>,
    name: Option<String>,
    checksum_sha256: Option<String>,
    args: Option<Vec<String>>,
    env: Option<std::collections::HashMap<String, String>>,
}

#[derive(serde::Deserialize)]
struct CreateTaskBody {
    name: String,
    description: Option<String>,
    priority: Option<String>,
    node_uuid: String,
    endpoint: String,
    version: String,
    capabilities: Option<Vec<String>>,
    metadata: Option<std::collections::HashMap<String, String>>,
    config: Option<std::collections::HashMap<String, String>>,
    executable: Option<CreateExecutableBody>,
}

async fn create_task(
    state: GatewayState,
    axum::extract::Json(body): axum::extract::Json<CreateTaskBody>,
) -> Json<serde_json::Value> {
    use crate::proto::sms::{ExecutableType, RegisterTaskRequest, TaskExecutable, TaskPriority};
    let p = match body.priority.as_ref().map(|s| s.to_ascii_lowercase()) {
        Some(s) if s == "low" => TaskPriority::Low as i32,
        Some(s) if s == "high" => TaskPriority::High as i32,
        Some(s) if s == "urgent" => TaskPriority::Urgent as i32,
        Some(s) if s == "unknown" => TaskPriority::Unknown as i32,
        _ => TaskPriority::Normal as i32,
    };
    let exe = body.executable.as_ref().map(|e| {
        let t = match e.r#type.as_ref().map(|s| s.to_ascii_lowercase()) {
            Some(s) if s == "binary" => ExecutableType::Binary as i32,
            Some(s) if s == "script" => ExecutableType::Script as i32,
            Some(s) if s == "container" => ExecutableType::Container as i32,
            Some(s) if s == "wasm" => ExecutableType::Wasm as i32,
            Some(s) if s == "process" => ExecutableType::Process as i32,
            _ => ExecutableType::Unknown as i32,
        };
        TaskExecutable {
            r#type: t,
            uri: e.uri.clone().unwrap_or_default(),
            name: e.name.clone().unwrap_or_default(),
            checksum_sha256: e.checksum_sha256.clone().unwrap_or_default(),
            args: e.args.clone().unwrap_or_default(),
            env: e.env.clone().unwrap_or_default(),
        }
    });
    let meta = body.metadata.clone().unwrap_or_default();
    let req = RegisterTaskRequest {
        name: body.name,
        description: body.description.unwrap_or_default(),
        priority: p,
        node_uuid: body.node_uuid,
        endpoint: body.endpoint,
        version: body.version,
        capabilities: body.capabilities.unwrap_or_default(),
        metadata: meta.clone(),
        config: body.config.unwrap_or_default(),
        executable: exe,
        execution_kind: {
            let ek = meta
                .get("execution_kind")
                .cloned()
                .unwrap_or_else(|| "short_running".to_string());
            if ek.to_lowercase() == "long_running" {
                crate::proto::sms::TaskExecutionKind::LongRunning as i32
            } else {
                crate::proto::sms::TaskExecutionKind::ShortRunning as i32
            }
        },
    };
    let mut client = state.task_client.clone();
    let resp = client.register_task(tonic::Request::new(req)).await;
    match resp {
        Ok(r) => {
            let inner = r.into_inner();
            Json(
                json!({ "success": inner.success, "task_id": inner.task_id, "message": inner.message }),
            )
        }
        Err(e) => Json(json!({ "success": false, "message": e.to_string() })),
    }
}

async fn get_task_detail(
    state: GatewayState,
    Path(task_id): Path<String>,
) -> Json<serde_json::Value> {
    use crate::proto::sms::GetTaskRequest;
    let mut client = state.task_client.clone();
    let resp = client.get_task(GetTaskRequest { task_id }).await;
    match resp {
        Ok(r) => {
            let inner = r.into_inner();
            if let Some(t) = inner.task {
                let status = match t.status {
                    1 => "registered",
                    2 => "active",
                    3 => "inactive",
                    4 => "unregistered",
                    _ => "unknown",
                };
                let priority = match t.priority {
                    1 => "low",
                    2 => "normal",
                    3 => "high",
                    4 => "urgent",
                    _ => "unknown",
                };
                let (exec_type, exec_uri, exec_name, exec_sum, exec_args, exec_env) =
                    if let Some(exec) = t.executable {
                        let et = match exec.r#type {
                            1 => "binary",
                            2 => "script",
                            3 => "container",
                            4 => "wasm",
                            5 => "process",
                            _ => "unknown",
                        };
                        (
                            et.to_string(),
                            exec.uri,
                            exec.name,
                            exec.checksum_sha256,
                            exec.args,
                            exec.env,
                        )
                    } else {
                        (
                            String::new(),
                            String::new(),
                            String::new(),
                            String::new(),
                            vec![],
                            std::collections::HashMap::new(),
                        )
                    };
                let execution_kind = match t.execution_kind {
                    x if x == crate::proto::sms::TaskExecutionKind::LongRunning as i32 => {
                        "long_running".to_string()
                    }
                    x if x == crate::proto::sms::TaskExecutionKind::ShortRunning as i32 => {
                        "short_running".to_string()
                    }
                    _ => "short_running".to_string(),
                };
                Json(json!({
                    "found": true,
                    "task": {
                        "task_id": t.task_id,
                        "name": t.name,
                        "description": t.description,
                        "status": status,
                        "priority": priority,
                        "node_uuid": t.node_uuid,
                        "endpoint": t.endpoint,
                        "version": t.version,
                        "capabilities": t.capabilities,
                        "registered_at": t.registered_at,
                        "last_heartbeat": t.last_heartbeat,
                        "metadata": t.metadata,
                        "config": t.config,
                        "execution_kind": execution_kind,
                        "executable_type": exec_type,
                        "executable_uri": exec_uri,
                        "executable_name": exec_name,
                        "executable_checksum": exec_sum,
                        "executable_args": exec_args,
                        "executable_env": exec_env,
                        "result_uris": t.result_uris,
                        "last_result_uri": t.last_result_uri,
                        "last_result_status": t.last_result_status,
                        "last_completed_at": t.last_completed_at,
                        "last_result_metadata": t.last_result_metadata,
                    }
                }))
            } else {
                Json(json!({"found": false}))
            }
        }
        Err(_) => Json(json!({"found": false})),
    }
}

async fn get_node_detail(state: GatewayState, Path(uuid): Path<String>) -> Json<serde_json::Value> {
    use crate::proto::sms::GetNodeWithResourceRequest;
    let mut client = state.node_client.clone();
    let resp = client
        .get_node_with_resource(GetNodeWithResourceRequest { uuid })
        .await
        .unwrap()
        .into_inner();
    let node = resp.node;
    let resource = resp.resource;
    let body = json!({
        "found": node.is_some(),
        "node": node.map(|n| json!({
            "uuid": n.uuid,
            "ip_address": n.ip_address,
            "port": n.port,
            "status": n.status,
            "last_heartbeat": n.last_heartbeat,
            "registered_at": n.registered_at,
            "metadata": n.metadata,
        })),
        "resource": resource.map(|r| json!({
            "cpu_usage_percent": r.cpu_usage_percent,
            "memory_usage_percent": r.memory_usage_percent,
            "disk_usage_percent": r.disk_usage_percent,
            "total_memory_bytes": r.total_memory_bytes,
            "used_memory_bytes": r.used_memory_bytes,
            "available_memory_bytes": r.available_memory_bytes,
        })),
    });
    Json(body)
}

async fn get_stats(state: GatewayState) -> Json<serde_json::Value> {
    use crate::proto::sms::ListNodesRequest;
    let mut client = state.node_client.clone();
    let resp = client
        .list_nodes(ListNodesRequest {
            status_filter: String::new(),
        })
        .await
        .unwrap()
        .into_inner();
    let now = chrono::Utc::now().timestamp();
    let mut total = 0i64;
    let mut online = 0i64;
    let mut offline = 0i64;
    let mut recent_60s = 0i64;
    for n in resp.nodes {
        total += 1;
        let s = n.status.to_lowercase();
        if s == "online" || s == "active" {
            online += 1;
        } else {
            offline += 1;
        }
        if now - n.last_heartbeat <= 60 {
            recent_60s += 1;
        }
    }
    Json(json!({
        "total_count": total,
        "online_count": online,
        "offline_count": offline,
        "recent_60s_count": recent_60s,
    }))
}

async fn admin_index() -> Html<&'static str> {
    Html(include_str!("../../assets/admin/index.html"))
}

async fn admin_static(headers: HeaderMap, Path(path): Path<String>) -> impl IntoResponse {
    let path = path.trim_start_matches('/');
    let enc = headers
        .get(axum::http::header::ACCEPT_ENCODING)
        .and_then(|v| v.to_str().ok())
        .unwrap_or("")
        .to_lowercase();
    let (bytes, mime, content_encoding) = match (path, enc.as_str()) {
        ("main.js", enc) if enc.contains("br") => (
            include_bytes!(concat!(env!("OUT_DIR"), "/main.js.br")).as_ref(),
            "application/javascript",
            Some("br"),
        ),
        ("main.js", enc) if enc.contains("gzip") => (
            include_bytes!(concat!(env!("OUT_DIR"), "/main.js.gz")).as_ref(),
            "application/javascript",
            Some("gzip"),
        ),
        ("main.css", enc) if enc.contains("br") => (
            include_bytes!(concat!(env!("OUT_DIR"), "/main.css.br")).as_ref(),
            "text/css",
            Some("br"),
        ),
        ("main.css", enc) if enc.contains("gzip") => (
            include_bytes!(concat!(env!("OUT_DIR"), "/main.css.gz")).as_ref(),
            "text/css",
            Some("gzip"),
        ),
        ("index.html", enc) if enc.contains("br") => (
            include_bytes!(concat!(env!("OUT_DIR"), "/index.html.br")).as_ref(),
            "text/html",
            Some("br"),
        ),
        ("index.html", enc) if enc.contains("gzip") => (
            include_bytes!(concat!(env!("OUT_DIR"), "/index.html.gz")).as_ref(),
            "text/html",
            Some("gzip"),
        ),
        ("main.js", _) => (
            include_bytes!("../../assets/admin/main.js").as_ref(),
            "application/javascript",
            None,
        ),
        ("main.css", _) => (
            include_bytes!("../../assets/admin/main.css").as_ref(),
            "text/css",
            None,
        ),
        ("index.html", _) => (
            include_bytes!("../../assets/admin/index.html").as_ref(),
            "text/html",
            None,
        ),
        _ => return axum::http::StatusCode::NOT_FOUND.into_response(),
    };
    let mut resp = axum::response::Response::new(bytes.into());
    resp.headers_mut()
        .insert(CONTENT_TYPE, mime.parse().unwrap());
    // Reduce caching to ensure UI updates are visible / 减少缓存以确保前端更新可见
    let cache = match path {
        "index.html" | "main.js" | "main.css" => "no-cache, no-store, must-revalidate",
        _ => "public, max-age=31536000",
    };
    resp.headers_mut()
        .insert(CACHE_CONTROL, cache.parse().unwrap());
    resp.headers_mut()
        .insert(axum::http::header::VARY, "Accept-Encoding".parse().unwrap());
    if let Some(ce) = content_encoding {
        resp.headers_mut()
            .insert(axum::http::header::CONTENT_ENCODING, ce.parse().unwrap());
    }
    resp
}

#[derive(Deserialize)]
struct StreamQuery {
    once: Option<bool>,
}

async fn nodes_stream(
    state: GatewayState,
    Query(q): Query<StreamQuery>,
) -> impl axum::response::IntoResponse {
    use axum::response::sse::{Event, Sse};
    type DynSseStream = Pin<Box<dyn futures::Stream<Item = Result<Event, Infallible>> + Send>>;
    let mut client = state.node_client.clone();
    if q.once.unwrap_or(false) {
        let event = match client
            .list_nodes(ListNodesRequest {
                status_filter: String::new(),
            })
            .await
        {
            Ok(r) => {
                let nodes = r.into_inner().nodes;
                let payload =
                    serde_json::to_string(&json!({"type":"snapshot","count": nodes.len()}))
                        .unwrap();
                Event::default().event("snapshot").data(payload)
            }
            Err(_) => Event::default().event("error").data("{}"),
        };
        let single: DynSseStream = Box::pin(futures::stream::once(async move {
            Ok::<Event, Infallible>(event)
        }));
        return Sse::new(single);
    }

    let cancel = state.cancel_token.clone();
    let stream: DynSseStream = Box::pin(
        IntervalStream::new(tokio::time::interval(Duration::from_secs(5)))
            .take_until(cancel.cancelled_owned())
            .then(move |_| {
                let mut client = client.clone();
                async move {
                    match client
                        .list_nodes(ListNodesRequest {
                            status_filter: String::new(),
                        })
                        .await
                    {
                        Ok(r) => {
                            let nodes = r.into_inner().nodes;
                            let payload = serde_json::to_string(
                                &json!({"type":"snapshot","count": nodes.len()}),
                            )
                            .unwrap();
                            Ok::<Event, Infallible>(
                                Event::default().event("snapshot").data(payload),
                            )
                        }
                        Err(_) => {
                            Ok::<Event, Infallible>(Event::default().event("error").data("{}"))
                        }
                    }
                }
            }),
    );
    Sse::new(stream)
}
