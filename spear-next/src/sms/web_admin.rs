use anyhow::Result;
use axum::http::header::{CACHE_CONTROL, CONTENT_TYPE};
use axum::http::{HeaderMap, Request, StatusCode};
use axum::middleware;
use axum::middleware::Next;
use axum::response::IntoResponse;
use axum::{
    extract::{Path, Query, State},
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

use crate::proto::sms::{node_service_client::NodeServiceClient, ListNodesRequest};
use crate::sms::gateway::GatewayState;
use crate::sms::handlers::{
    delete_file, download_file, get_file_meta, list_files, presign_upload, upload_file,
};

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
        let task_client = crate::proto::sms::task_service_client::TaskServiceClient::new(channel);
        let state = GatewayState {
            node_client,
            task_client,
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
        list = list
            .into_iter()
            .filter(|item| {
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
            })
            .collect();
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
                "executable_type": exec_type,
                "executable_uri": exec_uri,
                "executable_name": exec_name,
            })
        })
        .collect::<Vec<_>>();
    if let Some(qs) = q.q.as_ref().map(|s| s.to_lowercase()) {
        list = list
            .into_iter()
            .filter(|item| {
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
            })
            .collect();
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
    let req = RegisterTaskRequest {
        name: body.name,
        description: body.description.unwrap_or_default(),
        priority: p,
        node_uuid: body.node_uuid,
        endpoint: body.endpoint,
        version: body.version,
        capabilities: body.capabilities.unwrap_or_default(),
        metadata: body.metadata.unwrap_or_default(),
        config: body.config.unwrap_or_default(),
        executable: exe,
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
                        "executable_type": exec_type,
                        "executable_uri": exec_uri,
                        "executable_name": exec_name,
                        "executable_checksum": exec_sum,
                        "executable_args": exec_args,
                        "executable_env": exec_env,
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
        ("react-app.js", enc) if enc.contains("br") => (
            include_bytes!(concat!(env!("OUT_DIR"), "/react-app.js.br")).as_ref(),
            "application/javascript",
            Some("br"),
        ),
        ("react-app.js", enc) if enc.contains("gzip") => (
            include_bytes!(concat!(env!("OUT_DIR"), "/react-app.js.gz")).as_ref(),
            "application/javascript",
            Some("gzip"),
        ),
        ("style.css", enc) if enc.contains("br") => (
            include_bytes!(concat!(env!("OUT_DIR"), "/style.css.br")).as_ref(),
            "text/css",
            Some("br"),
        ),
        ("style.css", enc) if enc.contains("gzip") => (
            include_bytes!(concat!(env!("OUT_DIR"), "/style.css.gz")).as_ref(),
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
        ("react-app.js", _) => (
            include_bytes!("../../assets/admin/react-app.js").as_ref(),
            "application/javascript",
            None,
        ),
        ("style.css", _) => (
            include_bytes!("../../assets/admin/style.css").as_ref(),
            "text/css",
            None,
        ),
        ("index.html", _) => (
            include_bytes!("../../assets/admin/index.html").as_ref(),
            "text/html",
            None,
        ),
        ("app.js", _) => (
            include_bytes!("../../assets/admin/app.js").as_ref(),
            "application/javascript",
            None,
        ),
        _ => return axum::http::StatusCode::NOT_FOUND.into_response(),
    };
    let mut resp = axum::response::Response::new(bytes.into());
    resp.headers_mut()
        .insert(CONTENT_TYPE, mime.parse().unwrap());
    // Reduce caching to ensure UI updates are visible / 减少缓存以确保前端更新可见
    let cache = match path {
        "index.html" | "react-app.js" | "style.css" => "no-cache, no-store, must-revalidate",
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
