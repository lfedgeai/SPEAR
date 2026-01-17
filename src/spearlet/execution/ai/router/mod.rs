pub mod capabilities;
pub mod policy;
pub mod registry;

use crate::spearlet::execution::ai::ir::{CanonicalError, CanonicalRequestEnvelope};
use crate::spearlet::execution::ai::router::policy::SelectionPolicy;
use crate::spearlet::execution::ai::router::registry::{BackendInstance, BackendRegistry};
use tracing::debug;

fn requested_model(req: &CanonicalRequestEnvelope) -> Option<&str> {
    match &req.payload {
        crate::spearlet::execution::ai::ir::Payload::ChatCompletions(p) => Some(p.model.as_str()),
        crate::spearlet::execution::ai::ir::Payload::Embeddings(p) => p.model.as_deref(),
        crate::spearlet::execution::ai::ir::Payload::ImageGeneration(p) => p.model.as_deref(),
        crate::spearlet::execution::ai::ir::Payload::SpeechToText(p) => p.model.as_deref(),
        crate::spearlet::execution::ai::ir::Payload::TextToSpeech(p) => p.model.as_deref(),
        crate::spearlet::execution::ai::ir::Payload::RealtimeVoice(p) => p.model.as_deref(),
    }
}

#[derive(Clone)]
pub struct Router {
    registry: BackendRegistry,
    policy: SelectionPolicy,
}

impl Router {
    pub fn new(registry: BackendRegistry, policy: SelectionPolicy) -> Self {
        Self { registry, policy }
    }

    pub fn route<'a>(
        &'a self,
        req: &CanonicalRequestEnvelope,
    ) -> Result<&'a BackendInstance, CanonicalError> {
        let mut candidates = self.registry.candidates(req);

        if let Some(name) = req.routing.backend.as_ref() {
            candidates.retain(|c| c.name == *name);
        }

        if !req.routing.allowlist.is_empty() {
            candidates.retain(|c| req.routing.allowlist.iter().any(|x| x == &c.name));
        }

        if !req.routing.denylist.is_empty() {
            candidates.retain(|c| !req.routing.denylist.iter().any(|x| x == &c.name));
        }

        if let Some(model) = requested_model(req)
            .map(|s| s.trim())
            .filter(|s| !s.is_empty())
        {
            if candidates.iter().any(|c| c.model.is_some()) {
                candidates.retain(|c| c.model.as_deref() == Some(model));
                if candidates.is_empty() {
                    let mut available_models: Vec<String> = self
                        .registry
                        .instances()
                        .iter()
                        .filter(|inst| inst.capabilities.supports_operation(&req.operation))
                        .filter_map(|inst| inst.model.clone())
                        .collect();
                    available_models.sort();
                    available_models.dedup();
                    let msg = format!(
                        "no candidate backend for model={:?}: available_models={:?}",
                        model, available_models
                    );
                    return Err(CanonicalError {
                        code: "no_candidate_backend".to_string(),
                        message: msg,
                        retryable: false,
                        operation: Some(req.operation.clone()),
                    });
                }
            }
        }

        if candidates.is_empty() {
            let mut supporting: Vec<String> = Vec::new();
            for inst in self.registry.instances().iter() {
                if !inst.capabilities.supports_operation(&req.operation) {
                    continue;
                }

                let mut missing_features: Vec<&str> = Vec::new();
                for f in req.requirements.required_features.iter() {
                    if !inst.capabilities.has_feature(f) {
                        missing_features.push(f);
                    }
                }
                let mut missing_transports: Vec<&str> = Vec::new();
                for t in req.requirements.required_transports.iter() {
                    if !inst.capabilities.transports.iter().any(|x| x == t) {
                        missing_transports.push(t);
                    }
                }

                supporting.push(format!(
                    "{}(missing_features={:?}, missing_transports={:?}, features={:?}, transports={:?})",
                    inst.name, missing_features, missing_transports, inst.capabilities.features, inst.capabilities.transports
                ));
            }

            let msg = format!(
                "no candidate backend: op={:?} required_features={:?} required_transports={:?} routing_backend={:?} allowlist={:?} denylist={:?} backends={:?}",
                req.operation,
                req.requirements.required_features,
                req.requirements.required_transports,
                req.routing.backend,
                req.routing.allowlist,
                req.routing.denylist,
                supporting,
            );
            return Err(CanonicalError {
                code: "no_candidate_backend".to_string(),
                message: msg,
                retryable: false,
                operation: Some(req.operation.clone()),
            });
        }

        let candidate_count = candidates.len();
        let candidate_names: Vec<&str> =
            candidates.iter().take(8).map(|c| c.name.as_str()).collect();
        let selected = self.policy.select(req, candidates)?;
        debug!(
            op = ?req.operation,
            model = requested_model(req),
            routing_backend = req.routing.backend.as_deref(),
            allowlist_len = req.routing.allowlist.len(),
            denylist_len = req.routing.denylist.len(),
            candidate_count,
            candidate_names = ?candidate_names,
            selected_backend = %selected.name,
            selected_model = ?selected.model,
            "router selected backend"
        );
        Ok(selected)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::spearlet::execution::ai::backends::stub::StubBackendAdapter;
    use crate::spearlet::execution::ai::ir::{
        ChatCompletionsPayload, ChatMessage, Operation, Payload, RoutingHints,
    };
    use crate::spearlet::execution::ai::router::capabilities::Capabilities;
    use crate::spearlet::execution::ai::router::policy::SelectionPolicy;
    use serde_json::Value;
    use std::collections::HashMap;
    use std::sync::Arc;

    fn chat_req(model: &str) -> CanonicalRequestEnvelope {
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

    #[test]
    fn test_route_prefers_model_bound_backend() {
        let a = BackendInstance {
            name: "openai".to_string(),
            model: Some("gpt-4o-mini".to_string()),
            weight: 100,
            priority: 0,
            capabilities: Capabilities {
                ops: vec![Operation::ChatCompletions],
                features: vec![],
                transports: vec!["http".to_string()],
            },
            adapter: Arc::new(StubBackendAdapter::new("openai")),
        };
        let b = BackendInstance {
            name: "ollama".to_string(),
            model: Some("gemma3:1b".to_string()),
            weight: 100,
            priority: 0,
            capabilities: Capabilities {
                ops: vec![Operation::ChatCompletions],
                features: vec![],
                transports: vec!["http".to_string()],
            },
            adapter: Arc::new(StubBackendAdapter::new("ollama")),
        };
        let router = Router::new(
            BackendRegistry::new(vec![a, b]),
            SelectionPolicy::WeightedRandom,
        );
        let req = chat_req("gemma3:1b");
        let inst = router.route(&req).unwrap();
        assert_eq!(inst.name, "ollama");
    }

    #[test]
    fn test_route_errors_on_unknown_model_when_model_bound_exists() {
        let a = BackendInstance {
            name: "ollama".to_string(),
            model: Some("gemma3:1b".to_string()),
            weight: 100,
            priority: 0,
            capabilities: Capabilities {
                ops: vec![Operation::ChatCompletions],
                features: vec![],
                transports: vec!["http".to_string()],
            },
            adapter: Arc::new(StubBackendAdapter::new("ollama")),
        };
        let router = Router::new(
            BackendRegistry::new(vec![a]),
            SelectionPolicy::WeightedRandom,
        );
        let req = chat_req("gpt-4o-mini");
        let err = match router.route(&req) {
            Ok(_) => panic!("expected error"),
            Err(e) => e,
        };
        assert_eq!(err.code, "no_candidate_backend");
    }
}
