//! Shared local-provider resolution helpers.
//! 共享的本地 provider 解析辅助。

use crate::ai_backend_types::{CanonicalBackendKind, CanonicalBackendProvider};

/// Canonical local provider kind recognized by node-side controllers.
/// 节点侧控制器识别的规范本地 provider 类型。
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum LocalProviderKind {
    LlamaCpp,
    Vllm,
    Unsupported(String),
}

/// Resolve a local provider kind from multiple candidate strings.
/// 从多个候选字符串解析本地 provider 类型。
pub fn resolve_local_provider_kind<'a, I>(candidates: I) -> LocalProviderKind
where
    I: IntoIterator<Item = &'a str>,
{
    let normalized = candidates
        .into_iter()
        .map(|value| value.trim().to_ascii_lowercase())
        .collect::<Vec<_>>();

    if normalized.iter().any(|value| is_llamacpp_family(value)) {
        return LocalProviderKind::LlamaCpp;
    }
    if normalized.iter().any(|value| is_vllm_family(value)) {
        return LocalProviderKind::Vllm;
    }

    LocalProviderKind::Unsupported(
        normalized
            .into_iter()
            .find(|value| !value.is_empty())
            .unwrap_or_else(|| "unspecified".to_string()),
    )
}

/// Resolve a local provider kind from assignment backend/spec fields.
/// 从 assignment 的 backend/spec 字段解析本地 provider 类型。
pub fn resolve_local_provider_kind_for_assignment(
    backend_kind: &str,
    spec_kind: &str,
    spec_provider: &str,
) -> LocalProviderKind {
    resolve_local_provider_kind([backend_kind, spec_kind, spec_provider])
}

fn is_llamacpp_family(value: &str) -> bool {
    CanonicalBackendKind::parse(value) == CanonicalBackendKind::LlamaCpp
        || CanonicalBackendProvider::parse(value) == CanonicalBackendProvider::LlamaCpp
}

fn is_vllm_family(value: &str) -> bool {
    matches!(
        CanonicalBackendKind::parse(value),
        CanonicalBackendKind::Vllm | CanonicalBackendKind::VllmOpenAiCompatible
    ) || CanonicalBackendProvider::parse(value) == CanonicalBackendProvider::Vllm
}

#[cfg(test)]
mod tests {
    use super::{
        resolve_local_provider_kind, resolve_local_provider_kind_for_assignment, LocalProviderKind,
    };
    use crate::ai_backend_types::{BACKEND_PROVIDER_LLAMACPP, BACKEND_PROVIDER_VLLM};

    #[test]
    fn resolves_llamacpp_from_canonical_value() {
        assert_eq!(
            resolve_local_provider_kind([BACKEND_PROVIDER_LLAMACPP]),
            LocalProviderKind::LlamaCpp
        );
    }

    #[test]
    fn resolves_vllm_from_canonical_value() {
        assert_eq!(
            resolve_local_provider_kind([BACKEND_PROVIDER_VLLM]),
            LocalProviderKind::Vllm
        );
    }

    #[test]
    fn reports_first_non_empty_unknown_value() {
        assert_eq!(
            resolve_local_provider_kind(["", "mystery-provider"]),
            LocalProviderKind::Unsupported("mystery-provider".to_string())
        );
    }

    #[test]
    fn resolves_assignment_provider_kind_from_backend_and_spec_fields() {
        assert_eq!(
            resolve_local_provider_kind_for_assignment(
                "custom",
                BACKEND_PROVIDER_VLLM,
                BACKEND_PROVIDER_VLLM
            ),
            LocalProviderKind::Vllm
        );
        assert_eq!(
            resolve_local_provider_kind_for_assignment(BACKEND_PROVIDER_LLAMACPP, "", ""),
            LocalProviderKind::LlamaCpp
        );
    }
}
