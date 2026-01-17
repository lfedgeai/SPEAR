use crate::spearlet::execution::ai::backends::ollama_chat::OllamaChatBackendAdapter;
use crate::spearlet::execution::ai::backends::openai_chat_completion::OpenAIChatCompletionBackendAdapter;
use crate::spearlet::execution::ai::backends::openai_realtime_ws::OpenAIRealtimeWsBackendAdapter;
use crate::spearlet::execution::ai::backends::stub::StubBackendAdapter;
use crate::spearlet::execution::ai::ir::Operation;
use crate::spearlet::execution::ai::router::capabilities::Capabilities;
use crate::spearlet::execution::ai::router::policy::SelectionPolicy;
use crate::spearlet::execution::ai::router::registry::{BackendInstance, BackendRegistry};
use std::collections::HashMap;
use std::sync::Arc;

pub(super) fn build_registry_from_runtime_config(
    runtime_config: &super::super::runtime::RuntimeConfig,
) -> (BackendRegistry, SelectionPolicy) {
    let mut policy = SelectionPolicy::WeightedRandom;
    let mut instances: Vec<BackendInstance> = Vec::new();

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

            let api_key_env = if backend_requires_api_key(&b.kind) {
                let env_name = match resolve_backend_api_key_env(b, &cred_index) {
                    Ok(v) => v,
                    Err(msg) => {
                        tracing::warn!(backend = %b.name, kind = %b.kind, "invalid backend configuration: {msg}");
                        continue;
                    }
                };
                if !runtime_config.global_environment.contains_key(&env_name) {
                    tracing::warn!(backend = %b.name, kind = %b.kind, env = %env_name, "missing required env var");
                    continue;
                }
                env_name
            } else {
                String::new()
            };

            let adapter: Arc<dyn crate::spearlet::execution::ai::backends::BackendAdapter> = match b
                .kind
                .as_str()
            {
                "openai_chat_completion" => {
                    let api_key = runtime_config
                        .global_environment
                        .get(&api_key_env)
                        .cloned()
                        .unwrap_or_default();
                    if api_key.trim().is_empty() {
                        tracing::warn!(backend = %b.name, kind = %b.kind, env = %api_key_env, "missing required env var");
                        continue;
                    }
                    Arc::new(OpenAIChatCompletionBackendAdapter::new(
                        b.name.clone(),
                        b.base_url.clone(),
                        api_key,
                    ))
                }
                "openai_realtime_ws" => Arc::new(OpenAIRealtimeWsBackendAdapter::new(
                    b.name.clone(),
                    b.base_url.clone(),
                    api_key_env.clone(),
                )),
                "ollama_chat" => Arc::new(OllamaChatBackendAdapter::new(
                    b.name.clone(),
                    b.base_url.clone(),
                    b.model.clone(),
                )),
                _ => continue,
            };

            instances.push(BackendInstance {
                name: b.name.clone(),
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

    if instances.is_empty() {
        let stub = Arc::new(StubBackendAdapter::new("stub"));
        instances.push(BackendInstance {
            name: "stub".to_string(),
            model: None,
            weight: 100,
            priority: 0,
            capabilities: Capabilities {
                ops: vec![Operation::ChatCompletions],
                features: vec![
                    "supports_tools".to_string(),
                    "supports_json_schema".to_string(),
                    "supports_stream".to_string(),
                ],
                transports: vec!["in_process".to_string()],
            },
            adapter: stub,
        });
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

fn backend_requires_api_key(kind: &str) -> bool {
    matches!(kind, "openai_chat_completion" | "openai_realtime_ws")
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
