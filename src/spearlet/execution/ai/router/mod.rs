pub mod capabilities;
pub mod grpc_filter_stream;
pub mod policy;
pub mod registry;

use std::collections::HashMap;
use std::sync::Arc;

use crate::spearlet::execution::ai::ir::{CanonicalError, CanonicalRequestEnvelope};
use crate::spearlet::execution::ai::{backends::openai_chat_completion::OpenAIChatCompletionBackendAdapter, ir::Operation};
use crate::spearlet::execution::ai::router::policy::SelectionPolicy;
use crate::spearlet::execution::ai::router::registry::{BackendInstance, BackendRegistry, Hosting};
use crate::spearlet::local_models::{global_managed_backends, ManagedBackendRegistry};
use crate::spearlet::execution::ai::backends::KIND_OPENAI_CHAT_COMPLETION;
use parking_lot::RwLock;
use rand::Rng;
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
    grpc_filter_stream: Option<Arc<grpc_filter_stream::RouterFilterStreamHub>>,
    managed_backends: ManagedBackendRegistry,
    managed_cache: Arc<RwLock<ManagedBackendCache>>,
}

struct ManagedBackendCache {
    revision: u64,
    instances: Arc<Vec<BackendInstance>>,
}

impl Router {
    pub fn new(registry: BackendRegistry, policy: SelectionPolicy) -> Self {
        Self {
            registry,
            policy,
            grpc_filter_stream: None,
            managed_backends: global_managed_backends(),
            managed_cache: Arc::new(RwLock::new(ManagedBackendCache {
                revision: 0,
                instances: Arc::new(Vec::new()),
            })),
        }
    }

    pub fn new_with_filter(
        registry: BackendRegistry,
        policy: SelectionPolicy,
        grpc_filter_stream: Option<Arc<grpc_filter_stream::RouterFilterStreamHub>>,
    ) -> Self {
        Self {
            registry,
            policy,
            grpc_filter_stream,
            managed_backends: global_managed_backends(),
            managed_cache: Arc::new(RwLock::new(ManagedBackendCache {
                revision: 0,
                instances: Arc::new(Vec::new()),
            })),
        }
    }

    fn managed_instances(&self) -> Arc<Vec<BackendInstance>> {
        let rev = self.managed_backends.revision();
        {
            let cache = self.managed_cache.read();
            if cache.revision == rev {
                return cache.instances.clone();
            }
        }

        let mut out: Vec<BackendInstance> = Vec::new();
        for b in self.managed_backends.list().into_iter() {
            if let Some(inst) = managed_backend_info_to_instance(b) {
                out.push(inst);
            }
        }
        let instances = Arc::new(out);
        let mut cache = self.managed_cache.write();
        cache.revision = rev;
        cache.instances = instances.clone();
        instances
    }

    pub fn route<'a>(
        &'a self,
        req: &CanonicalRequestEnvelope,
    ) -> Result<BackendInstance, CanonicalError> {
        let managed = self.managed_instances();
        let mut instances: Vec<&BackendInstance> = Vec::new();
        let mut seen_names: std::collections::HashSet<String> = std::collections::HashSet::new();
        for inst in self.registry.instances().iter() {
            seen_names.insert(inst.name.clone());
            instances.push(inst);
        }
        for inst in managed.iter() {
            if seen_names.insert(inst.name.clone()) {
                instances.push(inst);
            }
        }

        let mut candidates: Vec<&BackendInstance> = instances
            .iter()
            .copied()
            .filter(|inst| inst.capabilities.supports_operation(&req.operation))
            .filter(|inst| {
                req.requirements
                    .required_features
                    .iter()
                    .all(|f| inst.capabilities.has_feature(f))
            })
            .filter(|inst| {
                req.requirements
                    .required_transports
                    .iter()
                    .all(|t| inst.capabilities.transports.iter().any(|x| x == t))
            })
            .collect();

        if let Some(name) = req.routing.backend.as_ref() {
            candidates.retain(|c| c.name == *name);
        }

        if !req.routing.allowlist.is_empty() {
            candidates.retain(|c| req.routing.allowlist.iter().any(|x| x == &c.name));
        }

        if !req.routing.denylist.is_empty() {
            candidates.retain(|c| !req.routing.denylist.iter().any(|x| x == &c.name));
        }

        let mut weight_overrides: HashMap<String, u32> = HashMap::new();
        if let Some(hub) = self.grpc_filter_stream.as_ref() {
            let decision_budget_ms = req
                .timeout_ms
                .map(|t| t.min(hub.config.decision_timeout_ms))
                .unwrap_or(hub.config.decision_timeout_ms);
            match hub.try_filter_candidates_blocking(req, &mut candidates, decision_budget_ms) {
                Ok((resp, _trace)) => {
                    if let Some(final_action) = resp.final_action.as_ref() {
                        if final_action.reject_request {
                            let code = if final_action.reject_code.trim().is_empty() {
                                "router_filter_rejected".to_string()
                            } else {
                                final_action.reject_code.clone()
                            };
                            let message = if final_action.reject_message.trim().is_empty() {
                                "router filter rejected".to_string()
                            } else {
                                final_action.reject_message.clone()
                            };
                            return Err(CanonicalError {
                                code,
                                message,
                                retryable: false,
                                operation: Some(req.operation.clone()),
                            });
                        }
                        if !final_action.force_backend.trim().is_empty() {
                            let forced = final_action.force_backend.trim();
                            candidates.retain(|c| c.name == forced);
                        }
                    }

                    let mut decision_by_name: HashMap<
                        &str,
                        &crate::proto::spearlet::CandidateDecision,
                    > = HashMap::new();
                    for d in resp.decisions.iter() {
                        decision_by_name.insert(d.name.as_str(), d);
                    }

                    candidates.retain(|c| {
                        let Some(d) = decision_by_name.get(c.name.as_str()) else {
                            return true;
                        };
                        d.action != crate::proto::spearlet::DecisionAction::Drop as i32
                    });

                    for (name, d) in decision_by_name {
                        if let Some(w) = d.weight_override {
                            weight_overrides.insert(name.to_string(), w.min(10_000));
                        }
                    }
                }
                Err(e) => {
                    if !hub.config.fail_open {
                        return Err(e);
                    }
                }
            }
        }

        let requested_model_val = requested_model(req)
            .map(|s| s.trim())
            .filter(|s| !s.is_empty());
        if let Some(model) = requested_model_val {
            if candidates.iter().any(|c| c.model.is_some()) {
                let exact: Vec<&BackendInstance> = candidates
                    .iter()
                    .copied()
                    .filter(|c| c.model.as_deref() == Some(model))
                    .collect();
                if exact.is_empty() {
                    let mut available_models: Vec<String> =
                        candidates.iter().filter_map(|c| c.model.clone()).collect();
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
                candidates = exact;
            }
        }

        if candidates.is_empty() {
            let mut supporting: Vec<String> = Vec::new();
            for inst in instances.iter().copied() {
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
        let selected = match self.policy {
            SelectionPolicy::WeightedRandom if !weight_overrides.is_empty() => {
                select_weighted_random_overrides(req, candidates, &weight_overrides)?
            }
            _ => self.policy.select(req, candidates)?.clone(),
        };
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

fn select_weighted_random_overrides(
    req: &CanonicalRequestEnvelope,
    candidates: Vec<&BackendInstance>,
    weight_overrides: &HashMap<String, u32>,
) -> Result<BackendInstance, CanonicalError> {
    let total: u32 = candidates
        .iter()
        .map(|c| {
            weight_overrides
                .get(&c.name)
                .copied()
                .unwrap_or(c.weight)
                .max(1)
        })
        .sum();
    let mut rng = rand::thread_rng();
    let mut pick = rng.gen_range(0..total);
    for c in candidates {
        let w = weight_overrides
            .get(&c.name)
            .copied()
            .unwrap_or(c.weight)
            .max(1);
        if pick < w {
            return Ok(c.clone());
        }
        pick -= w;
    }
    Err(CanonicalError {
        code: "no_candidate_backend".to_string(),
        message: "no candidate backend".to_string(),
        retryable: false,
        operation: Some(req.operation.clone()),
    })
}

fn parse_operation(s: &str) -> Option<Operation> {
    match s {
        "chat_completions" => Some(Operation::ChatCompletions),
        "embeddings" => Some(Operation::Embeddings),
        "image_generation" => Some(Operation::ImageGeneration),
        "speech_to_text" => Some(Operation::SpeechToText),
        "text_to_speech" => Some(Operation::TextToSpeech),
        "realtime_voice" => Some(Operation::RealtimeVoice),
        _ => None,
    }
}

fn managed_backend_info_to_instance(b: crate::proto::sms::BackendInfo) -> Option<BackendInstance> {
    if b.name.trim().is_empty() {
        return None;
    }
    let ops = b
        .operations
        .iter()
        .filter_map(|s| parse_operation(s.as_str()))
        .collect::<Vec<_>>();
    if ops.is_empty() {
        return None;
    }
    let hosting = match b.hosting {
        2 => Hosting::Local,
        1 => Hosting::Remote,
        _ => Hosting::Unknown,
    };
    let adapter: Arc<dyn crate::spearlet::execution::ai::backends::BackendAdapter> =
        match b.kind.as_str() {
            KIND_OPENAI_CHAT_COMPLETION => {
                let mut a = OpenAIChatCompletionBackendAdapter::new(
                    b.name.clone(),
                    b.base_url.clone(),
                    None,
                );
                if !b.model.trim().is_empty() {
                    a = a.with_fixed_model(b.model.clone());
                }
                Arc::new(a)
            }
            _ => return None,
        };
    Some(BackendInstance {
        name: b.name,
        kind: b.kind,
        base_url: b.base_url,
        hosting,
        model: None,
        weight: b.weight,
        priority: b.priority,
        capabilities: crate::spearlet::execution::ai::router::capabilities::Capabilities {
            ops,
            features: b.features,
            transports: b.transports,
        },
        adapter,
    })
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::spearlet::config::RouterGrpcFilterStreamConfig;
    use crate::spearlet::execution::ai::backends::stub::StubBackendAdapter;
    use crate::spearlet::execution::ai::ir::{
        ChatCompletionsPayload, ChatMessage, Operation, Payload, RoutingHints,
    };
    use crate::spearlet::execution::ai::router::capabilities::Capabilities;
    use crate::spearlet::execution::ai::router::grpc_filter_stream::RouterFilterStreamHub;
    use crate::spearlet::execution::ai::router::policy::SelectionPolicy;
    use crate::spearlet::execution::ai::router::registry::Hosting;
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
            kind: "openai_chat_completion".to_string(),
            base_url: "https://api.openai.com/v1".to_string(),
            hosting: Hosting::Remote,
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
            kind: "ollama_chat".to_string(),
            base_url: "http://127.0.0.1:11434".to_string(),
            hosting: Hosting::Local,
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
            kind: "ollama_chat".to_string(),
            base_url: "http://127.0.0.1:11434".to_string(),
            hosting: Hosting::Local,
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

    #[test]
    fn test_route_fail_open_when_filter_unavailable() {
        let a = BackendInstance {
            name: "stub".to_string(),
            kind: "stub".to_string(),
            base_url: String::new(),
            hosting: Hosting::Local,
            model: Some("gpt-4o-mini".to_string()),
            weight: 100,
            priority: 0,
            capabilities: Capabilities {
                ops: vec![Operation::ChatCompletions],
                features: vec![],
                transports: vec!["http".to_string()],
            },
            adapter: Arc::new(StubBackendAdapter::new("stub")),
        };

        let hub = Arc::new(RouterFilterStreamHub::new(RouterGrpcFilterStreamConfig {
            enabled: true,
            fail_open: true,
            ..Default::default()
        }));

        let router = Router::new_with_filter(
            BackendRegistry::new(vec![a]),
            SelectionPolicy::WeightedRandom,
            Some(hub),
        );
        let req = chat_req("gpt-4o-mini");
        let inst = router.route(&req).unwrap();
        assert_eq!(inst.name, "stub");
    }

    #[test]
    fn test_route_fail_closed_when_filter_unavailable() {
        let a = BackendInstance {
            name: "stub".to_string(),
            kind: "stub".to_string(),
            base_url: String::new(),
            hosting: Hosting::Local,
            model: Some("gpt-4o-mini".to_string()),
            weight: 100,
            priority: 0,
            capabilities: Capabilities {
                ops: vec![Operation::ChatCompletions],
                features: vec![],
                transports: vec!["http".to_string()],
            },
            adapter: Arc::new(StubBackendAdapter::new("stub")),
        };

        let hub = Arc::new(RouterFilterStreamHub::new(RouterGrpcFilterStreamConfig {
            enabled: true,
            fail_open: false,
            ..Default::default()
        }));

        let router = Router::new_with_filter(
            BackendRegistry::new(vec![a]),
            SelectionPolicy::WeightedRandom,
            Some(hub),
        );
        let req = chat_req("gpt-4o-mini");
        let err = match router.route(&req) {
            Ok(_) => panic!("expected error"),
            Err(e) => e,
        };
        assert_eq!(err.code, "router_filter_unavailable");
    }
}
