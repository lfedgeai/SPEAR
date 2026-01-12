use serde_json::json;

use crate::spearlet::execution::ai::backends::BackendAdapter;
use crate::spearlet::execution::ai::ir::{
    CanonicalError, CanonicalRequestEnvelope, CanonicalResponseEnvelope, Operation, Payload,
    ResultPayload,
};

pub struct StubBackendAdapter {
    name: String,
}

fn parse_first_two_i64(s: &str) -> Option<(i64, i64)> {
    let mut nums: Vec<i64> = Vec::with_capacity(2);
    let mut i = 0;
    let bytes = s.as_bytes();
    while i < bytes.len() && nums.len() < 2 {
        while i < bytes.len() && !(bytes[i] == b'-' || (bytes[i] >= b'0' && bytes[i] <= b'9')) {
            i += 1;
        }
        if i >= bytes.len() {
            break;
        }
        let start = i;
        i += 1;
        while i < bytes.len() && (bytes[i] >= b'0' && bytes[i] <= b'9') {
            i += 1;
        }
        if let Ok(v) = s[start..i].parse::<i64>() {
            nums.push(v);
        }
    }
    if nums.len() == 2 {
        Some((nums[0], nums[1]))
    } else {
        None
    }
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
                let has_tool_result = p
                    .messages
                    .iter()
                    .rev()
                    .any(|m| m.role.eq_ignore_ascii_case("tool"));

                let last_user = p
                    .messages
                    .iter()
                    .rev()
                    .find(|m| m.role.eq_ignore_ascii_case("user"))
                    .and_then(|m| m.content.as_str())
                    .unwrap_or("");

                let should_tool_call = !p.tools.is_empty()
                    && !has_tool_result
                    && parse_first_two_i64(last_user).is_some();

                if should_tool_call {
                    let name = p
                        .tools
                        .get(0)
                        .and_then(|t| t.get("function"))
                        .and_then(|f| f.get("name"))
                        .and_then(|n| n.as_str())
                        .unwrap_or("tool_call");

                    let arguments = parse_first_two_i64(last_user)
                        .map(|(a, b)| json!({"a": a, "b": b}).to_string())
                        .unwrap_or_else(|| "{}".to_string());
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
                                    "content": null,
                                    "tool_calls": [
                                        {
                                            "id": "call_stub_1",
                                            "type": "function",
                                            "function": {
                                                "name": name,
                                                "arguments": arguments
                                            }
                                        }
                                    ]
                                },
                                "finish_reason": "tool_calls"
                            }
                        ]
                    });
                    return Ok(CanonicalResponseEnvelope {
                        version: 1,
                        request_id: req.request_id.clone(),
                        operation: Operation::ChatCompletions,
                        backend: self.name.clone(),
                        result: ResultPayload::Payload(response_json),
                        raw: None,
                    });
                }

                let assistant_content = if has_tool_result {
                    "stub chat completion after tool".to_string()
                } else if last_user.is_empty() {
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
