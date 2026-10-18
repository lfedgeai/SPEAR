use axum::{
    extract::{Path, Query},
    routing::{delete, get, post, put},
    Json, Router,
};

use crate::sms::gateway::GatewayState;
use crate::sms::handlers::{
    delete_object, download_execution_logs_admin, download_object, get_execution_logs_admin,
    get_object_meta, get_object_storage_status, list_objects, pin_object, release_object,
    retain_object, unpin_object, upload_object,
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
            "/admin/api/nodes/{uuid}/ai/credentials",
            get({
                let state = state.clone();
                move |p: Path<String>| super::get_node_ai_credential_sync(state.clone(), p)
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
            "/admin/api/nodes/{uuid}/backends",
            get({
                let state = state.clone();
                move |p: Path<String>| super::get_node_backends(state.clone(), p)
            }),
        )
        .route(
            "/admin/api/ai-backends",
            get({
                let state = state.clone();
                move |q: Query<super::types::AiBackendsQuery>| {
                    super::list_ai_backends_admin(state.clone(), q)
                }
            }),
        )
        .route(
            "/admin/api/ai-backends",
            post({
                let state = state.clone();
                move |body: Json<super::AiBackendWriteBody>| {
                    super::create_ai_backend_admin(state.clone(), body)
                }
            }),
        )
        .route(
            "/admin/api/ai-backends/preflight",
            post({
                let state = state.clone();
                move |body: Json<super::AiBackendPreflightBody>| {
                    super::preflight_ai_backend_admin(state.clone(), body)
                }
            }),
        )
        .route(
            "/admin/api/ai-backends/{backend_id}",
            get({
                let state = state.clone();
                move |p: Path<String>| super::get_ai_backend_admin(state.clone(), p)
            }),
        )
        .route(
            "/admin/api/ai-backends/{backend_id}",
            put({
                let state = state.clone();
                move |p: Path<String>, body: Json<super::AiBackendWriteBody>| {
                    super::update_ai_backend_admin(state.clone(), p, body)
                }
            }),
        )
        .route(
            "/admin/api/ai-backends/{backend_id}",
            delete({
                let state = state.clone();
                move |p: Path<String>| super::delete_ai_backend_admin(state.clone(), p)
            }),
        )
        .route(
            "/admin/api/ai-backends/{backend_id}/desired-state",
            post({
                let state = state.clone();
                move |p: Path<String>, body: Json<super::SetAiBackendDesiredStateBody>| {
                    super::set_ai_backend_desired_state_admin(state.clone(), p, body)
                }
            }),
        )
        .route(
            "/admin/api/ai-backend-placements",
            get({
                let state = state.clone();
                move |q: Query<super::AiBackendPlacementsQuery>| {
                    super::list_ai_backend_placements_admin(state.clone(), q)
                }
            }),
        )
        .route(
            "/admin/api/ai-backend-placements",
            post({
                let state = state.clone();
                move |body: Json<super::AiBackendPlacementWriteBody>| {
                    super::upsert_ai_backend_placement_admin(state.clone(), body)
                }
            }),
        )
        .route(
            "/admin/api/ai-backend-placements/{placement_id}",
            delete({
                let state = state.clone();
                move |p: Path<String>| super::delete_ai_backend_placement_admin(state.clone(), p)
            }),
        )
        .route(
            "/admin/api/ai-backend-assignments/{node_uuid}",
            get({
                let state = state.clone();
                move |p: Path<String>| super::list_ai_backend_assignments_admin(state.clone(), p)
            }),
        )
        .route(
            "/admin/api/ai-backend-statuses/{backend_id}",
            get({
                let state = state.clone();
                move |p: Path<String>| super::list_ai_backend_node_statuses_admin(state.clone(), p)
            }),
        )
        .route(
            "/admin/api/ai-model-views",
            get({
                let state = state.clone();
                move |q: Query<super::types::AiModelViewsQuery>| {
                    super::list_ai_model_views_admin(state.clone(), q)
                }
            }),
        )
        .route(
            "/admin/api/ai/credentials",
            get({
                let state = state.clone();
                move || super::list_credentials_admin(state.clone())
            }),
        )
        .route(
            "/admin/api/ai/credentials",
            post({
                let state = state.clone();
                move |body: Json<super::UpsertCredentialBody>| {
                    super::upsert_credential_admin(state.clone(), body)
                }
            }),
        )
        .route(
            "/admin/api/ai/credentials/{name}",
            delete({
                let state = state.clone();
                move |p: Path<String>| super::delete_credential_admin(state.clone(), p)
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
            "/admin/api/tasks/{task_id}",
            delete({
                let state = state.clone();
                move |p: Path<String>, payload: axum::extract::Json<super::DeleteTaskBody>| {
                    super::delete_task_admin(state.clone(), p, payload)
                }
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
            "/admin/api/instances/{instance_id}",
            get({
                let state = state.clone();
                move |p: Path<String>| super::get_instance_admin(state.clone(), p)
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
            "/admin/api/instances/{instance_id}/destroy",
            post({
                let state = state.clone();
                move |p: Path<String>, body: axum::extract::Json<super::DestroyInstanceBody>| {
                    super::destroy_instance_admin(state.clone(), p, body)
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
            get({
                let state = state.clone();
                move |q: Query<super::types::ExecutionHistoryQuery>| {
                    super::list_execution_history_admin(state.clone(), q)
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
            "/admin/api/executions/{execution_id}/terminate",
            post({
                let state = state.clone();
                move |p: Path<String>, body: axum::extract::Json<super::TerminateExecutionBody>| {
                    super::terminate_execution_admin(state.clone(), p, body)
                }
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
        .route("/admin/api/objects", get(list_objects))
        .route("/admin/api/objects", post(upload_object))
        .route("/admin/api/objects/{key}", get(get_object_meta))
        .route("/admin/api/objects/{key}", delete(delete_object))
        .route("/admin/api/objects/{key}/content", get(download_object))
        .route("/admin/api/objects/{key}/retain", post(retain_object))
        .route("/admin/api/objects/{key}/release", post(release_object))
        .route("/admin/api/objects/{key}/pin", post(pin_object))
        .route("/admin/api/objects/{key}/unpin", post(unpin_object))
        .route("/admin/api/object-storage/status", get(get_object_storage_status))
        .with_state(state)
}
