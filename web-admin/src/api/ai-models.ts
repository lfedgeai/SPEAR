import { fetchJson } from '@/api/client'

export type AiModelInstance = {
  node_uuid: string
  backend_name: string
  kind: string
  base_url: string
  status: 'available' | 'unavailable'
  status_reason?: string
  weight?: number
  priority?: number
  provider: string
  model: string
  hosting: 'local' | 'remote' | 'unknown'
}

export type AiModelInfo = {
  provider: string
  model: string
  hosting: 'local' | 'remote' | 'unknown'
  operations: string[]
  features: string[]
  transports: string[]
  available_nodes: number
  total_nodes: number
  instances: AiModelInstance[]
}

export type ListAiModelsResponse = {
  success: boolean
  message?: string
  models?: AiModelInfo[]
  total_count?: number
}

export type GetAiModelDetailResponse = {
  success: boolean
  message?: string
  found?: boolean
  model?: AiModelInfo
}

export function listAiModels(params: {
  hosting?: 'local' | 'remote'
  provider?: string
  q?: string
  limit?: number
  offset?: number
}) {
  const qs = new URLSearchParams()
  if (params.hosting) qs.set('hosting', params.hosting)
  if (params.provider) qs.set('provider', params.provider)
  if (params.q) qs.set('q', params.q)
  if (typeof params.limit === 'number') qs.set('limit', String(params.limit))
  if (typeof params.offset === 'number') qs.set('offset', String(params.offset))
  const path = qs.toString()
    ? `/admin/api/ai-models?${qs.toString()}`
    : '/admin/api/ai-models'
  return fetchJson<ListAiModelsResponse>(path)
}

export function getAiModelDetail(params: {
  provider: string
  model: string
  hosting?: 'local' | 'remote'
}) {
  const qs = new URLSearchParams()
  if (params.hosting) qs.set('hosting', params.hosting)
  const path = qs.toString()
    ? `/admin/api/ai-models/${encodeURIComponent(params.provider)}/${encodeURIComponent(params.model)}?${qs.toString()}`
    : `/admin/api/ai-models/${encodeURIComponent(params.provider)}/${encodeURIComponent(params.model)}`
  return fetchJson<GetAiModelDetailResponse>(path)
}

