use axum::extract::{Path, Query};
use axum::http::StatusCode;
use axum::response::IntoResponse;
use axum::Json;
use serde::Deserialize;
use serde_json::json;

use crate::sms::execution_logs::{read_logs_download_text, read_logs_page};

#[derive(Debug, Deserialize)]
pub struct ReadLogsQuery {
    pub cursor: Option<String>,
    pub limit: Option<usize>,
}

#[derive(Debug, Deserialize)]
pub struct DownloadLogsQuery {
    pub format: Option<String>,
}

pub async fn get_execution_logs_admin(
    Path(execution_id): Path<String>,
    Query(q): Query<ReadLogsQuery>,
) -> Result<Json<serde_json::Value>, StatusCode> {
    let limit = q.limit.unwrap_or(200).max(1).min(2000);
    let cursor = q.cursor.as_deref();
    let out = read_logs_page(&execution_id, cursor, limit)
        .await
        .map_err(|_| StatusCode::INTERNAL_SERVER_ERROR)?;
    Ok(Json(json!({
        "success": true,
        "execution_id": execution_id,
        "lines": out.lines,
        "next_cursor": out.next_cursor,
        "truncated": out.truncated,
        "completed": out.completed,
    })))
}

pub async fn download_execution_logs_admin(
    Path(execution_id): Path<String>,
    Query(q): Query<DownloadLogsQuery>,
) -> Result<impl IntoResponse, StatusCode> {
    let format = q.format.unwrap_or_else(|| "text".to_string());
    if format != "text" {
        return Err(StatusCode::BAD_REQUEST);
    }
    let (bytes, truncated) = read_logs_download_text(&execution_id)
        .await
        .map_err(|_| StatusCode::INTERNAL_SERVER_ERROR)?;
    let mut resp = axum::response::Response::new(axum::body::Body::from(bytes));
    resp.headers_mut().insert(
        axum::http::header::CONTENT_TYPE,
        "text/plain; charset=utf-8".parse().unwrap(),
    );
    resp.headers_mut().insert(
        axum::http::header::CACHE_CONTROL,
        "no-cache, no-store, must-revalidate".parse().unwrap(),
    );
    if truncated {
        resp.headers_mut().insert(
            axum::http::header::HeaderName::from_static("x-sms-log-truncated"),
            "1".parse().unwrap(),
        );
    }
    Ok(resp)
}
