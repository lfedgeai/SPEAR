use crate::spearlet::execution::ai::backends::ollama_chat::OllamaChatBackendAdapter;
use crate::spearlet::execution::ai::backends::openai_chat_completion::OpenAIChatCompletionBackendAdapter;
use crate::spearlet::execution::ai::backends::openai_realtime_ws::OpenAIRealtimeWsBackendAdapter;
use crate::spearlet::execution::ai::backends::stub::StubBackendAdapter;
use crate::spearlet::execution::ai::backends::{
    KIND_OPENAI_CHAT_COMPLETION, KIND_OPENAI_REALTIME_WS, KIND_OLLAMA_CHAT, KIND_STUB,
};
use crate::spearlet::execution::ai::ir::Operation;
use crate::spearlet::execution::ai::router::capabilities::Capabilities;
use crate::spearlet::execution::ai::router::policy::SelectionPolicy;
use crate::spearlet::execution::ai::router::registry::{BackendInstance, BackendRegistry, Hosting};
use std::collections::{HashMap, HashSet};
use std::sync::Arc;

fn parse_hosting(s: &str) -> Hosting {
    let v = s.trim().to_ascii_lowercase();
    match v.as_str() {
        "local" => Hosting::Local,
        "remote" => Hosting::Remote,
        _ => Hosting::Unknown,
    }
}

fn resolve_hosting(override_value: Option<&str>) -> Hosting {
    if let Some(v) = override_value.map(|s| s.trim()).filter(|s| !s.is_empty()) {
        let parsed = parse_hosting(v);
        if parsed != Hosting::Unknown {
            return parsed;
        }
    }
    Hosting::Unknown
}

pub(crate) fn build_registry_from_runtime_config(
    runtime_config: &super::super::runtime::RuntimeConfig,
) -> (BackendRegistry, SelectionPolicy) {
    let mut policy = SelectionPolicy::WeightedRandom;
    let mut instances: Vec<BackendInstance> = Vec::new();
    let mut seen_names: HashSet<String> = HashSet::new();

    if let Some(cfg) = runtime_config.spearlet_config.as_ref() {
        let cred_index = build_credential_index(&cfg.llm);
        if let Some(p) = cfg.llm.default_policy.as_ref() {
            policy = match p.as_str() {
                "weighted_random" => SelectionPolicy::WeightedRandom,
                _ => SelectionPolicy::WeightedRandom,
            };
        }

        for b in cfg.llm.backends.iter() {
            let ops = b
                .ops
                .iter()
                .filter_map(|s| parse_operation(s))
                .collect::<Vec<_>>();
            if ops.is_empty() {
                continue;
            }

            let adapter: Arc<dyn crate::spearlet::execution::ai::backends::BackendAdapter> = match b
                .kind
                .as_str()
            {
                KIND_OPENAI_CHAT_COMPLETION => {
                    let api_key = match b.credential_ref.as_deref().map(|s| s.trim()) {
                        Some(r) if !r.is_empty() => {
                            let env_name = match resolve_backend_api_key_env(b, &cred_index) {
                                Ok(v) => v,
                                Err(msg) => {
                                    tracing::warn!(backend = %b.name, kind = %b.kind, "invalid backend configuration: {msg}");
                                    continue;
                                }
                            };
                            match runtime_config.global_environment.get(&env_name) {
                                Some(v) if !v.trim().is_empty() => Some(v.clone()),
                                _ => {
                                    tracing::warn!(backend = %b.name, kind = %b.kind, env = %env_name, "missing required env var");
                                    continue;
                                }
                            }
                        }
                        _ => None,
                    };
                    Arc::new(OpenAIChatCompletionBackendAdapter::new(
                        b.name.clone(),
                        b.base_url.clone(),
                        api_key,
                    ))
                }
                KIND_OPENAI_REALTIME_WS => {
                    let api_key_env = match b.credential_ref.as_deref().map(|s| s.trim()) {
                        Some(r) if !r.is_empty() => {
                            let env_name = match resolve_backend_api_key_env(b, &cred_index) {
                                Ok(v) => v,
                                Err(msg) => {
                                    tracing::warn!(backend = %b.name, kind = %b.kind, "invalid backend configuration: {msg}");
                                    continue;
                                }
                            };
                            match runtime_config.global_environment.get(&env_name) {
                                Some(v) if !v.trim().is_empty() => Some(env_name),
                                _ => {
                                    tracing::warn!(backend = %b.name, kind = %b.kind, env = %env_name, "missing required env var");
                                    continue;
                                }
                            }
                        }
                        _ => None,
                    };
                    Arc::new(OpenAIRealtimeWsBackendAdapter::new(
                        b.name.clone(),
                        b.base_url.clone(),
                        api_key_env,
                    ))
                }
                KIND_OLLAMA_CHAT => Arc::new(OllamaChatBackendAdapter::new(
                    b.name.clone(),
                    b.base_url.clone(),
                    b.model.clone(),
                )),
                KIND_STUB => Arc::new(StubBackendAdapter::new(&b.name)),
                _ => continue,
            };

            if !seen_names.insert(b.name.clone()) {
                tracing::warn!(backend = %b.name, kind = %b.kind, "duplicated backend name");
                continue;
            }
            instances.push(BackendInstance {
                name: b.name.clone(),
                kind: b.kind.clone(),
                base_url: b.base_url.clone(),
                hosting: resolve_hosting(b.hosting.as_deref()),
                model: b.model.clone(),
                weight: b.weight,
                priority: b.priority,
                capabilities: Capabilities {
                    ops,
                    features: b.features.clone(),
                    transports: b.transports.clone(),
                },
                adapter,
            });
        }
    }

    (BackendRegistry::new(instances), policy)
}

fn build_credential_index(
    cfg: &crate::spearlet::config::LlmConfig,
) -> HashMap<String, crate::spearlet::config::LlmCredentialConfig> {
    let mut out: HashMap<String, crate::spearlet::config::LlmCredentialConfig> = HashMap::new();
    for c in cfg.credentials.iter() {
        if c.name.trim().is_empty() {
            tracing::warn!("llm.credentials: missing name");
            continue;
        }
        if c.kind.as_str() != "env" {
            tracing::warn!(credential = %c.name, kind = %c.kind, "llm.credentials: unsupported kind");
            continue;
        }
        if c.api_key_env.trim().is_empty() {
            tracing::warn!(credential = %c.name, "llm.credentials: missing api_key_env");
            continue;
        }
        if out.contains_key(&c.name) {
            tracing::warn!(credential = %c.name, "llm.credentials: duplicated name");
            continue;
        }
        out.insert(c.name.clone(), c.clone());
    }
    out
}

fn resolve_backend_api_key_env(
    backend: &crate::spearlet::config::LlmBackendConfig,
    cred_index: &HashMap<String, crate::spearlet::config::LlmCredentialConfig>,
) -> Result<String, String> {
    let r = backend
        .credential_ref
        .as_ref()
        .ok_or_else(|| "missing credential_ref".to_string())?;
    if r.trim().is_empty() {
        return Err("credential_ref is empty".to_string());
    }
    let c = cred_index
        .get(r)
        .ok_or_else(|| format!("credential_ref not found: {r}"))?;
    Ok(c.api_key_env.clone())
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
