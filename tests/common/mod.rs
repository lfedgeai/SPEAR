use spear_next::spearlet::config::{AiBackendConfig, AppConfig, SpearletConfig};

#[allow(dead_code)]
pub struct ResolvedBackend {
    pub name: String,
    pub kind: String,
    pub base_url: String,
    pub api_key_env: String,
    pub api_key: String,
}

/// Return true only when live tests are explicitly enabled.
/// 仅在显式启用 live tests 时返回 true。
fn live_tests_enabled() -> bool {
    std::env::var("SPEAR_RUN_LIVE_TESTS")
        .ok()
        .map(|v| v == "1" || v.eq_ignore_ascii_case("true"))
        .unwrap_or(false)
}

/// Load the spearlet config used by integration live tests.
/// 加载集成 live tests 使用的 spearlet 配置。
fn load_spearlet_config() -> Option<SpearletConfig> {
    let path = std::env::var("SPEAR_TEST_CONFIG")
        .ok()
        .filter(|v| !v.trim().is_empty())
        .unwrap_or_else(|| format!("{}/config/spearlet/config.toml", env!("CARGO_MANIFEST_DIR")));

    let s = std::fs::read_to_string(&path).ok()?;
    let cfg: AppConfig = toml::from_str(&s).ok()?;
    Some(cfg.spearlet)
}

/// Resolve one backend from config plus its env-backed credential.
/// 从配置和 env 类型凭据中解析一个 backend。
fn resolve_backend(op: &str, transport: &str, kinds: &[&str]) -> Option<ResolvedBackend> {
    resolve_backends(op, transport, kinds).into_iter().next()
}

/// Return true when a backend matches the requested live-test routing filters.
/// 当 backend 命中 live-test 需要的路由过滤条件时返回 true。
fn matches_backend_filters(
    backend: &AiBackendConfig,
    op: &str,
    transport: &str,
    kinds: &[&str],
) -> bool {
    backend.ops.iter().any(|x| x == op)
        && backend.transports.iter().any(|x| x == transport)
        && kinds.iter().any(|k| *k == backend.kind)
}

/// Resolve one concrete backend entry plus its env-backed credential.
/// 解析单个 backend 条目及其对应的 env 类型凭据。
fn resolve_backend_entry(cfg: &SpearletConfig, backend: &AiBackendConfig) -> Option<ResolvedBackend> {
    let cred_name = backend.credential_ref.as_ref()?;
    let cred = cfg.ai.credentials.iter().find(|c| {
        c.name == *cred_name && c.kind == "env" && !c.api_key_env.trim().is_empty()
    })?;

    let env_name = cred.api_key_env.trim().to_string();
    let api_key = std::env::var(&env_name).ok()?.trim().to_string();
    if api_key.is_empty() {
        return None;
    }

    let base_url = backend.base_url.trim().to_string();
    if base_url.is_empty() {
        return None;
    }

    Some(ResolvedBackend {
        name: backend.name.clone(),
        kind: backend.kind.clone(),
        base_url,
        api_key_env: env_name,
        api_key,
    })
}

/// Resolve all backends from config plus their env-backed credentials.
/// 从配置和 env 类型凭据中解析所有匹配的 backend。
fn resolve_backends(op: &str, transport: &str, kinds: &[&str]) -> Vec<ResolvedBackend> {
    let Some(cfg) = load_spearlet_config() else {
        return Vec::new();
    };

    cfg.ai
        .backends
        .iter()
        .filter(|backend| matches_backend_filters(backend, op, transport, kinds))
        .filter_map(|backend| resolve_backend_entry(&cfg, backend))
        .collect()
}

#[allow(dead_code)]
pub fn resolve_live_chat_backend() -> Option<ResolvedBackend> {
    if !live_tests_enabled() {
        return None;
    }
    resolve_backend("chat_completions", "http", &["chat_http"])
}

#[allow(dead_code)]
pub fn resolve_realtime_asr_backend() -> Option<ResolvedBackend> {
    if !live_tests_enabled() {
        return None;
    }
    resolve_backend(
        "speech_to_text",
        "websocket",
        &["rt_asr_openai_ws"],
    )
}

/// Resolve a Qwen realtime ASR backend from the shared live-test config.
/// 从共享 live-test 配置中解析 Qwen realtime ASR backend。
#[allow(dead_code)]
pub fn resolve_qwen_realtime_asr_backend() -> Option<ResolvedBackend> {
    if !live_tests_enabled() {
        return None;
    }
    resolve_backend(
        "speech_to_text",
        "websocket",
        &["rt_asr_qwen_ws"],
    )
}

/// Resolve all configured realtime ASR backends that are ready for live tests.
/// 解析所有已就绪、可用于 live test 的 realtime ASR backend。
#[allow(dead_code)]
pub fn resolve_all_realtime_asr_backends() -> Vec<ResolvedBackend> {
    if !live_tests_enabled() {
        return Vec::new();
    }
    resolve_backends(
        "speech_to_text",
        "websocket",
        &[
            "rt_asr_openai_ws",
            "rt_asr_qwen_ws",
        ],
    )
}
