import { fetchJson } from '@/api/client'

export type TerminateExecutionResponse = {
  success: boolean
  message?: string
  execution_id?: string
  node_uuid?: string
  final_status?: number
}

export function terminateExecution(input: { execution_id: string; reason?: string }) {
  return fetchJson<TerminateExecutionResponse>(
    `/admin/api/executions/${encodeURIComponent(input.execution_id)}/terminate`,
    {
      method: 'POST',
      headers: { 'Content-Type': 'application/json' },
      body: JSON.stringify({ reason: input.reason || '' }),
    },
  )
}

export type DestroyInstanceResponse = {
  success: boolean
  message?: string
  instance_id?: string
  node_uuid?: string
}

export function destroyInstance(input: {
  instance_id: string
  node_uuid: string
  reason?: string
}) {
  return fetchJson<DestroyInstanceResponse>(
    `/admin/api/instances/${encodeURIComponent(input.instance_id)}/destroy`,
    {
      method: 'POST',
      headers: { 'Content-Type': 'application/json' },
      body: JSON.stringify({ node_uuid: input.node_uuid, reason: input.reason || '' }),
    },
  )
}

