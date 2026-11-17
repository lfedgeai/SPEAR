/**
 * Canonical backend family registry shared by web-admin editor flows and tests.
 * web-admin 编辑流与测试共享的规范 backend family 注册表。
 */

import type { AiBackendHosting } from '@/api/ai-backends'

export const AI_BACKEND_FAMILIES = {
  CHAT_HTTP: 'chat_http',
  RT_ASR_OPENAI_WS: 'rt_asr_openai_ws',
  RT_ASR_QWEN_WS: 'rt_asr_qwen_ws',
  OLLAMA_CHAT: 'ollama_chat',
  LLAMACPP: 'llamacpp',
  VLLM: 'vllm',
} as const

export const AI_BACKEND_PROVIDERS = {
  OPENAI: 'openai',
  QWEN: 'qwen',
  OLLAMA: 'ollama',
  LLAMACPP: 'llamacpp',
  VLLM: 'vllm',
} as const

export type AiBackendFamilyId =
  (typeof AI_BACKEND_FAMILIES)[keyof typeof AI_BACKEND_FAMILIES]

export type BackendKindDefaults = {
  operations: string[]
  transports: string
  features: string
  base_url: string
  model: string
}

export const REMOTE_PROVIDER_OPTIONS: string[] = [
  AI_BACKEND_PROVIDERS.OPENAI,
  AI_BACKEND_PROVIDERS.QWEN,
  AI_BACKEND_PROVIDERS.OLLAMA,
]

export const LOCAL_PROVIDER_OPTIONS: string[] = [
  AI_BACKEND_PROVIDERS.LLAMACPP,
  AI_BACKEND_PROVIDERS.VLLM,
]

// Keep the structured-provider boundary in one place so form serialization and
// tests do not drift apart. / 将 structured-provider 边界集中到一个位置，
// 避免表单序列化与测试各自漂移。
export const STRUCTURED_REMOTE_PROVIDER_OPTIONS: ReadonlySet<string> = new Set([
  AI_BACKEND_PROVIDERS.OPENAI,
  AI_BACKEND_PROVIDERS.OLLAMA,
])

export const BACKEND_FAMILY_OPTIONS: Record<AiBackendHosting, Record<string, AiBackendFamilyId[]>> = {
  remote: {
    [AI_BACKEND_PROVIDERS.OPENAI]: [
      AI_BACKEND_FAMILIES.CHAT_HTTP,
      AI_BACKEND_FAMILIES.RT_ASR_OPENAI_WS,
    ],
    [AI_BACKEND_PROVIDERS.QWEN]: [
      AI_BACKEND_FAMILIES.CHAT_HTTP,
      AI_BACKEND_FAMILIES.RT_ASR_QWEN_WS,
    ],
    [AI_BACKEND_PROVIDERS.OLLAMA]: [AI_BACKEND_FAMILIES.OLLAMA_CHAT],
  },
  local: {
    [AI_BACKEND_PROVIDERS.LLAMACPP]: [AI_BACKEND_FAMILIES.LLAMACPP],
    [AI_BACKEND_PROVIDERS.VLLM]: [AI_BACKEND_FAMILIES.VLLM],
  },
}

export const BACKEND_FAMILY_BASE_DEFAULTS: Record<
  AiBackendFamilyId,
  Omit<BackendKindDefaults, 'base_url' | 'model'>
> = {
  [AI_BACKEND_FAMILIES.CHAT_HTTP]: {
    operations: ['chat_completions'],
    transports: 'http',
    features: '',
  },
  [AI_BACKEND_FAMILIES.RT_ASR_OPENAI_WS]: {
    operations: ['speech_to_text'],
    transports: 'websocket',
    features: '',
  },
  [AI_BACKEND_FAMILIES.RT_ASR_QWEN_WS]: {
    operations: ['speech_to_text'],
    transports: 'websocket',
    features: '',
  },
  [AI_BACKEND_FAMILIES.OLLAMA_CHAT]: {
    operations: ['chat_completions'],
    transports: 'http',
    features: '',
  },
  [AI_BACKEND_FAMILIES.LLAMACPP]: {
    operations: ['chat_completions'],
    transports: 'http',
    features: '',
  },
  [AI_BACKEND_FAMILIES.VLLM]: {
    operations: ['chat_completions'],
    transports: 'http',
    features: '',
  },
}

export const BACKEND_FAMILY_PROVIDER_DEFAULTS: Record<
  AiBackendHosting,
  Record<string, Partial<Record<AiBackendFamilyId, Pick<BackendKindDefaults, 'base_url' | 'model'>>>>
> = {
  remote: {
    [AI_BACKEND_PROVIDERS.OPENAI]: {
      [AI_BACKEND_FAMILIES.CHAT_HTTP]: {
        base_url: 'https://api.openai.com/v1',
        model: '',
      },
      [AI_BACKEND_FAMILIES.RT_ASR_OPENAI_WS]: {
        base_url: 'https://api.openai.com/v1',
        model: 'gpt-4o-mini-transcribe',
      },
    },
    [AI_BACKEND_PROVIDERS.QWEN]: {
      [AI_BACKEND_FAMILIES.CHAT_HTTP]: {
        base_url: 'https://dashscope.aliyuncs.com/compatible-mode/v1',
        model: 'qwen3.7-plus',
      },
      [AI_BACKEND_FAMILIES.RT_ASR_QWEN_WS]: {
        base_url: 'https://maas.qianwenaiapi.com/compatible-mode/v1',
        model: 'qwen3-asr-flash-realtime',
      },
    },
    [AI_BACKEND_PROVIDERS.OLLAMA]: {
      [AI_BACKEND_FAMILIES.OLLAMA_CHAT]: {
        base_url: 'http://127.0.0.1:11434',
        model: '',
      },
    },
  },
  local: {
    [AI_BACKEND_PROVIDERS.LLAMACPP]: {
      [AI_BACKEND_FAMILIES.LLAMACPP]: {
        base_url: '',
        model: '',
      },
    },
    [AI_BACKEND_PROVIDERS.VLLM]: {
      [AI_BACKEND_FAMILIES.VLLM]: {
        base_url: '',
        model: '',
      },
    },
  },
}
