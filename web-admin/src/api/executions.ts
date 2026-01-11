import { fetchJson } from '@/api/client'

export type CreateExecutionPayload = {
  task_id: string
  node_uuid?: string
  execution_mode?: 'sync' | 'async' | 'stream'
  max_candidates?: number
}

export type CreateExecutionResponse = {
  success: boolean
  message?: string
  execution_id?: string
  node_uuid?: string
  decision_id?: string
}

export function createExecution(payload: CreateExecutionPayload) {
  return fetchJson<CreateExecutionResponse>('/admin/api/executions', {
    method: 'POST',
    headers: { 'content-type': 'application/json' },
    body: JSON.stringify({
      task_id: payload.task_id,
      node_uuid: payload.node_uuid,
      execution_mode: payload.execution_mode,
      max_candidates: payload.max_candidates,
    }),
  })
}
