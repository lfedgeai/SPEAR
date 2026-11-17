//! Shared typed helpers for AI backend provider/kind/hosting values.
//! AI backend 的 provider/kind/hosting 共享强类型辅助。

use crate::proto::sms::{
    AiBackendDesiredState, AiBackendHosting, AiBackendManagementMode, BackendOrigin,
};

/// Canonical backend family ids shared across runtime, config, and tests.
/// runtime、配置与测试共享的规范 backend family id。
pub const BACKEND_FAMILY_CHAT_HTTP: &str = "chat_http";
pub const BACKEND_FAMILY_RT_ASR_OPENAI_WS: &str = "rt_asr_openai_ws";
pub const BACKEND_FAMILY_RT_ASR_QWEN_WS: &str = "rt_asr_qwen_ws";
pub const BACKEND_FAMILY_OLLAMA_CHAT: &str = "ollama_chat";
pub const BACKEND_FAMILY_LLAMACPP: &str = "llamacpp";
pub const BACKEND_FAMILY_VLLM: &str = "vllm";
pub const BACKEND_FAMILY_VLLM_OPENAI_COMPATIBLE: &str = "vllm_openai_compatible";
pub const BACKEND_FAMILY_STUB: &str = "stub";

/// Canonical backend provider ids shared across runtime, config, and tests.
/// runtime、配置与测试共享的规范 backend provider id。
pub const BACKEND_PROVIDER_OPENAI: &str = "openai";
pub const BACKEND_PROVIDER_OLLAMA: &str = "ollama";
pub const BACKEND_PROVIDER_QWEN: &str = "qwen";
pub const BACKEND_PROVIDER_LLAMACPP: &str = "llamacpp";
pub const BACKEND_PROVIDER_VLLM: &str = "vllm";
pub const BACKEND_PROVIDER_INTERNAL: &str = "internal";

/// Canonical backend hosting kind used by internal code paths.
/// 内部代码路径使用的规范 backend hosting 类型。
#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub enum CanonicalBackendHosting {
    /// Node-local managed runtime / 节点本地托管运行时
    Local,
    /// Remote endpoint adapter / 远端 endpoint 适配器
    Remote,
    /// Unknown or unsupported value / 未知或不支持的取值
    Unknown(String),
}

impl CanonicalBackendHosting {
    /// Parse hosting from a raw string boundary value.
    /// 从边界层原始字符串解析 hosting。
    pub fn parse(value: &str) -> Self {
        match value.trim().to_ascii_lowercase().as_str() {
            "local" => Self::Local,
            "remote" => Self::Remote,
            other => Self::Unknown(other.to_string()),
        }
    }

    /// Return the canonical string form for stable serialization.
    /// 返回稳定序列化使用的规范字符串形式。
    pub fn as_str(&self) -> &str {
        match self {
            Self::Local => "local",
            Self::Remote => "remote",
            Self::Unknown(value) => value.as_str(),
        }
    }

    /// Convert to protobuf hosting enum.
    /// 转换为 protobuf hosting 枚举。
    pub fn to_proto_enum(&self) -> AiBackendHosting {
        match self {
            Self::Local => AiBackendHosting::Local,
            Self::Remote => AiBackendHosting::Remote,
            Self::Unknown(_) => AiBackendHosting::Unspecified,
        }
    }

    /// Parse from protobuf hosting enum value.
    /// 从 protobuf hosting 枚举值解析。
    pub fn from_proto_i32(value: i32) -> Self {
        match AiBackendHosting::try_from(value).unwrap_or(AiBackendHosting::Unspecified) {
            AiBackendHosting::Local => Self::Local,
            AiBackendHosting::Remote => Self::Remote,
            AiBackendHosting::Unspecified => Self::Unknown("unspecified".to_string()),
        }
    }
}

/// Canonical backend provider used by internal orchestration code.
/// 内部编排代码使用的规范 backend provider。
#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub enum CanonicalBackendProvider {
    /// OpenAI or OpenAI-compatible provider family / OpenAI 或 OpenAI-compatible provider 家族
    OpenAi,
    /// Ollama provider family / Ollama provider 家族
    Ollama,
    /// Qwen (DashScope / Model Studio) provider family / Qwen（百炼）provider 家族
    Qwen,
    /// Local llama.cpp managed runtime / 本地 llama.cpp 托管运行时
    LlamaCpp,
    /// Local vLLM family / 本地 vLLM 家族
    Vllm,
    /// Internal testing or stub provider / 内部测试或 stub provider
    Internal,
    /// Unknown or unsupported value / 未知或不支持的取值
    Unknown(String),
}

impl CanonicalBackendProvider {
    /// Parse provider from a raw string boundary value.
    /// 从边界层原始字符串解析 provider。
    pub fn parse(value: &str) -> Self {
        match value.trim().to_ascii_lowercase().as_str() {
            BACKEND_PROVIDER_OPENAI => Self::OpenAi,
            BACKEND_PROVIDER_OLLAMA => Self::Ollama,
            BACKEND_PROVIDER_QWEN => Self::Qwen,
            BACKEND_PROVIDER_LLAMACPP => Self::LlamaCpp,
            BACKEND_PROVIDER_VLLM => Self::Vllm,
            BACKEND_PROVIDER_INTERNAL => Self::Internal,
            other => Self::Unknown(other.to_string()),
        }
    }

    /// Return the canonical string form for stable serialization.
    /// 返回稳定序列化使用的规范字符串形式。
    pub fn as_str(&self) -> &str {
        match self {
            Self::OpenAi => BACKEND_PROVIDER_OPENAI,
            Self::Ollama => BACKEND_PROVIDER_OLLAMA,
            Self::Qwen => BACKEND_PROVIDER_QWEN,
            Self::LlamaCpp => BACKEND_PROVIDER_LLAMACPP,
            Self::Vllm => BACKEND_PROVIDER_VLLM,
            Self::Internal => BACKEND_PROVIDER_INTERNAL,
            Self::Unknown(value) => value.as_str(),
        }
    }
}

/// Canonical backend kind used to centralize adapter-family rules.
/// 用于集中承载 adapter-family 规则的规范 backend kind。
#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub enum CanonicalBackendKind {
    /// Provider-neutral chat-completions HTTP+JSON adapter family.
    /// Provider-neutral chat-completions HTTP+JSON 适配器家族。
    ChatHttp,
    /// OpenAI-style realtime speech-to-text websocket adapter family.
    /// OpenAI 风格实时语音识别 websocket 适配器家族。
    RtAsrOpenAiWs,
    /// Qwen realtime speech-to-text websocket adapter family.
    /// Qwen 实时语音识别 websocket 适配器家族。
    RtAsrQwenWs,
    /// Ollama chat backend / Ollama chat backend
    OllamaChat,
    /// Local llama.cpp backend / 本地 llama.cpp backend
    LlamaCpp,
    /// Local vLLM backend / 本地 vLLM backend
    Vllm,
    /// Local vLLM OpenAI-compatible backend / 本地 vLLM OpenAI-compatible backend
    VllmOpenAiCompatible,
    /// Internal stub backend / 内部 stub backend
    Stub,
    /// Unknown or unsupported value / 未知或不支持的取值
    Unknown(String),
}

impl CanonicalBackendKind {
    /// Parse a backend kind from a raw string boundary value.
    /// 从边界层原始字符串解析 backend kind。
    pub fn parse(value: &str) -> Self {
        match value.trim().to_ascii_lowercase().as_str() {
            BACKEND_FAMILY_CHAT_HTTP => Self::ChatHttp,
            BACKEND_FAMILY_RT_ASR_OPENAI_WS => Self::RtAsrOpenAiWs,
            BACKEND_FAMILY_RT_ASR_QWEN_WS => Self::RtAsrQwenWs,
            BACKEND_FAMILY_OLLAMA_CHAT => Self::OllamaChat,
            BACKEND_FAMILY_LLAMACPP => Self::LlamaCpp,
            BACKEND_FAMILY_VLLM => Self::Vllm,
            BACKEND_FAMILY_VLLM_OPENAI_COMPATIBLE => Self::VllmOpenAiCompatible,
            BACKEND_FAMILY_STUB => Self::Stub,
            other => Self::Unknown(other.to_string()),
        }
    }

    /// Return the canonical string form for stable serialization.
    /// 返回稳定序列化使用的规范字符串形式。
    pub fn as_str(&self) -> &str {
        match self {
            Self::ChatHttp => BACKEND_FAMILY_CHAT_HTTP,
            Self::RtAsrOpenAiWs => BACKEND_FAMILY_RT_ASR_OPENAI_WS,
            Self::RtAsrQwenWs => BACKEND_FAMILY_RT_ASR_QWEN_WS,
            Self::OllamaChat => BACKEND_FAMILY_OLLAMA_CHAT,
            Self::LlamaCpp => BACKEND_FAMILY_LLAMACPP,
            Self::Vllm => BACKEND_FAMILY_VLLM,
            Self::VllmOpenAiCompatible => BACKEND_FAMILY_VLLM_OPENAI_COMPATIBLE,
            Self::Stub => BACKEND_FAMILY_STUB,
            Self::Unknown(value) => value.as_str(),
        }
    }

    /// Return the default provider for one backend kind when that mapping is unambiguous.
    /// 当映射无歧义时，返回一个 backend kind 的默认 provider。
    pub fn default_provider(&self) -> Option<CanonicalBackendProvider> {
        match self {
            Self::ChatHttp => None,
            Self::RtAsrOpenAiWs => Some(CanonicalBackendProvider::OpenAi),
            Self::RtAsrQwenWs => Some(CanonicalBackendProvider::Qwen),
            Self::OllamaChat => Some(CanonicalBackendProvider::Ollama),
            Self::LlamaCpp => Some(CanonicalBackendProvider::LlamaCpp),
            Self::Vllm | Self::VllmOpenAiCompatible => Some(CanonicalBackendProvider::Vllm),
            Self::Stub => Some(CanonicalBackendProvider::Internal),
            Self::Unknown(_) => None,
        }
    }

    /// Check whether one provider is allowed for this backend kind.
    /// 检查该 backend kind 是否允许指定 provider。
    pub fn supports_provider(&self, provider: &CanonicalBackendProvider) -> bool {
        match self {
            Self::ChatHttp => matches!(
                provider,
                CanonicalBackendProvider::OpenAi | CanonicalBackendProvider::Qwen
            ),
            Self::RtAsrOpenAiWs => *provider == CanonicalBackendProvider::OpenAi,
            Self::RtAsrQwenWs => *provider == CanonicalBackendProvider::Qwen,
            Self::OllamaChat => *provider == CanonicalBackendProvider::Ollama,
            Self::LlamaCpp => *provider == CanonicalBackendProvider::LlamaCpp,
            Self::Vllm | Self::VllmOpenAiCompatible => *provider == CanonicalBackendProvider::Vllm,
            Self::Stub => *provider == CanonicalBackendProvider::Internal,
            Self::Unknown(_) => !matches!(provider, CanonicalBackendProvider::Unknown(_)),
        }
    }

    /// Check whether this kind needs an explicit provider because the kind spans multiple vendors.
    /// 检查该 kind 是否必须显式声明 provider，因为该 kind 跨多个厂商。
    pub fn requires_explicit_provider(&self) -> bool {
        match self {
            Self::ChatHttp => true,
            _ => false,
        }
    }

    /// Return the allowed hosting family for one backend kind.
    /// 返回一个 backend kind 允许的 hosting 家族。
    pub fn allowed_hosting(&self) -> AllowedHosting {
        match self {
            Self::LlamaCpp | Self::Vllm | Self::VllmOpenAiCompatible => AllowedHosting::LocalOnly,
            Self::ChatHttp | Self::RtAsrOpenAiWs => {
                AllowedHosting::RemoteOnly
            }
            Self::RtAsrQwenWs => AllowedHosting::RemoteOnly,
            Self::OllamaChat => AllowedHosting::Any,
            Self::Stub | Self::Unknown(_) => AllowedHosting::Any,
        }
    }

    /// Check whether this kind supports local-model preflight.
    /// 检查该 kind 是否支持 local-model preflight。
    pub fn supports_local_model_preflight(&self) -> bool {
        match self {
            Self::LlamaCpp => true,
            _ => false,
        }
    }

    /// Check whether this kind/provider pair supports remote preflight.
    /// 检查该 kind/provider 组合是否支持 remote preflight。
    pub fn supports_remote_preflight_for(&self, provider: &CanonicalBackendProvider) -> bool {
        match self {
            Self::ChatHttp => matches!(
                provider,
                CanonicalBackendProvider::OpenAi | CanonicalBackendProvider::Qwen
            ),
            Self::RtAsrOpenAiWs => *provider == CanonicalBackendProvider::OpenAi,
            Self::RtAsrQwenWs => *provider == CanonicalBackendProvider::Qwen,
            Self::OllamaChat => *provider == CanonicalBackendProvider::Ollama,
            _ => false,
        }
    }
}

/// Allowed hosting family for one backend kind.
/// 单个 backend kind 允许的 hosting 家族。
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum AllowedHosting {
    /// Only local hosting is valid / 仅允许 local hosting
    LocalOnly,
    /// Only remote hosting is valid / 仅允许 remote hosting
    RemoteOnly,
    /// Both hosting kinds are acceptable / 两种 hosting 都可接受
    Any,
}

/// Canonical backend origin value used across config/admin boundaries.
/// 跨 config/admin 边界复用的规范 backend origin 取值。
#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub enum CanonicalBackendOrigin {
    /// Originated from SMS control plane / 来源于 SMS 控制面
    Sms,
    /// Originated from static config / 来源于静态配置
    StaticConfig,
    /// Originated from local controller / 来源于本地控制器
    LocalController,
    /// Unknown or unsupported value / 未知或不支持的取值
    Unknown(String),
}

impl CanonicalBackendOrigin {
    /// Parse origin from an optional raw string value.
    /// 从可选原始字符串解析 origin。
    pub fn parse_optional(value: Option<&str>) -> Self {
        match value.map(|inner| inner.trim().to_ascii_lowercase()) {
            Some(inner) if inner == "sms" => Self::Sms,
            Some(inner) if inner == "static_config" => Self::StaticConfig,
            Some(inner) if inner == "local_controller" => Self::LocalController,
            Some(inner) => Self::Unknown(inner),
            None => Self::StaticConfig,
        }
    }

    /// Return the canonical string form for stable serialization.
    /// 返回稳定序列化使用的规范字符串形式。
    pub fn as_str(&self) -> &str {
        match self {
            Self::Sms => "sms",
            Self::StaticConfig => "static_config",
            Self::LocalController => "local_controller",
            Self::Unknown(value) => value.as_str(),
        }
    }

    /// Parse from protobuf origin enum value.
    /// 从 protobuf origin 枚举值解析。
    pub fn from_proto_i32(value: i32) -> Self {
        match BackendOrigin::try_from(value).ok() {
            Some(BackendOrigin::Sms) => Self::Sms,
            Some(BackendOrigin::StaticConfig) => Self::StaticConfig,
            Some(BackendOrigin::LocalController) => Self::LocalController,
            _ => Self::Unknown("unspecified".to_string()),
        }
    }

    /// Convert to protobuf origin enum.
    /// 转换为 protobuf origin 枚举。
    pub fn to_proto_enum(&self) -> BackendOrigin {
        match self {
            Self::Sms => BackendOrigin::Sms,
            Self::StaticConfig => BackendOrigin::StaticConfig,
            Self::LocalController => BackendOrigin::LocalController,
            Self::Unknown(_) => BackendOrigin::StaticConfig,
        }
    }
}

/// Canonical desired-state value used by admin input mapping.
/// 管理端输入映射复用的规范 desired-state 取值。
#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub enum CanonicalBackendDesiredState {
    /// Enabled desired state / 启用态
    Enabled,
    /// Disabled desired state / 禁用态
    Disabled,
    /// Unknown or unsupported value / 未知或不支持的取值
    Unknown(String),
}

impl CanonicalBackendDesiredState {
    /// Parse desired-state from a raw string value.
    /// 从原始字符串解析 desired-state。
    pub fn parse(value: &str) -> Self {
        match value.trim().to_ascii_lowercase().as_str() {
            "enabled" => Self::Enabled,
            "disabled" => Self::Disabled,
            other => Self::Unknown(other.to_string()),
        }
    }

    /// Parse from protobuf desired-state enum value.
    /// 从 protobuf desired-state 枚举值解析。
    pub fn from_proto_i32(value: i32) -> Self {
        match AiBackendDesiredState::try_from(value).unwrap_or(AiBackendDesiredState::Unspecified) {
            AiBackendDesiredState::Enabled => Self::Enabled,
            AiBackendDesiredState::Disabled => Self::Disabled,
            AiBackendDesiredState::Unspecified => Self::Unknown("unspecified".to_string()),
        }
    }

    /// Convert to protobuf desired-state enum.
    /// 转换为 protobuf desired-state 枚举。
    pub fn to_proto_enum(&self) -> AiBackendDesiredState {
        match self {
            Self::Enabled => AiBackendDesiredState::Enabled,
            Self::Disabled => AiBackendDesiredState::Disabled,
            Self::Unknown(_) => AiBackendDesiredState::Unspecified,
        }
    }
}

/// Canonical management-mode value used by admin input mapping.
/// 管理端输入映射复用的规范 management-mode 取值。
#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub enum CanonicalBackendManagementMode {
    /// SMS-managed remote mode / SMS 管理的远端模式
    SmsRemote,
    /// SMS-managed local mode / SMS 管理的本地模式
    SmsLocal,
    /// Unknown or unsupported value / 未知或不支持的取值
    Unknown(String),
}

impl CanonicalBackendManagementMode {
    /// Parse management-mode from a raw string value.
    /// 从原始字符串解析 management-mode。
    pub fn parse(value: &str) -> Self {
        match value.trim().to_ascii_lowercase().as_str() {
            "sms_remote" | "remote" => Self::SmsRemote,
            "sms_local" | "local" => Self::SmsLocal,
            other => Self::Unknown(other.to_string()),
        }
    }

    /// Parse from protobuf management-mode enum value.
    /// 从 protobuf management-mode 枚举值解析。
    pub fn from_proto_i32(value: i32) -> Self {
        match AiBackendManagementMode::try_from(value)
            .unwrap_or(AiBackendManagementMode::Unspecified)
        {
            AiBackendManagementMode::SmsRemote => Self::SmsRemote,
            AiBackendManagementMode::SmsLocal => Self::SmsLocal,
            AiBackendManagementMode::Unspecified => Self::Unknown("unspecified".to_string()),
        }
    }

    /// Convert to protobuf management-mode enum.
    /// 转换为 protobuf management-mode 枚举。
    pub fn to_proto_enum(&self) -> AiBackendManagementMode {
        match self {
            Self::SmsRemote => AiBackendManagementMode::SmsRemote,
            Self::SmsLocal => AiBackendManagementMode::SmsLocal,
            Self::Unknown(_) => AiBackendManagementMode::Unspecified,
        }
    }

    /// Derive default management-mode from hosting.
    /// 基于 hosting 推断默认 management-mode。
    pub fn default_for_hosting(hosting: &CanonicalBackendHosting) -> Self {
        match hosting {
            CanonicalBackendHosting::Remote => Self::SmsRemote,
            CanonicalBackendHosting::Local => Self::SmsLocal,
            CanonicalBackendHosting::Unknown(_) => Self::Unknown("unspecified".to_string()),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::{
        AllowedHosting, CanonicalBackendDesiredState, CanonicalBackendHosting, CanonicalBackendKind,
        CanonicalBackendManagementMode, CanonicalBackendOrigin, CanonicalBackendProvider,
        BACKEND_FAMILY_LLAMACPP, BACKEND_FAMILY_OLLAMA_CHAT, BACKEND_FAMILY_RT_ASR_OPENAI_WS,
    };

    #[test]
    fn canonical_backend_kind_exposes_default_provider_and_hosting_family() {
        let kind = CanonicalBackendKind::parse(BACKEND_FAMILY_RT_ASR_OPENAI_WS);
        assert_eq!(
            kind.default_provider(),
            Some(CanonicalBackendProvider::OpenAi)
        );
        assert_eq!(kind.allowed_hosting(), AllowedHosting::RemoteOnly);
    }

    #[test]
    fn canonical_ollama_kind_allows_both_local_and_remote_hosting() {
        let kind = CanonicalBackendKind::parse(BACKEND_FAMILY_OLLAMA_CHAT);
        assert_eq!(
            kind.default_provider(),
            Some(CanonicalBackendProvider::Ollama)
        );
        assert_eq!(kind.allowed_hosting(), AllowedHosting::Any);
    }

    #[test]
    fn canonical_backend_kind_parses_canonical_llamacpp_value() {
        let kind = CanonicalBackendKind::parse(BACKEND_FAMILY_LLAMACPP);
        assert_eq!(kind.as_str(), BACKEND_FAMILY_LLAMACPP);
        assert!(kind.supports_local_model_preflight());
    }

    #[test]
    fn chat_http_kind_accepts_openai_and_qwen() {
        let kind = CanonicalBackendKind::ChatHttp;
        assert!(kind.supports_provider(&CanonicalBackendProvider::OpenAi));
        assert!(kind.supports_provider(&CanonicalBackendProvider::Qwen));
        assert!(kind.requires_explicit_provider());
        assert_eq!(kind.default_provider(), None);
    }

    #[test]
    fn legacy_kind_aliases_are_rejected() {
        assert!(matches!(
            CanonicalBackendKind::parse("openai_chat_completion"),
            CanonicalBackendKind::Unknown(_)
        ));
        assert!(matches!(
            CanonicalBackendKind::parse("openai_realtime_ws"),
            CanonicalBackendKind::Unknown(_)
        ));
        assert!(matches!(
            CanonicalBackendKind::parse("qwen_asr_realtime_ws"),
            CanonicalBackendKind::Unknown(_)
        ));
        assert!(matches!(
            CanonicalBackendKind::parse("vllm-openai"),
            CanonicalBackendKind::Unknown(_)
        ));
    }

    #[test]
    fn canonical_backend_hosting_parses_unknown_values() {
        assert_eq!(CanonicalBackendHosting::parse("local").as_str(), "local");
        assert_eq!(CanonicalBackendHosting::parse("edge").as_str(), "edge");
    }

    #[test]
    fn canonical_backend_origin_parses_and_serializes() {
        let origin = CanonicalBackendOrigin::parse_optional(Some("local_controller"));
        assert_eq!(origin.as_str(), "local_controller");
    }

    #[test]
    fn canonical_desired_state_parses_and_handles_unknown() {
        assert_eq!(
            CanonicalBackendDesiredState::parse("enabled"),
            CanonicalBackendDesiredState::Enabled
        );
        assert!(matches!(
            CanonicalBackendDesiredState::parse("paused"),
            CanonicalBackendDesiredState::Unknown(_)
        ));
    }

    #[test]
    fn canonical_management_mode_defaults_from_hosting() {
        assert_eq!(
            CanonicalBackendManagementMode::default_for_hosting(&CanonicalBackendHosting::Remote),
            CanonicalBackendManagementMode::SmsRemote
        );
        assert_eq!(
            CanonicalBackendManagementMode::default_for_hosting(&CanonicalBackendHosting::Local),
            CanonicalBackendManagementMode::SmsLocal
        );
    }
}
