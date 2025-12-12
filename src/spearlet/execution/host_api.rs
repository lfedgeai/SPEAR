use crate::spearlet::execution::ExecutionError;
use std::collections::HashMap;
use std::time::{SystemTime, UNIX_EPOCH};
use tracing::{debug, info, warn};

pub trait SpearHostApi: Send + Sync {
    fn log(&self, level: &str, message: &str);
    fn time_now_ms(&self) -> u64;
    fn random_bytes(&self, len: usize) -> Vec<u8>;
    fn get_env(&self, key: &str) -> Option<String>;
    fn http_call(
        &self,
        method: &str,
        url: &str,
        headers: HashMap<String, String>,
        body: Vec<u8>,
    ) -> Result<(i32, Vec<u8>, HashMap<String, String>), ExecutionError>;
    fn put_result(
        &self,
        task_id: &str,
        data: Vec<u8>,
        metadata: HashMap<String, String>,
    ) -> Result<String, ExecutionError>;
    fn get_object(&self, id: &str) -> Result<Vec<u8>, ExecutionError>;
    fn put_object(&self, name: &str, bytes: Vec<u8>) -> Result<String, ExecutionError>;
}

#[derive(Clone, Debug)]
pub struct DefaultHostApi {
    runtime_config: super::runtime::RuntimeConfig,
}

impl DefaultHostApi {
    pub fn new(runtime_config: super::runtime::RuntimeConfig) -> Self {
        Self { runtime_config }
    }
}

impl SpearHostApi for DefaultHostApi {
    fn log(&self, level: &str, message: &str) {
        match level.to_ascii_lowercase().as_str() {
            "debug" => debug!("{}", message),
            "info" => info!("{}", message),
            "warn" => warn!("{}", message),
            _ => info!("{}", message),
        }
    }

    fn time_now_ms(&self) -> u64 {
        SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .unwrap_or_default()
            .as_millis() as u64
    }

    fn random_bytes(&self, len: usize) -> Vec<u8> {
        let mut out = Vec::with_capacity(len);
        while out.len() < len {
            out.extend_from_slice(uuid::Uuid::new_v4().as_bytes());
        }
        out.truncate(len);
        out
    }

    fn get_env(&self, key: &str) -> Option<String> {
        self.runtime_config.global_environment.get(key).cloned()
    }

    fn http_call(
        &self,
        method: &str,
        url: &str,
        headers: HashMap<String, String>,
        body: Vec<u8>,
    ) -> Result<(i32, Vec<u8>, HashMap<String, String>), ExecutionError> {
        let rt = tokio::runtime::Runtime::new().map_err(|e| ExecutionError::RuntimeError {
            message: e.to_string(),
        })?;
        rt.block_on(async move {
            let client = reqwest::Client::new();
            let mut req = match method.to_ascii_uppercase().as_str() {
                "GET" => client.get(url),
                "POST" => client.post(url),
                "PUT" => client.put(url),
                "DELETE" => client.delete(url),
                _ => {
                    return Err(ExecutionError::InvalidRequest {
                        message: format!("Unsupported method: {}", method),
                    })
                }
            };
            for (k, v) in headers.iter() {
                req = req.header(k, v);
            }
            let resp =
                if method.eq_ignore_ascii_case("GET") || method.eq_ignore_ascii_case("DELETE") {
                    req.send().await.map_err(|e| ExecutionError::RuntimeError {
                        message: e.to_string(),
                    })?
                } else {
                    req.body(body)
                        .send()
                        .await
                        .map_err(|e| ExecutionError::RuntimeError {
                            message: e.to_string(),
                        })?
                };
            let status = resp.status().as_u16() as i32;
            let mut out_headers = HashMap::new();
            for (k, v) in resp.headers().iter() {
                out_headers.insert(k.to_string(), v.to_str().unwrap_or("").to_string());
            }
            let bytes = resp
                .bytes()
                .await
                .map_err(|e| ExecutionError::RuntimeError {
                    message: e.to_string(),
                })?
                .to_vec();
            Ok((status, bytes, out_headers))
        })
    }

    fn put_result(
        &self,
        task_id: &str,
        data: Vec<u8>,
        metadata: HashMap<String, String>,
    ) -> Result<String, ExecutionError> {
        let name = format!("{}-{}.bin", task_id, self.time_now_ms());
        let id = self.put_object(&name, data)?;
        let result_uri = format!("sms+file://{}", id);
        let addr = self
            .runtime_config
            .spearlet_config
            .as_ref()
            .map(|c| c.sms_grpc_addr.clone())
            .unwrap_or_else(|| "127.0.0.1:50051".to_string());
        let url = format!("http://{}", addr);
        let completed_at = (SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .unwrap_or_default()
            .as_secs()) as i64;
        let req = crate::proto::sms::UpdateTaskResultRequest {
            task_id: task_id.to_string(),
            result_uri: result_uri.clone(),
            result_status: "completed".to_string(),
            completed_at,
            result_metadata: metadata,
        };
        let rt = tokio::runtime::Runtime::new().map_err(|e| ExecutionError::RuntimeError {
            message: e.to_string(),
        })?;
        rt.block_on(async move {
            match tonic::transport::Channel::from_shared(url)
                .unwrap()
                .connect()
                .await
            {
                Ok(channel) => {
                    let mut client =
                        crate::proto::sms::task_service_client::TaskServiceClient::new(channel);
                    if let Err(e) = client.update_task_result(req).await {
                        return Err(ExecutionError::RuntimeError {
                            message: e.to_string(),
                        });
                    }
                }
                Err(e) => {
                    return Err(ExecutionError::RuntimeError {
                        message: e.to_string(),
                    })
                }
            }
            Ok(())
        })?;
        Ok(result_uri)
    }

    fn get_object(&self, id: &str) -> Result<Vec<u8>, ExecutionError> {
        let addr = self
            .runtime_config
            .spearlet_config
            .as_ref()
            .map(|c| c.sms_http_addr.clone())
            .ok_or_else(|| ExecutionError::InvalidConfiguration {
                message: "Missing SpearletConfig.sms_http_addr".to_string(),
            })?;
        let path = format!("/api/v1/files/{}", id);
        let rt = tokio::runtime::Runtime::new().map_err(|e| ExecutionError::RuntimeError {
            message: e.to_string(),
        })?;
        let bytes = rt.block_on(async move {
            crate::spearlet::execution::artifact_fetch::fetch_sms_file(&addr, &path).await
        })?;
        Ok(bytes)
    }

    fn put_object(&self, name: &str, bytes: Vec<u8>) -> Result<String, ExecutionError> {
        let addr = self
            .runtime_config
            .spearlet_config
            .as_ref()
            .map(|c| c.sms_http_addr.clone())
            .ok_or_else(|| ExecutionError::InvalidConfiguration {
                message: "Missing SpearletConfig.sms_http_addr".to_string(),
            })?;
        let url = format!("http://{}/admin/api/files", addr);
        let rt = tokio::runtime::Runtime::new().map_err(|e| ExecutionError::RuntimeError {
            message: e.to_string(),
        })?;
        let id = rt.block_on(async move {
            let client = reqwest::Client::new();
            let resp = client
                .post(&url)
                .header("content-type", "application/octet-stream")
                .header("x-file-name", name)
                .body(bytes)
                .send()
                .await
                .map_err(|e| ExecutionError::RuntimeError {
                    message: e.to_string(),
                })?;
            if !resp.status().is_success() {
                return Err(ExecutionError::RuntimeError {
                    message: format!("Upload failed: {}", resp.status()),
                });
            }
            let v: serde_json::Value =
                resp.json()
                    .await
                    .map_err(|e| ExecutionError::RuntimeError {
                        message: e.to_string(),
                    })?;
            let id = v
                .get("id")
                .and_then(|x| x.as_str())
                .unwrap_or("")
                .to_string();
            if id.is_empty() {
                return Err(ExecutionError::RuntimeError {
                    message: "Upload response missing id".to_string(),
                });
            }
            Ok::<String, ExecutionError>(id)
        })?;
        Ok(id)
    }
}
