use serde_json::{json, Value};
use std::collections::HashMap;
use std::time::Duration;

use crate::spearlet::execution::ai::backends::BackendAdapter;
use crate::spearlet::execution::ai::ir::{
    CanonicalError, CanonicalRequestEnvelope, CanonicalResponseEnvelope, Operation, Payload,
    ResultPayload,
};

pub struct OpenAICompatibleBackendAdapter {
    name: String,
    base_url: String,
    api_key_env: Option<String>,
    global_environment: HashMap<String, String>,
}

impl OpenAICompatibleBackendAdapter {
    pub fn new(
        name: impl Into<String>,
        base_url: impl Into<String>,
        api_key_env: Option<String>,
        global_environment: HashMap<String, String>,
    ) -> Self {
        Self {
            name: name.into(),
            base_url: base_url.into(),
            api_key_env,
            global_environment,
        }
    }

    fn get_api_key(&self) -> Result<String, CanonicalError> {
        let Some(env) = self.api_key_env.as_ref() else {
            return Err(CanonicalError {
                code: "invalid_configuration".to_string(),
                message: "missing api_key_env".to_string(),
                retryable: false,
                operation: None,
            });
        };
        self.global_environment
            .get(env)
            .cloned()
            .ok_or_else(|| CanonicalError {
                code: "invalid_configuration".to_string(),
                message: format!("missing env var: {}", env),
                retryable: false,
                operation: None,
            })
    }

    fn build_chat_completions_body(
        &self,
        req: &CanonicalRequestEnvelope,
    ) -> Result<Value, CanonicalError> {
        let Payload::ChatCompletions(p) = &req.payload else {
            return Err(CanonicalError {
                code: "payload_mismatch".to_string(),
                message: "expected chat_completions payload".to_string(),
                retryable: false,
                operation: Some(req.operation.clone()),
            });
        };

        if p.model.trim().is_empty() {
            return Err(CanonicalError {
                code: "invalid_request".to_string(),
                message: "missing model".to_string(),
                retryable: false,
                operation: Some(req.operation.clone()),
            });
        }

        if p.messages.is_empty() {
            return Err(CanonicalError {
                code: "invalid_request".to_string(),
                message: "missing messages".to_string(),
                retryable: false,
                operation: Some(req.operation.clone()),
            });
        }

        let mut body = json!({
            "model": p.model,
            "messages": p.messages.iter().map(|m| json!({"role": m.role, "content": m.content})).collect::<Vec<_>>(),
        });

        if !p.tools.is_empty() {
            body["tools"] = Value::Array(p.tools.clone());
        }

        if let Some(obj) = body.as_object_mut() {
            for (k, v) in p.params.iter() {
                if k == "model"
                    || k == "backend"
                    || k == "timeout_ms"
                    || k == "messages"
                    || k == "tools"
                {
                    continue;
                }
                obj.insert(k.clone(), v.clone());
            }
        }

        Ok(body)
    }

    fn join_url(&self, path: &str) -> String {
        let base = self.base_url.trim_end_matches('/');
        let p = path.trim_start_matches('/');
        format!("{}/{}", base, p)
    }

    fn extract_openai_error_message(json: &Value) -> Option<String> {
        let e = json.get("error")?;
        let msg = e.get("message").and_then(|v| v.as_str()).unwrap_or("");
        let ty = e.get("type").and_then(|v| v.as_str()).unwrap_or("");
        let code_owned = if let Some(s) = e.get("code").and_then(|v| v.as_str()) {
            s.to_string()
        } else if let Some(n) = e.get("code").and_then(|v| v.as_i64()) {
            n.to_string()
        } else {
            String::new()
        };

        let mut parts: Vec<String> = Vec::new();
        if !ty.is_empty() {
            parts.push(ty.to_string());
        }
        if !code_owned.is_empty() {
            parts.push(code_owned);
        }
        if !msg.is_empty() {
            parts.push(msg.to_string());
        }
        if parts.is_empty() {
            None
        } else {
            Some(parts.join(": "))
        }
    }
}

impl BackendAdapter for OpenAICompatibleBackendAdapter {
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
                message: "openai_compatible only supports chat_completions".to_string(),
                retryable: false,
                operation: Some(req.operation.clone()),
            });
        }

        let api_key = self.get_api_key()?;
        let body_json = self.build_chat_completions_body(req)?;
        let body_bytes = serde_json::to_vec(&body_json).map_err(|e| CanonicalError {
            code: "serialization".to_string(),
            message: e.to_string(),
            retryable: false,
            operation: Some(req.operation.clone()),
        })?;

        let url = if self.base_url.contains("/v1") {
            self.join_url("chat/completions")
        } else {
            self.join_url("v1/chat/completions")
        };

        let timeout = req.timeout_ms.map(Duration::from_millis);

        let rt = tokio::runtime::Runtime::new().map_err(|e| CanonicalError {
            code: "runtime_error".to_string(),
            message: e.to_string(),
            retryable: false,
            operation: Some(req.operation.clone()),
        })?;

        let (status, resp_body, _headers) = rt.block_on(async move {
            let client = reqwest::Client::new();
            let mut r = client
                .post(url)
                .header("content-type", "application/json")
                .header("authorization", format!("Bearer {}", api_key))
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
            let extra = Self::extract_openai_error_message(&parsed);
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

        Ok(CanonicalResponseEnvelope {
            version: 1,
            request_id: req.request_id.clone(),
            operation: req.operation.clone(),
            backend: self.name.clone(),
            result: ResultPayload::Payload(parsed),
            raw: Some(resp_body),
        })
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::spearlet::execution::ai::ir::{ChatCompletionsPayload, ChatMessage, RoutingHints};

    #[test]
    fn test_missing_api_key_env_is_error() {
        let adapter = OpenAICompatibleBackendAdapter::new(
            "openai",
            "https://api.openai.com/v1",
            Some("OPENAI_API_KEY".to_string()),
            HashMap::new(),
        );
        let req = CanonicalRequestEnvelope {
            version: 1,
            request_id: "r1".to_string(),
            operation: Operation::ChatCompletions,
            meta: HashMap::new(),
            routing: RoutingHints::default(),
            requirements: Default::default(),
            timeout_ms: None,
            payload: Payload::ChatCompletions(ChatCompletionsPayload {
                model: "gpt-test".to_string(),
                messages: vec![ChatMessage {
                    role: "user".to_string(),
                    content: "hi".to_string(),
                }],
                tools: vec![],
                params: HashMap::new(),
            }),
            extra: HashMap::new(),
        };
        let err = adapter.invoke(&req).unwrap_err();
        assert_eq!(err.code, "invalid_configuration");
    }

    #[test]
    fn test_join_url() {
        let adapter = OpenAICompatibleBackendAdapter::new(
            "openai",
            "https://api.openai.com/v1/",
            None,
            HashMap::new(),
        );
        assert_eq!(
            adapter.join_url("chat/completions"),
            "https://api.openai.com/v1/chat/completions"
        );
    }

    #[test]
    fn test_params_cannot_override_messages_or_tools() {
        let adapter = OpenAICompatibleBackendAdapter::new(
            "openai",
            "https://api.openai.com/v1/",
            None,
            HashMap::new(),
        );

        let mut params = HashMap::new();
        params.insert(
            "messages".to_string(),
            json!([{ "role": "user", "content": "override" }]),
        );
        params.insert(
            "tools".to_string(),
            json!([{ "type": "function", "function": {"name":"x"}}]),
        );

        let req = CanonicalRequestEnvelope {
            version: 1,
            request_id: "r1".to_string(),
            operation: Operation::ChatCompletions,
            meta: HashMap::new(),
            routing: RoutingHints::default(),
            requirements: Default::default(),
            timeout_ms: None,
            payload: Payload::ChatCompletions(ChatCompletionsPayload {
                model: "gpt-test".to_string(),
                messages: vec![ChatMessage {
                    role: "user".to_string(),
                    content: "original".to_string(),
                }],
                tools: vec![json!({"type":"function","function":{"name":"y"}})],
                params,
            }),
            extra: HashMap::new(),
        };

        let body = adapter.build_chat_completions_body(&req).unwrap();
        assert_eq!(
            body.get("messages").unwrap()[0]["content"],
            Value::String("original".to_string())
        );
        assert_eq!(body.get("tools").unwrap()[0]["function"]["name"], "y");
    }
}
