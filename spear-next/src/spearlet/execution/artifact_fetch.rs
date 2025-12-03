use crate::spearlet::execution::{ExecutionError, ExecutionResult};
use reqwest::StatusCode;

pub async fn fetch_sms_file(sms_http_addr: &str, path: &str) -> ExecutionResult<Vec<u8>> {
    let url = if path.starts_with('/') { format!("http://{}{}", sms_http_addr, path) } else { format!("http://{}/{}", sms_http_addr, path) };
    let resp = reqwest::get(url)
        .await
        .map_err(|e| ExecutionError::RuntimeError { message: format!("Failed to fetch SMS file: {}", e) })?;
    if resp.status() != StatusCode::OK {
        return Err(ExecutionError::RuntimeError { message: format!("SMS file download failed: status {}", resp.status()) });
    }
    let body = resp.bytes().await.map_err(|e| ExecutionError::RuntimeError { message: format!("Failed to read SMS file body: {}", e) })?;
    Ok(body.to_vec())
}
