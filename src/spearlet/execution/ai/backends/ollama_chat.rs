use serde_json::{json, Value};
use std::collections::HashMap;
use std::future::Future;
use std::time::Duration;

use crate::spearlet::execution::ai::backends::BackendAdapter;
use crate::spearlet::execution::ai::ir::{
    CanonicalError, CanonicalRequestEnvelope, CanonicalResponseEnvelope, Operation, Payload,
    ResultPayload,
};
use crate::spearlet::param_keys::{chat as chat_keys, mcp as mcp_keys};

pub struct OllamaChatBackendAdapter {
    name: String,
    base_url: String,
    fixed_model: Option<String>,
}

impl OllamaChatBackendAdapter {
    pub fn new(
        name: impl Into<String>,
        base_url: impl Into<String>,
        fixed_model: Option<String>,
    ) -> Self {
        Self {
            name: name.into(),
            base_url: base_url.into(),
            fixed_model,
        }
    }

    fn join_url(&self, path: &str) -> String {
        let mut base = self.base_url.trim_end_matches('/').to_string();
        base.push('/');
        base.push_str(path.trim_start_matches('/'));
        base
    }

    fn build_chat_body(&self, req: &CanonicalRequestEnvelope) -> Result<Value, CanonicalError> {
        let Payload::ChatCompletions(p) = &req.payload else {
            return Err(CanonicalError {
                code: "invalid_request".to_string(),
                message: "expected chat_completions payload".to_string(),
                retryable: false,
                operation: Some(req.operation.clone()),
            });
        };

        let model = self
            .fixed_model
            .clone()
            .filter(|s| !s.trim().is_empty())
            .unwrap_or_else(|| p.model.clone());

        let messages = p
            .messages
            .iter()
            .map(|m| {
                let content = if let Some(s) = m.content.as_str() {
                    s.to_string()
                } else {
                    serde_json::to_string(&m.content).unwrap_or_default()
                };
                json!({
                    "role": m.role,
                    "content": content,
                })
            })
            .collect::<Vec<_>>();

        let mut body = json!({
            "model": model,
            "messages": messages,
            "stream": false,
        });

        if !p.tools.is_empty() {
            body["tools"] = Value::Array(p.tools.clone());
        }

        if let Some(obj) = body.as_object_mut() {
            let mut options = serde_json::Map::new();
            for (k, v) in p.params.iter() {
                if k.starts_with(mcp_keys::param::PREFIX) {
                    continue;
                }
                if chat_keys::is_structural_param_key(k) {
                    continue;
                }
                options.insert(k.clone(), v.clone());
            }
            if !options.is_empty() {
                obj.insert("options".to_string(), Value::Object(options));
            }
        }

        Ok(body)
    }

    fn to_openai_chat_completion(
        &self,
        req: &CanonicalRequestEnvelope,
        model: String,
        assistant_content: String,
    ) -> Value {
        json!({
            "id": req.request_id,
            "object": "chat.completion",
            "created": chrono::Utc::now().timestamp(),
            "model": model,
            "choices": [
                {
                    "index": 0,
                    "message": {
                        "role": "assistant",
                        "content": assistant_content,
                    },
                    "finish_reason": "stop",
                }
            ]
        })
    }

    fn extract_error_message(v: &Value) -> Option<String> {
        v.get("error")
            .and_then(|x| x.as_str())
            .map(|s| s.to_string())
    }
}

impl BackendAdapter for OllamaChatBackendAdapter {
    fn name(&self) -> &str {
        &self.name
    }

    fn invoke(
        &self,
        req: &CanonicalRequestEnvelope,
    ) -> Result<CanonicalResponseEnvelope, CanonicalError> {
        if req.operation != Operation::ChatCompletions {
            return Err(CanonicalError {
                code: "unsupported_operation".to_string(),
                message: "ollama_chat supports chat_completions only".to_string(),
                retryable: false,
                operation: Some(req.operation.clone()),
            });
        }

        let body_json = self.build_chat_body(req)?;
        let model = body_json
            .get("model")
            .and_then(|v| v.as_str())
            .unwrap_or("")
            .to_string();
        if model.trim().is_empty() {
            return Err(CanonicalError {
                code: "invalid_request".to_string(),
                message: "missing model".to_string(),
                retryable: false,
                operation: Some(req.operation.clone()),
            });
        }

        let body_bytes = serde_json::to_vec(&body_json).map_err(|e| CanonicalError {
            code: "serialization".to_string(),
            message: e.to_string(),
            retryable: false,
            operation: Some(req.operation.clone()),
        })?;

        let url = self.join_url("api/chat");
        let timeout = req.timeout_ms.map(Duration::from_millis);

        let (status, resp_body, _headers) = run_async(async move {
            let client = reqwest::Client::new();
            let mut r = client
                .post(url)
                .header("content-type", "application/json")
                .body(body_bytes);
            if let Some(t) = timeout {
                r = r.timeout(t);
            }
            let resp = r.send().await.map_err(|e| CanonicalError {
                code: "network_error".to_string(),
                message: e.to_string(),
                retryable: true,
                operation: Some(Operation::ChatCompletions),
            })?;
            let status = resp.status();
            let headers = resp
                .headers()
                .iter()
                .filter_map(|(k, v)| v.to_str().ok().map(|vs| (k.to_string(), vs.to_string())))
                .collect::<HashMap<_, _>>();
            let body = resp.bytes().await.map_err(|e| CanonicalError {
                code: "network_error".to_string(),
                message: e.to_string(),
                retryable: true,
                operation: Some(Operation::ChatCompletions),
            })?;
            Ok::<_, CanonicalError>((status.as_u16() as i32, body.to_vec(), headers))
        })?;

        let status_u16 = status as u16;
        let ok = (200..300).contains(&status_u16);
        let parsed = serde_json::from_slice::<Value>(&resp_body).map_err(|e| CanonicalError {
            code: "invalid_response".to_string(),
            message: e.to_string(),
            retryable: status_u16 >= 500,
            operation: Some(req.operation.clone()),
        })?;

        if !ok {
            let extra = Self::extract_error_message(&parsed);
            return Err(CanonicalError {
                code: "upstream_error".to_string(),
                message: match extra {
                    Some(m) => format!("upstream status: {}: {}", status_u16, m),
                    None => format!("upstream status: {}", status_u16),
                },
                retryable: status_u16 == 429 || status_u16 >= 500,
                operation: Some(req.operation.clone()),
            });
        }

        let assistant_content = parsed
            .get("message")
            .and_then(|m| m.get("content"))
            .and_then(|c| c.as_str())
            .unwrap_or("")
            .to_string();
        let openai_like = self.to_openai_chat_completion(req, model, assistant_content);

        Ok(CanonicalResponseEnvelope {
            version: 1,
            request_id: req.request_id.clone(),
            operation: req.operation.clone(),
            backend: self.name.clone(),
            result: ResultPayload::Payload(openai_like),
            raw: Some(resp_body),
        })
    }
}

fn run_async<T>(
    fut: impl Future<Output = Result<T, CanonicalError>> + Send + 'static,
) -> Result<T, CanonicalError>
where
    T: Send + 'static,
{
    match tokio::runtime::Handle::try_current() {
        Ok(_) => std::thread::spawn(move || {
            let rt = tokio::runtime::Runtime::new().map_err(|e| CanonicalError {
                code: "runtime_error".to_string(),
                message: e.to_string(),
                retryable: false,
                operation: Some(Operation::ChatCompletions),
            })?;
            rt.block_on(fut)
        })
        .join()
        .unwrap_or_else(|_| {
            Err(CanonicalError {
                code: "runtime_error".to_string(),
                message: "thread join failed".to_string(),
                retryable: false,
                operation: Some(Operation::ChatCompletions),
            })
        }),
        Err(_) => {
            let rt = tokio::runtime::Runtime::new().map_err(|e| CanonicalError {
                code: "runtime_error".to_string(),
                message: e.to_string(),
                retryable: false,
                operation: Some(Operation::ChatCompletions),
            })?;
            rt.block_on(fut)
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use axum::{routing::post, Json, Router};
    use serde_json::json;
    use tokio::net::TcpListener;

    fn chat_req(model: &str) -> CanonicalRequestEnvelope {
        use crate::spearlet::execution::ai::ir::{
            ChatCompletionsPayload, ChatMessage, RoutingHints,
        };

        CanonicalRequestEnvelope {
            version: 1,
            request_id: "r1".to_string(),
            operation: Operation::ChatCompletions,
            meta: HashMap::new(),
            routing: RoutingHints::default(),
            requirements: Default::default(),
            timeout_ms: None,
            payload: Payload::ChatCompletions(ChatCompletionsPayload {
                model: model.to_string(),
                messages: vec![ChatMessage {
                    role: "user".to_string(),
                    content: Value::String("hi".to_string()),
                    tool_call_id: None,
                    tool_calls: None,
                    name: None,
                }],
                tools: vec![],
                params: HashMap::new(),
            }),
            extra: HashMap::new(),
        }
    }

    async fn start_mock_chat() -> String {
        let app = Router::new().route(
            "/api/chat",
            post(|Json(_v): Json<Value>| async move {
                Json(json!({
                    "model": "llama3",
                    "message": {"role": "assistant", "content": "ok"},
                    "done": true
                }))
            }),
        );

        let listener = TcpListener::bind("127.0.0.1:0").await.unwrap();
        let addr = listener.local_addr().unwrap();
        tokio::spawn(async move {
            axum::serve(listener, app).await.unwrap();
        });
        format!("http://{}", addr)
    }

    #[test]
    fn test_missing_model_is_error() {
        let adapter = OllamaChatBackendAdapter::new("o", "http://127.0.0.1:11434", None);
        let mut req = chat_req("");
        if let Payload::ChatCompletions(p) = &mut req.payload {
            p.model = "".to_string();
        }
        let err = adapter.invoke(&req).unwrap_err();
        assert_eq!(err.code, "invalid_request");
    }

    #[tokio::test(flavor = "multi_thread", worker_threads = 2)]
    async fn test_invoke_returns_openai_like_shape() {
        let base = start_mock_chat().await;
        let adapter = OllamaChatBackendAdapter::new("o", base, Some("llama3".to_string()));
        let req = chat_req("ignored");
        let resp = tokio::task::spawn_blocking(move || adapter.invoke(&req))
            .await
            .unwrap()
            .unwrap();
        let v = match resp.result {
            ResultPayload::Payload(v) => v,
            _ => panic!("unexpected"),
        };
        assert!(v.get("choices").is_some());
        assert_eq!(
            v["choices"][0]["message"]["content"],
            Value::String("ok".to_string())
        );
    }
}
