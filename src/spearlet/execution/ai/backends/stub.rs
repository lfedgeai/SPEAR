use serde_json::json;

use crate::spearlet::execution::ai::backends::BackendAdapter;
use crate::spearlet::execution::ai::ir::{
    CanonicalError, CanonicalRequestEnvelope, CanonicalResponseEnvelope, Operation, Payload,
    ResultPayload,
};

pub struct StubBackendAdapter {
    name: String,
}

impl StubBackendAdapter {
    pub fn new(name: impl Into<String>) -> Self {
        Self { name: name.into() }
    }
}

impl BackendAdapter for StubBackendAdapter {
    fn name(&self) -> &str {
        &self.name
    }

    fn invoke(
        &self,
        req: &CanonicalRequestEnvelope,
    ) -> Result<CanonicalResponseEnvelope, CanonicalError> {
        match &req.payload {
            Payload::ChatCompletions(p) => {
                let last_user = p
                    .messages
                    .iter()
                    .rev()
                    .find(|m| m.role.eq_ignore_ascii_case("user"))
                    .map(|m| m.content.clone())
                    .unwrap_or_default();
                let assistant_content = if last_user.is_empty() {
                    "stub chat completion".to_string()
                } else {
                    format!("stub chat completion: {}", last_user)
                };
                let response_json = json!({
                    "id": req.request_id,
                    "object": "chat.completion",
                    "created": chrono::Utc::now().timestamp(),
                    "model": p.model,
                    "choices": [
                        {
                            "index": 0,
                            "message": {
                                "role": "assistant",
                                "content": assistant_content,
                            },
                            "finish_reason": "stop"
                        }
                    ]
                });
                Ok(CanonicalResponseEnvelope {
                    version: 1,
                    request_id: req.request_id.clone(),
                    operation: Operation::ChatCompletions,
                    backend: self.name.clone(),
                    result: ResultPayload::Payload(response_json),
                    raw: None,
                })
            }
            _ => Err(CanonicalError {
                code: "unsupported_operation".to_string(),
                message: "stub backend only supports chat_completions".to_string(),
                retryable: false,
                operation: Some(req.operation.clone()),
            }),
        }
    }
}
