import { fetchJson } from '@/api/client'

export type ModelDeploymentStatus = {
  phase: number
  message: string
  updated_at_ms: number
}

export type ModelDeploymentSpec = {
  target_node_uuid: string
  provider: string
  model: string
  params: Record<string, string>
}

export type ModelDeploymentRecord = {
  deployment_id: string
  revision: number
  created_at_ms: number
  updated_at_ms: number
  spec?: ModelDeploymentSpec
  status?: ModelDeploymentStatus
}

export type CreateNodeModelDeploymentResponse = {
  success: boolean
  message?: string
  deployment_id?: string
  revision?: number
}

export type ListNodeModelDeploymentsResponse = {
  success: boolean
  message?: string
  revision?: number
  total_count?: number
  deployments?: ModelDeploymentRecord[]
}

export async function createNodeModelDeployment(params: {
  node_uuid: string
  provider: string
  model: string
  params?: Record<string, string>
}) {
  return fetchJson<CreateNodeModelDeploymentResponse>(
    `/admin/api/nodes/${encodeURIComponent(params.node_uuid)}/ai-models`,
    {
      method: 'POST',
      headers: { 'Content-Type': 'application/json' },
      body: JSON.stringify({
        provider: params.provider,
        model: params.model,
        params: params.params,
      }),
    },
  )
}

export async function listNodeModelDeployments(params: {
  node_uuid: string
  limit?: number
  offset?: number
}) {
  const qs = new URLSearchParams()
  if (typeof params.limit === 'number') qs.set('limit', String(params.limit))
  if (typeof params.offset === 'number') qs.set('offset', String(params.offset))
  const path = qs.toString()
    ? `/admin/api/nodes/${encodeURIComponent(params.node_uuid)}/ai-models/deployments?${qs.toString()}`
    : `/admin/api/nodes/${encodeURIComponent(params.node_uuid)}/ai-models/deployments`
  return fetchJson<ListNodeModelDeploymentsResponse>(path)
}

export type DeleteNodeModelDeploymentResponse = {
  success: boolean
  message?: string
  revision?: number
}

export async function deleteNodeModelDeployment(params: {
  node_uuid: string
  deployment_id: string
}) {
  return fetchJson<DeleteNodeModelDeploymentResponse>(
    `/admin/api/nodes/${encodeURIComponent(params.node_uuid)}/ai-models/deployments/${encodeURIComponent(params.deployment_id)}`,
    { method: 'DELETE' },
  )
}
