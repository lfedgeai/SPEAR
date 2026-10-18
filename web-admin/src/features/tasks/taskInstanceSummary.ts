/**
 * Task instance summary helpers shared by task instance views.
 * 任务实例摘要辅助函数，供任务实例视图共享使用。
 */

import type { InstanceSummary } from '@/api/types'

export function summarizeTaskInstances(instances: InstanceSummary[]) {
  const ready = instances.filter((row) => {
    const status = row.status.toLowerCase()
    return status === 'running' || status === 'idle' || status === 'ready'
  }).length
  const activeExecutions = instances.filter(
    (row) => !!row.current_execution_id,
  ).length
  const terminating = instances.filter((row) => {
    const status = row.status.toLowerCase()
    return (
      status === 'terminating' ||
      status === 'stopping' ||
      status === 'terminated'
    )
  }).length
  return {
    total: instances.length,
    ready,
    activeExecutions,
    terminating,
  }
}
