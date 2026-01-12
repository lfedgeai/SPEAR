use std::collections::HashMap;

use serde_json::Value;

use crate::spearlet::execution::ai::ir::{
    CanonicalRequestEnvelope, ChatCompletionsPayload, Operation, Payload, Requirements,
    RoutingHints,
};
use crate::spearlet::execution::host_api::ChatSessionSnapshot;

pub fn normalize_cchat_session(snapshot: &ChatSessionSnapshot) -> CanonicalRequestEnvelope {
    let model = snapshot
        .params
        .get("model")
        .and_then(|v| v.as_str())
        .unwrap_or("stub-model")
        .to_string();

    let mut requirements = Requirements::default();
    if !snapshot.tools.is_empty() {
        requirements
            .required_features
            .push("supports_tools".to_string());
    }

    if snapshot
        .params
        .get("response_format")
        .and_then(|v| v.get("type"))
        .and_then(|v| v.as_str())
        == Some("json_schema")
    {
        requirements
            .required_features
            .push("supports_json_schema".to_string());
    }

    let tools = snapshot
        .tools
        .iter()
        .filter_map(|(_, json)| serde_json::from_str::<Value>(json).ok())
        .collect::<Vec<_>>();

    let messages = snapshot.messages.clone();

    let mut routing = RoutingHints::default();
    if let Some(b) = snapshot
        .params
        .get("backend")
        .and_then(|v| v.as_str())
        .map(|s| s.to_string())
    {
        routing.backend = Some(b);
    }

    let mut meta = HashMap::new();
    meta.insert("source".to_string(), "cchat".to_string());

    CanonicalRequestEnvelope {
        version: 1,
        request_id: format!("chatcmpl_{}", snapshot.fd),
        operation: Operation::ChatCompletions,
        meta,
        routing,
        requirements,
        timeout_ms: snapshot.params.get("timeout_ms").and_then(|v| v.as_u64()),
        payload: Payload::ChatCompletions(ChatCompletionsPayload {
            model,
            messages,
            tools,
            params: snapshot.params.clone(),
        }),
        extra: HashMap::new(),
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::spearlet::execution::ai::ir::ChatMessage;

    #[test]
    fn test_normalize_minimal() {
        let mut params = HashMap::new();
        params.insert("model".to_string(), Value::String("gpt-test".to_string()));

        let snapshot = ChatSessionSnapshot {
            fd: 1000,
            messages: vec![ChatMessage {
                role: "user".to_string(),
                content: Value::String("hi".to_string()),
                tool_call_id: None,
                tool_calls: None,
                name: None,
            }],
            tools: vec![],
            params,
        };

        let req = normalize_cchat_session(&snapshot);
        assert_eq!(req.version, 1);
        assert_eq!(req.operation, Operation::ChatCompletions);
        match req.payload {
            Payload::ChatCompletions(p) => {
                assert_eq!(p.model, "gpt-test");
                assert_eq!(p.messages.len(), 1);
            }
            _ => panic!("unexpected payload"),
        }
    }

    #[test]
    fn test_require_tools_feature() {
        let snapshot = ChatSessionSnapshot {
            fd: 1000,
            messages: vec![],
            tools: vec![(0, "{}".to_string())],
            params: HashMap::new(),
        };
        let req = normalize_cchat_session(&snapshot);
        assert!(req
            .requirements
            .required_features
            .iter()
            .any(|f| f == "supports_tools"));
    }
}
