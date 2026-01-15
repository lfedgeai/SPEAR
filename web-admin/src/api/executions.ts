import { fetchJson } from '@/api/client'

export type CreateExecutionPayload = {
  /** Task ID / 任务 ID */
  task_id: string
  /** Preferred node uuid (optional) / 期望节点 uuid（可选） */
  node_uuid?: string
  /** Execution mode (optional) / 执行模式（可选） */
  execution_mode?: 'sync' | 'async' | 'stream'
  /** Maximum placement candidates (optional) / 最大候选节点数（可选） */
  max_candidates?: number
}

export type CreateExecutionResponse = {
  /** Whether the request succeeded / 请求是否成功 */
  success: boolean
  /** Human readable message (optional) / 人类可读信息（可选） */
  message?: string
  /** Created execution id (optional) / 创建的 execution id（可选） */
  execution_id?: string
  /** Selected node uuid (optional) / 选中的节点 uuid（可选） */
  node_uuid?: string
  /** Placement decision id (optional) / 调度决策 id（可选） */
  decision_id?: string
}

/**
 * Create an execution via SMS Web Admin API.
 * 通过 SMS Web Admin API 创建一次执行。
 */
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
