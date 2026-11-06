import { fetchJson } from '@/api/client'

type ExecutionLogLine = {
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
