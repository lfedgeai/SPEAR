/**
 * Covers AI backend detail-page summary and provider-field resolution.
 * 覆盖 AI backend 详情页摘要与 provider 字段解析逻辑。
 */

import { describe, expect, it } from 'vitest'

import type { AiBackendSummary } from '@/api/ai-backends'
import {
  resolveBackendDetailSummary,
  resolveBackendProviderFields,
} from '@/features/ai-backends/AiBackendDetailPage.utils'
import { AI_BACKEND_FAMILIES, AI_BACKEND_PROVIDERS } from '@/features/ai-backends/backendFamilyRegistry'

describe('AiBackendDetailPage helpers', () => {
  it('prefers typed remote config over legacy spec fields for detail summary', () => {
    const backend: AiBackendSummary = {
      backend_id: 'backend-remote-1',
      display_name: 'Remote OpenAI',
      provider: AI_BACKEND_PROVIDERS.OPENAI,
      model: 'gpt-4o-mini',
      hosting: 'remote',
      backend_kind: AI_BACKEND_FAMILIES.CHAT_HTTP,
      desired_state: 'enabled',
      management_mode: 'sms_remote',
      credential_ref: null,
      spec: {
        name: 'backend-remote-1',
        kind: AI_BACKEND_FAMILIES.CHAT_HTTP,
        operations: ['legacy_operation'],
        features: ['legacy_feature'],
        transports: ['legacy_transport'],
        weight: 100,
        priority: 0,
        base_url: 'https://legacy.example.com/v1',
        provider: AI_BACKEND_PROVIDERS.OPENAI,
        model: 'gpt-4o-mini',
        credential_ref: 'legacy-credential',
        origin: 0,
        deployment_id: '',
      },
      remote: {
        provider_family: AI_BACKEND_PROVIDERS.OPENAI,
        config: {
          base_url: 'https://api.openai.com/v1',
          credential_ref: 'openai-prod',
          operations: ['chat_completions', 'embeddings'],
          features: ['stream'],
          transports: ['http'],
        },
      },
      metadata: {},
      labels: {},
      generation: 1,
      created_at_ms: 0,
      updated_at_ms: 0,
    }

    expect(resolveBackendDetailSummary(backend)).toEqual({
      operations: ['chat_completions', 'embeddings'],
      features: ['stream'],
      transports: ['http'],
      baseUrl: 'https://api.openai.com/v1',
      credentialRef: 'openai-prod',
    })
  })

  it('exposes typed local provider fields for known local providers', () => {
    const backend: AiBackendSummary = {
      backend_id: 'backend-local-1',
      display_name: 'Local llama.cpp',
      provider: AI_BACKEND_PROVIDERS.LLAMACPP,
      model: 'llama3.1',
      hosting: 'local',
      backend_kind: AI_BACKEND_FAMILIES.LLAMACPP,
      desired_state: 'enabled',
      management_mode: 'sms_local',
      credential_ref: null,
      spec: {
        name: 'backend-local-1',
        kind: AI_BACKEND_FAMILIES.LLAMACPP,
        operations: ['chat_completions'],
        features: [],
        transports: ['http'],
        weight: 100,
        priority: 0,
        base_url: 'http://127.0.0.1:8080/v1',
        provider: AI_BACKEND_PROVIDERS.LLAMACPP,
        model: 'llama3.1',
        credential_ref: null,
        origin: 0,
        deployment_id: '',
      },
      local: {
        provider_family: AI_BACKEND_PROVIDERS.LLAMACPP,
        config: {
          model_url: 'https://models.example.com/llama3.1.gguf',
          model_path: '/models/llama3.1.gguf',
          skip_download: false,
          download_timeout_s: 90,
          threads: 8,
          ctx_size: 4096,
        },
      },
      metadata: {},
      labels: {},
      generation: 1,
      created_at_ms: 0,
      updated_at_ms: 0,
    }

    expect(resolveBackendProviderFields(backend)).toEqual([
      { label: 'Model URL', value: 'https://models.example.com/llama3.1.gguf' },
      { label: 'Model Path', value: '/models/llama3.1.gguf' },
      { label: 'Skip download', value: 'false' },
      { label: 'Download timeout', value: '90' },
      { label: 'Threads', value: '8' },
      { label: 'Context size', value: '4096' },
    ])
  })

  it('keeps qwen remote detail summary on top-level spec fields', () => {
    const backend: AiBackendSummary = {
      backend_id: 'backend-qwen-1',
      display_name: 'Remote Qwen Realtime ASR',
      provider: AI_BACKEND_PROVIDERS.QWEN,
      model: 'qwen3-asr-flash-realtime',
      hosting: 'remote',
      backend_kind: AI_BACKEND_FAMILIES.RT_ASR_QWEN_WS,
      desired_state: 'enabled',
      management_mode: 'sms_remote',
      credential_ref: 'dashscope-prod',
      spec: {
        name: 'backend-qwen-1',
        kind: AI_BACKEND_FAMILIES.RT_ASR_QWEN_WS,
        operations: ['speech_to_text'],
        features: [],
        transports: ['websocket'],
        weight: 100,
        priority: 0,
        base_url: 'https://maas.qianwenaiapi.com/compatible-mode/v1',
        provider: AI_BACKEND_PROVIDERS.QWEN,
        model: 'qwen3-asr-flash-realtime',
        credential_ref: 'dashscope-prod',
        origin: 0,
        deployment_id: '',
      },
      metadata: {},
      labels: {},
      generation: 1,
      created_at_ms: 0,
      updated_at_ms: 0,
    }

    expect(resolveBackendDetailSummary(backend)).toEqual({
      operations: ['speech_to_text'],
      features: [],
      transports: ['websocket'],
      baseUrl: 'https://maas.qianwenaiapi.com/compatible-mode/v1',
      credentialRef: 'dashscope-prod',
    })
    expect(resolveBackendProviderFields(backend)).toEqual([])
  })
})
