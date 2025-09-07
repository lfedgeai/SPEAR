use axum::Json;
use serde_json::json;

/// Health check endpoint / 健康检查端点
pub async fn health_check() -> Json<serde_json::Value> {
    Json(json!({
        "status": "healthy",
        "timestamp": chrono::Utc::now().to_rfc3339(),
        "service": "sms"
    }))
}