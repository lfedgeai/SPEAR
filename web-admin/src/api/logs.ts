import { fetchJson, getAdminToken } from '@/api/client'

export type ExecutionLogLine = {
  ts_ms: number
  seq: number
  stream: string
  level: string
  message: string
}

export type GetExecutionLogsResponse = {
  success: boolean
  execution_id: string
  lines: ExecutionLogLine[]
  next_cursor: string
  truncated: boolean
  completed: boolean
}

export function getExecutionLogs(input: {
  execution_id: string
  cursor?: string
  limit?: number
}) {
  const qs = new URLSearchParams()
  if (input.cursor) qs.set('cursor', input.cursor)
  if (typeof input.limit === 'number') qs.set('limit', String(input.limit))
  const suffix = qs.toString() ? `?${qs.toString()}` : ''
  return fetchJson<GetExecutionLogsResponse>(
    `/admin/api/executions/${encodeURIComponent(input.execution_id)}/logs${suffix}`,
  )
}

export async function downloadExecutionLogsText(execution_id: string) {
  const token = getAdminToken()
  const headers: Record<string, string> = {}
  if (token) headers.Authorization = `Bearer ${token}`
  const resp = await fetch(
    `/admin/api/executions/${encodeURIComponent(execution_id)}/logs/download?format=text`,
    { headers },
  )
  if (!resp.ok) throw new Error(`Download failed: HTTP ${resp.status}`)
  return await resp.text()
}
