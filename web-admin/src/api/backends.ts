import { fetchJson } from '@/api/client'

export type BackendNodeEntry = {
  node_uuid: string
  status: 'available' | 'unavailable'
  status_reason?: string
  weight?: number
  priority?: number
  base_url?: string
}

export type AggregatedBackend = {
  name: string
  kind: string
  operations: string[]
  features: string[]
  transports: string[]
  available_nodes: number
  total_nodes: number
  nodes: BackendNodeEntry[]
}

export type ListBackendsResponse = {
  success: boolean
  message?: string
  backends?: AggregatedBackend[]
  total_count?: number
}

export type GetBackendDetailResponse = {
  success: boolean
  message?: string
  found?: boolean
  backend?: AggregatedBackend
}

export function listBackends(params: {
  q?: string
  status?: 'available' | 'unavailable'
  limit?: number
  offset?: number
}) {
  const qs = new URLSearchParams()
  if (params.q) qs.set('q', params.q)
  if (params.status) qs.set('status', params.status)
  if (typeof params.limit === 'number') qs.set('limit', String(params.limit))
  if (typeof params.offset === 'number') qs.set('offset', String(params.offset))
  const path = qs.toString() ? `/admin/api/backends?${qs.toString()}` : '/admin/api/backends'
  return fetchJson<ListBackendsResponse>(path)
}

export function getBackendDetail(params: { kind: string; name: string }) {
  return fetchJson<GetBackendDetailResponse>(
    `/admin/api/backends/${encodeURIComponent(params.kind)}/${encodeURIComponent(params.name)}`,
  )
}
