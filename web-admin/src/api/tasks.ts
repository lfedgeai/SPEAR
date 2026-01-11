import { fetchJson } from '@/api/client'
import type { TaskDetail, TaskSummary } from '@/api/types'

export type ListTasksParams = {
  q?: string
  sort_by?: string
  order?: 'asc' | 'desc'
  limit?: number
}

export async function listTasks(params: ListTasksParams) {
  const url = new URL('/admin/api/tasks', window.location.origin)
  if (params.q) url.searchParams.set('q', params.q)
  if (params.sort_by) url.searchParams.set('sort_by', params.sort_by)
  if (params.order) url.searchParams.set('order', params.order)
  if (params.limit) url.searchParams.set('limit', String(params.limit))
  return fetchJson<{ tasks: TaskSummary[]; total_count: number }>(
    url.pathname + url.search,
  )
}

export function getTaskDetail(taskId: string) {
  return fetchJson<TaskDetail>(`/admin/api/tasks/${encodeURIComponent(taskId)}`)
}

export type CreateTaskPayload = {
  name: string
  description?: string
  priority?: string
  execution_kind?: 'short_running' | 'long_running'
  node_uuid?: string
  endpoint: string
  version: string
  capabilities?: string[]
  executable?: {
    type: string
    uri?: string
    name?: string
    checksum_sha256?: string
    args?: string[]
    env?: Record<string, string>
  }
}

export function createTask(payload: CreateTaskPayload) {
  return fetchJson<{ success: boolean; task_id?: string; message?: string }>(
    '/admin/api/tasks',
    {
      method: 'POST',
      headers: { 'content-type': 'application/json' },
      body: JSON.stringify({
        name: payload.name,
        description: payload.description,
        priority: payload.priority,
        node_uuid: payload.node_uuid || '',
        endpoint: payload.endpoint,
        version: payload.version,
        capabilities: payload.capabilities,
        metadata: payload.execution_kind
          ? { execution_kind: payload.execution_kind }
          : undefined,
        executable: payload.executable,
      }),
    },
  )
}
