import { fetchJson } from '@/api/client'
import type { NodeDetail, NodeSummary } from '@/api/types'

export type ListNodesParams = {
  q?: string
  sort_by?: string
  order?: 'asc' | 'desc'
  limit?: number
}

export async function listNodes(params: ListNodesParams) {
  const url = new URL('/admin/api/nodes', window.location.origin)
  if (params.q) url.searchParams.set('q', params.q)
  if (params.sort_by) url.searchParams.set('sort_by', params.sort_by)
  if (params.order) url.searchParams.set('order', params.order)
  if (params.limit) url.searchParams.set('limit', String(params.limit))
  return fetchJson<{ nodes: NodeSummary[]; total_count: number }>(
    url.pathname + url.search,
  )
}

export function getNodeDetail(uuid: string) {
  return fetchJson<NodeDetail>(`/admin/api/nodes/${encodeURIComponent(uuid)}`)
}

