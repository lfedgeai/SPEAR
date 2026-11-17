/**
 * AI backend detail page helper utilities.
 * AI backend 详情页辅助工具。
 */

import type {
  AiBackendRemoteInput,
  AiBackendSummary,
} from '@/api/ai-backends'
import { AI_BACKEND_PROVIDERS } from './backendFamilyRegistry'

export type BackendDetailSummaryView = {
  operations: string[]
  features: string[]
  transports: string[]
  baseUrl: string
  credentialRef: string
}

export type BackendDetailProviderField = {
  label: string
  value: string
}

function isOpenAiRemoteInput(
  value?: AiBackendRemoteInput,
): value is Extract<
  AiBackendRemoteInput,
  { provider_family: typeof AI_BACKEND_PROVIDERS.OPENAI }
> {
  return value?.provider_family === AI_BACKEND_PROVIDERS.OPENAI
}

function isOllamaRemoteInput(
  value?: AiBackendRemoteInput,
): value is Extract<
  AiBackendRemoteInput,
  { provider_family: typeof AI_BACKEND_PROVIDERS.OLLAMA }
> {
  return value?.provider_family === AI_BACKEND_PROVIDERS.OLLAMA
}

function usesStructuredRemoteProviderInput(provider: string): boolean {
  return (
    provider === AI_BACKEND_PROVIDERS.OPENAI ||
    provider === AI_BACKEND_PROVIDERS.OLLAMA
  )
}

export function resolveBackendDetailSummary(
  backend: AiBackendSummary,
): BackendDetailSummaryView {
  const remoteOpenAi = isOpenAiRemoteInput(backend.remote) ? backend.remote : undefined
  const remoteOllama = isOllamaRemoteInput(backend.remote) ? backend.remote : undefined
  const normalizedProvider = backend.provider.trim().toLowerCase()

  return {
    operations: usesStructuredRemoteProviderInput(normalizedProvider)
      ? remoteOpenAi?.config.operations || remoteOllama?.config.operations || []
      : backend.spec?.operations || [],
    features: usesStructuredRemoteProviderInput(normalizedProvider)
      ? remoteOpenAi?.config.features || remoteOllama?.config.features || []
      : backend.spec?.features || [],
    transports: usesStructuredRemoteProviderInput(normalizedProvider)
      ? remoteOpenAi?.config.transports || remoteOllama?.config.transports || []
      : backend.spec?.transports || [],
    baseUrl: usesStructuredRemoteProviderInput(normalizedProvider)
      ? remoteOpenAi?.config.base_url || remoteOllama?.config.base_url || ''
      : backend.spec?.base_url || '',
    credentialRef:
      normalizedProvider === AI_BACKEND_PROVIDERS.OPENAI
        ? remoteOpenAi?.config.credential_ref || ''
        : !usesStructuredRemoteProviderInput(normalizedProvider)
          ? backend.credential_ref || backend.spec?.credential_ref || ''
          : '',
  }
}

export function resolveBackendProviderFields(
  backend: AiBackendSummary,
): BackendDetailProviderField[] {
  if (backend.local?.provider_family === AI_BACKEND_PROVIDERS.LLAMACPP) {
    const fields: BackendDetailProviderField[] = []
    const config = backend.local.config
    if (config.model_url) fields.push({ label: 'Model URL', value: config.model_url })
    if (config.model_path) fields.push({ label: 'Model Path', value: config.model_path })
    if (config.skip_download !== undefined) {
      fields.push({ label: 'Skip download', value: String(config.skip_download) })
    }
    if (config.download_timeout_s !== undefined) {
      fields.push({ label: 'Download timeout', value: String(config.download_timeout_s) })
    }
    if (config.threads !== undefined) {
      fields.push({ label: 'Threads', value: String(config.threads) })
    }
    if (config.ctx_size !== undefined) {
      fields.push({ label: 'Context size', value: String(config.ctx_size) })
    }
    return fields
  }

  if (backend.local?.provider_family === AI_BACKEND_PROVIDERS.VLLM) {
    const fields: BackendDetailProviderField[] = []
    const config = backend.local.config
    if (config.mode) fields.push({ label: 'Mode', value: config.mode })
    if (config.managed_externally !== undefined) {
      fields.push({
        label: 'Managed externally',
        value: String(config.managed_externally),
      })
    }
    return fields
  }

  return []
}
