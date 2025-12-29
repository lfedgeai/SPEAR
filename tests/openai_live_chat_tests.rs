use spear_next::spearlet::execution::ai::backends::openai_compatible::OpenAICompatibleBackendAdapter;
use spear_next::spearlet::execution::ai::backends::BackendAdapter;
use spear_next::spearlet::execution::ai::ir::{
    CanonicalRequestEnvelope, ChatCompletionsPayload, ChatMessage, Operation, Payload, RoutingHints,
};
use std::collections::HashMap;

#[test]
fn test_openai_live_chat_completion() {
    let api_key = match std::env::var("OPENAI_API_KEY") {
        Ok(v) if !v.trim().is_empty() => v,
        _ => {
            eprintln!("skipped: missing OPENAI_API_KEY");
            return;
        }
    };
    let base_url = match std::env::var("OPENAI_API_BASE") {
        Ok(v) if !v.trim().is_empty() => v,
        _ => {
            eprintln!("skipped: missing OPENAI_API_BASE");
            return;
        }
    };

    let model = std::env::var("OPENAI_MODEL").unwrap_or_else(|_| "gpt-4o-mini".to_string());

    let mut env = HashMap::new();
    env.insert("OPENAI_API_KEY".to_string(), api_key);

    let adapter = OpenAICompatibleBackendAdapter::new(
        "openai-live",
        base_url,
        Some("OPENAI_API_KEY".to_string()),
        env,
    );

    let req = CanonicalRequestEnvelope {
        version: 1,
        request_id: "live_test_1".to_string(),
        operation: Operation::ChatCompletions,
        meta: HashMap::new(),
        routing: RoutingHints::default(),
        requirements: Default::default(),
        timeout_ms: Some(30_000),
        payload: Payload::ChatCompletions(ChatCompletionsPayload {
            model,
            messages: vec![ChatMessage {
                role: "user".to_string(),
                content: "Reply with exactly: pong".to_string(),
            }],
            tools: vec![],
            params: {
                let mut p = HashMap::new();
                p.insert("temperature".to_string(), serde_json::json!(0));
                p.insert("max_completion_tokens".to_string(), serde_json::json!(16));
                p
            },
        }),
        extra: HashMap::new(),
    };

    let resp = adapter.invoke(&req).expect("live chat completion failed");
    let v = match resp.result {
        spear_next::spearlet::execution::ai::ir::ResultPayload::Payload(v) => v,
        spear_next::spearlet::execution::ai::ir::ResultPayload::Error(e) => {
            panic!("unexpected error payload: {}", e.message)
        }
    };

    if let Ok(s) = serde_json::to_string_pretty(&v) {
        println!("openai response json:\n{}", s);
    }

    let content = v
        .get("choices")
        .and_then(|c| c.get(0))
        .and_then(|c0| c0.get("message"))
        .and_then(|m| m.get("content"))
        .and_then(|s| s.as_str())
        .unwrap_or("");

    println!("openai assistant content: {}", content);

    assert!(
        content.trim().eq_ignore_ascii_case("pong"),
        "unexpected content: {}",
        content
    );
}
