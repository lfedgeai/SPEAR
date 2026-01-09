use axum::{extract::State, http::StatusCode, response::Json};
use serde::{Deserialize, Serialize};
use serde_json::json;

use crate::proto::sms::{
    InvocationOutcomeClass, PlaceInvocationRequest, ReportInvocationOutcomeRequest,
};
use crate::sms::gateway::GatewayState;

/// Place an invocation via HTTP (REST) and return candidate nodes.
///
/// 通过 HTTP（REST）请求 SMS placement，返回候选节点列表。
#[derive(Debug, Deserialize, Serialize)]
pub struct HttpPlaceInvocationRequest {
    pub request_id: String,
    pub task_id: String,
    pub max_candidates: Option<u32>,
    pub labels: Option<std::collections::HashMap<String, String>>,
}

/// REST endpoint: POST /api/v1/placement/invocations/place
///
/// REST 端点：POST /api/v1/placement/invocations/place
pub async fn place_invocation(
    State(state): State<GatewayState>,
    Json(req): Json<HttpPlaceInvocationRequest>,
) -> Result<(StatusCode, Json<serde_json::Value>), StatusCode> {
    let mut client = state.placement_client.clone();
    let grpc_req = PlaceInvocationRequest {
        request_id: req.request_id,
        task_id: req.task_id,
        max_candidates: req.max_candidates.unwrap_or(3),
        labels: req.labels.unwrap_or_default(),
    };
    match client.place_invocation(grpc_req).await {
        Ok(resp) => {
            let inner = resp.into_inner();
            let candidates = inner
                .candidates
                .into_iter()
                .map(|c| {
                    json!({
                        "node_uuid": c.node_uuid,
                        "ip_address": c.ip_address,
                        "port": c.port,
                        "score": c.score,
                    })
                })
                .collect::<Vec<_>>();
            Ok((
                StatusCode::OK,
                Json(json!({
                    "decision_id": inner.decision_id,
                    "candidates": candidates,
                })),
            ))
        }
        Err(_) => Err(StatusCode::SERVICE_UNAVAILABLE),
    }
}

/// Report invocation outcome via HTTP (REST) for placement feedback.
///
/// 通过 HTTP（REST）上报执行结果，用于 placement 反馈（惩罚/熔断/恢复）。
#[derive(Debug, Deserialize, Serialize)]
pub struct HttpReportInvocationOutcomeRequest {
    pub decision_id: String,
    pub request_id: String,
    pub task_id: String,
    pub node_uuid: String,
    pub outcome_class: Option<String>,
    pub error_message: Option<String>,
}

/// Parse outcome_class from HTTP into SMS enum.
///
/// 将 HTTP 字符串的 outcome_class 解析成 SMS 的枚举值。
fn parse_outcome_class(v: Option<String>) -> i32 {
    match v.as_deref().map(|s| s.to_ascii_lowercase()) {
        Some(s) if s == "success" => InvocationOutcomeClass::Success as i32,
        Some(s) if s == "overloaded" => InvocationOutcomeClass::Overloaded as i32,
        Some(s) if s == "unavailable" => InvocationOutcomeClass::Unavailable as i32,
        Some(s) if s == "timeout" => InvocationOutcomeClass::Timeout as i32,
        Some(s) if s == "rejected" => InvocationOutcomeClass::Rejected as i32,
        Some(s) if s == "bad_request" => InvocationOutcomeClass::BadRequest as i32,
        Some(s) if s == "internal" => InvocationOutcomeClass::Internal as i32,
        _ => InvocationOutcomeClass::Unknown as i32,
    }
}

/// REST endpoint: POST /api/v1/placement/invocations/report-outcome
///
/// REST 端点：POST /api/v1/placement/invocations/report-outcome
pub async fn report_invocation_outcome(
    State(state): State<GatewayState>,
    Json(req): Json<HttpReportInvocationOutcomeRequest>,
) -> Result<(StatusCode, Json<serde_json::Value>), StatusCode> {
    let mut client = state.placement_client.clone();
    let grpc_req = ReportInvocationOutcomeRequest {
        decision_id: req.decision_id,
        request_id: req.request_id,
        task_id: req.task_id,
        node_uuid: req.node_uuid,
        outcome_class: parse_outcome_class(req.outcome_class),
        error_message: req.error_message.unwrap_or_default(),
    };
    match client.report_invocation_outcome(grpc_req).await {
        Ok(resp) => {
            let inner = resp.into_inner();
            Ok((StatusCode::OK, Json(json!({ "accepted": inner.accepted }))))
        }
        Err(_) => Err(StatusCode::SERVICE_UNAVAILABLE),
    }
}
