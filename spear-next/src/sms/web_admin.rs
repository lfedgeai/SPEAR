use std::net::SocketAddr;
use anyhow::Result;
use axum::{Router, routing::get, extract::{Query, Path}, response::{Json, Html}};
use axum::middleware;
use axum::http::{Request, StatusCode};
use axum::middleware::Next;
use std::convert::Infallible;
use tokio_util::sync::CancellationToken;
use axum::response::IntoResponse;
use axum::http::header::{CONTENT_TYPE, CACHE_CONTROL};
use serde::Deserialize;
use serde_json::json;
use tokio_stream::wrappers::IntervalStream;
use futures::StreamExt;
use std::pin::Pin;
use std::time::Duration;

use crate::proto::sms::{node_service_client::NodeServiceClient, ListNodesRequest};
use crate::sms::gateway::GatewayState;

pub struct WebAdminServer {
    addr: SocketAddr,
    grpc_addr: SocketAddr,
    enabled: bool,
}

impl WebAdminServer {
    pub fn new(addr: SocketAddr, grpc_addr: SocketAddr, enabled: bool) -> Self {
        Self { addr, grpc_addr, enabled }
    }

    pub async fn start_with_shutdown<F>(self, shutdown: F) -> Result<()>
    where
        F: std::future::Future<Output = ()> + Send + 'static,
    {
        if !self.enabled { return Ok(()); }
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


    async fn prepare_with_token(&self, cancel_token: CancellationToken) -> Result<(tokio::net::TcpListener, Router)> {
        let grpc_url = format!("http://{}", self.grpc_addr);
        let channel = tonic::transport::Channel::from_shared(grpc_url)
            .expect("Invalid gRPC URL")
            .connect_lazy();
        let node_client = NodeServiceClient::new(channel.clone());
        let task_client = crate::proto::sms::task_service_client::TaskServiceClient::new(channel);
        let state = GatewayState { node_client, task_client, cancel_token: cancel_token.clone() };
        let mut app = create_admin_router(state);
        if let Ok(token) = std::env::var("SMS_WEB_ADMIN_TOKEN") {
            let bearer = format!("Bearer {}", token);
            app = app.layer(middleware::from_fn(move |req: Request<axum::body::Body>, next: Next| {
                let bearer = bearer.clone();
                async move {
                    let authorized = req.headers().get(axum::http::header::AUTHORIZATION)
                        .and_then(|h| h.to_str().ok())
                        .map(|v| v == bearer)
                        .unwrap_or(false);
                    if !authorized { return StatusCode::UNAUTHORIZED.into_response(); }
                    next.run(req).await
                }
            }));
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
        .route("/admin/static/*path", get(admin_static))
        .route("/admin/api/nodes", get({
            let state = state.clone();
            move |q: Query<ListQuery>| list_nodes(state.clone(), q)
        }))
        .route("/admin/api/nodes/:uuid", get({
            let state = state.clone();
            move |p: Path<String>| get_node_detail(state.clone(), p)
        }))
        .route("/admin/api/nodes/stream", get({
            let state = state.clone();
            move |q: Query<StreamQuery>| nodes_stream(state.clone(), q)
        }))
        .route("/admin/api/stats", get({
            let state = state.clone();
            move || get_stats(state.clone())
        }))
}

async fn list_nodes(state: GatewayState, Query(q): Query<ListQuery>) -> Json<serde_json::Value> {
    let mut client = state.node_client.clone();
    let req = ListNodesRequest { status_filter: q.status.unwrap_or_default() };
    let resp = client.list_nodes(req).await.unwrap().into_inner();
    let total = resp.nodes.len();
    let mut list = resp.nodes.into_iter().map(|n| {
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
    })}).collect::<Vec<_>>();
    if let Some(q) = q.q.as_ref().map(|s| s.to_lowercase()) {
        list = list.into_iter().filter(|item| {
            let uuid = item["uuid"].as_str().unwrap_or("").to_lowercase();
            let ip = item["ip_address"].as_str().unwrap_or("").to_lowercase();
            let meta = item["metadata"].as_object();
            let meta_hit = meta.map(|m| m.iter().any(|(k,v)| k.to_lowercase().contains(&q) || v.as_str().unwrap_or("").to_lowercase().contains(&q))).unwrap_or(false);
            uuid.contains(&q) || ip.contains(&q) || meta_hit
        }).collect();
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
    } else { (String::new(), true) };
    if !field.is_empty() {
        match field.as_str() {
            "last_heartbeat" => list.sort_by_key(|i| i["last_heartbeat"].as_i64().unwrap_or(0)),
            "registered_at" => list.sort_by_key(|i| i["registered_at"].as_i64().unwrap_or(0)),
            _ => {}
        }
        if !asc { list.reverse(); }
    }
    if let Some(offset) = q.offset { if offset < list.len() { list = list.split_off(offset); } else { list.clear(); } }
    if let Some(limit) = q.limit { if limit < list.len() { list.truncate(limit); } }
    Json(json!({ "nodes": list, "total_count": total }))
}

async fn get_node_detail(state: GatewayState, Path(uuid): Path<String>) -> Json<serde_json::Value> {
    use crate::proto::sms::GetNodeWithResourceRequest;
    let mut client = state.node_client.clone();
    let resp = client.get_node_with_resource(GetNodeWithResourceRequest { uuid }).await.unwrap().into_inner();
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
    let resp = client.list_nodes(ListNodesRequest { status_filter: String::new() }).await.unwrap().into_inner();
    let now = chrono::Utc::now().timestamp();
    let mut total = 0i64;
    let mut online = 0i64;
    let mut offline = 0i64;
    let mut recent_60s = 0i64;
    for n in resp.nodes {
        total += 1;
        let s = n.status.to_lowercase();
        if s == "online" || s == "active" { online += 1; } else { offline += 1; }
        if now - n.last_heartbeat <= 60 { recent_60s += 1; }
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

async fn admin_static(Path(path): Path<String>) -> impl IntoResponse {
    let path = path.trim_start_matches('/');
    let (bytes, mime) = match path {
        "app.js" => (include_bytes!("../../assets/admin/app.js").as_ref(), "application/javascript"),
        "react-app.js" => (include_bytes!("../../assets/admin/react-app.js").as_ref(), "application/javascript"),
        "style.css" => (include_bytes!("../../assets/admin/style.css").as_ref(), "text/css"),
        "index.html" => (include_bytes!("../../assets/admin/index.html").as_ref(), "text/html"),
        _ => return axum::http::StatusCode::NOT_FOUND.into_response(),
    };
    let mut resp = axum::response::Response::new(bytes.into());
    resp.headers_mut().insert(CONTENT_TYPE, mime.parse().unwrap());
    resp.headers_mut().insert(CACHE_CONTROL, "no-cache".parse().unwrap());
    resp
}

#[derive(Deserialize)]
struct StreamQuery { once: Option<bool> }

async fn nodes_stream(state: GatewayState, Query(q): Query<StreamQuery>) -> impl axum::response::IntoResponse {
    use axum::response::sse::{Event, Sse};
    type DynSseStream = Pin<Box<dyn futures::Stream<Item = Result<Event, Infallible>> + Send>>;
    let mut client = state.node_client.clone();
    if q.once.unwrap_or(false) {
        let event = match client.list_nodes(ListNodesRequest { status_filter: String::new() }).await {
            Ok(r) => {
                let nodes = r.into_inner().nodes;
                let payload = serde_json::to_string(&json!({"type":"snapshot","count": nodes.len()})).unwrap();
                Event::default().event("snapshot").data(payload)
            }
            Err(_) => Event::default().event("error").data("{}"),
        };
        let single: DynSseStream = Box::pin(futures::stream::once(async move { Ok::<Event, Infallible>(event) }));
        return Sse::new(single);
    }

    let cancel = state.cancel_token.clone();
    let stream: DynSseStream = Box::pin(IntervalStream::new(tokio::time::interval(Duration::from_secs(5)))
        .take_until(cancel.cancelled_owned())
        .then(move |_| {
            let mut client = client.clone();
            async move {
                match client.list_nodes(ListNodesRequest { status_filter: String::new() }).await {
                    Ok(r) => {
                        let nodes = r.into_inner().nodes;
                        let payload = serde_json::to_string(&json!({"type":"snapshot","count": nodes.len()})).unwrap();
                        Ok::<Event, Infallible>(Event::default().event("snapshot").data(payload))
                    }
                    Err(_) => Ok::<Event, Infallible>(Event::default().event("error").data("{}")),
                }
            }
        }));
    Sse::new(stream)
}
