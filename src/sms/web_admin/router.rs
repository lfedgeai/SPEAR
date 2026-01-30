use axum::{
    extract::{Path, Query},
    routing::{delete, get, post},
    Json, Router,
};

use crate::sms::gateway::GatewayState;
use crate::sms::handlers::{
    delete_file, download_execution_logs_admin, download_file, get_execution_logs_admin,
    get_file_meta, list_files, presign_upload, upload_file,
};

use super::types::{ListQuery, PageTokenQuery, StreamQuery};

pub fn create_admin_router(state: GatewayState) -> Router {
    Router::new()
        .route("/", get(super::admin_index))
        .route("/admin", get(super::admin_index))
        .route("/admin/", get(super::admin_index))
        .route("/admin/static/{*path}", get(super::admin_static))
        .route(
            "/admin/api/nodes",
            get({
                let state = state.clone();
                move |q: Query<ListQuery>| super::list_nodes(state.clone(), q)
            }),
        )
        .route(
            "/admin/api/nodes/{uuid}",
            get({
                let state = state.clone();
                move |p: Path<String>| super::get_node_detail(state.clone(), p)
            }),
        )
        .route(
            "/admin/api/nodes/stream",
            get({
                let state = state.clone();
                move |q: Query<StreamQuery>| super::nodes_stream(state.clone(), q)
            }),
        )
        .route(
            "/admin/api/stats",
            get({
                let state = state.clone();
                move || super::get_stats(state.clone())
            }),
        )
        .route(
            "/admin/api/backends",
            get({
                let state = state.clone();
                move |q: Query<ListQuery>| super::list_backends(state.clone(), q)
            }),
        )
        .route(
            "/admin/api/backends/{kind}/{name}",
            get({
                let state = state.clone();
                move |p: Path<(String, String)>| super::get_backend_detail(state.clone(), p)
            }),
        )
        .route(
            "/admin/api/nodes/{uuid}/backends",
            get({
                let state = state.clone();
                move |p: Path<String>| super::get_node_backends(state.clone(), p)
            }),
        )
        .route(
            "/admin/api/mcp/servers",
            get({
                let state = state.clone();
                move || super::list_mcp_servers(state.clone())
            }),
        )
        .route(
            "/admin/api/mcp/servers/{server_id}",
            get({
                let state = state.clone();
                move |p: Path<String>| super::get_mcp_server(state.clone(), p)
            }),
        )
        .route(
            "/admin/api/mcp/servers",
            post({
                let state = state.clone();
                move |body: Json<super::McpServerUpsertBody>| {
                    super::upsert_mcp_server(state.clone(), body)
                }
            }),
        )
        .route(
            "/admin/api/mcp/servers/{server_id}",
            delete({
                let state = state.clone();
                move |p: Path<String>| super::delete_mcp_server(state.clone(), p)
            }),
        )
        .route(
            "/admin/api/tasks",
            get({
                let state = state.clone();
                move |q: Query<ListQuery>| super::list_tasks(state.clone(), q)
            }),
        )
        .route(
            "/admin/api/tasks",
            post({
                let state = state.clone();
                move |payload: axum::extract::Json<super::CreateTaskBody>| {
                    super::create_task(state.clone(), payload)
                }
            }),
        )
        .route(
            "/admin/api/tasks/{task_id}",
            get({
                let state = state.clone();
                move |p: Path<String>| super::get_task_detail(state.clone(), p)
            }),
        )
        .route(
            "/admin/api/tasks/{task_id}/instances",
            get({
                let state = state.clone();
                move |p: Path<String>, q: Query<PageTokenQuery>| {
                    super::list_task_instances_admin(state.clone(), p, q)
                }
            }),
        )
        .route(
            "/admin/api/instances/{instance_id}/executions",
            get({
                let state = state.clone();
                move |p: Path<String>, q: Query<PageTokenQuery>| {
                    super::list_instance_executions_admin(state.clone(), p, q)
                }
            }),
        )
        .route(
            "/admin/api/invocations",
            post({
                let state = state.clone();
                move |payload: axum::extract::Json<super::CreateExecutionBody>| {
                    super::create_invocation(state.clone(), payload)
                }
            }),
        )
        .route(
            "/admin/api/executions",
            post({
                let state = state.clone();
                move |payload: axum::extract::Json<super::CreateExecutionBody>| {
                    super::create_invocation(state.clone(), payload)
                }
            }),
        )
        .route(
            "/admin/api/executions/{execution_id}",
            get({
                let state = state.clone();
                move |p: Path<String>| super::get_execution_admin(state.clone(), p)
            }),
        )
        .route(
            "/admin/api/executions/{execution_id}/logs",
            get(get_execution_logs_admin),
        )
        .route(
            "/admin/api/executions/{execution_id}/logs/download",
            get(download_execution_logs_admin),
        )
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
