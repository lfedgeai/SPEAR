import { fetchJson } from '@/api/client'
import type { TaskDetail, TaskSummary } from '@/api/types'

export type ListTasksParams = {
  /** Query string for fuzzy search / 模糊搜索关键词 */
  q?: string
  /** Sort field name / 排序字段 */
  sort_by?: string
  /** Sort order / 排序方向 */
  order?: 'asc' | 'desc'
  /** Maximum number of items / 最大返回条数 */
  limit?: number
}

/**
 * List tasks from SMS.
 * 从 SMS 列出任务。
 */
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

/**
 * Get task detail by task id.
 * 通过 task id 获取任务详情。
 */
export function getTaskDetail(taskId: string) {
  return fetchJson<TaskDetail>(`/admin/api/tasks/${encodeURIComponent(taskId)}`)
}

export type CreateTaskPayload = {
  /** Task name / 任务名称 */
  name: string
  /** Task description (optional) / 任务描述（可选） */
  description?: string
  /** Task priority (optional) / 任务优先级（可选） */
  priority?: string
  /** Execution kind (optional) / 执行类型（可选） */
  execution_kind?: 'short_running' | 'long_running'
  /** Target node uuid (optional) / 目标节点 uuid（可选） */
  node_uuid?: string
  /** Task endpoint / 任务端点 */
  endpoint: string
  /** Task version / 任务版本 */
  version: string
  /** Task capabilities (optional) / 任务能力（可选） */
  capabilities?: string[]
  /** Executable descriptor / 可执行描述 */
  executable?: {
    /** Executable type / 可执行类型 */
    type: string
    /** Executable uri (optional) / 可执行 uri（可选） */
    uri?: string
    /** Executable name (optional) / 可执行名称（可选） */
    name?: string
    /** SHA256 checksum (optional) / SHA256 校验（可选） */
    checksum_sha256?: string
    /** Default args (optional) / 默认参数（可选） */
    args?: string[]
    /** Default environment variables (optional) / 默认环境变量（可选） */
    env?: Record<string, string>
  }
  /** Task config map / Task 配置（map<string,string>） */
  config?: Record<string, string>
}

/**
 * Create a task.
 * 创建任务。
 */
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
        config: payload.config,
        executable: payload.executable,
      }),
    },
  )
}
