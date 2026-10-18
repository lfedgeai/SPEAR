import { useMemo, useState } from 'react'
import { useInfiniteQuery } from '@tanstack/react-query'

import { listTaskInstances } from '@/api/instanceExecution'
import type { InstanceSummary } from '@/api/types'

export function useTaskInstances(taskId: string) {
  const [destroyPendingIdsState, setDestroyPendingIdsState] = useState<string[]>([])

  const query = useInfiniteQuery({
    queryKey: ['task-instances', taskId],
    queryFn: ({ pageParam }) =>
      listTaskInstances({
        task_id: taskId,
        limit: 100,
        page_token: pageParam || undefined,
      }),
    enabled: !!taskId,
    initialPageParam: '',
    getNextPageParam: (lastPage) => {
      if (!lastPage.success) return undefined
      return lastPage.next_page_token || undefined
    },
    refetchInterval: destroyPendingIdsState.length > 0 ? 1_500 : 15_000,
  })

  const instances: InstanceSummary[] = useMemo(() => {
    const pages = query.data?.pages || []
    const all: InstanceSummary[] = []
    for (const page of pages) {
      if (!page.success) continue
      all.push(...(page.instances || []))
    }
    return all
  }, [query.data])

  const destroyPendingIds = useMemo(
    () =>
      destroyPendingIdsState.filter((instanceId) =>
        instances.some((row) => row.instance_id === instanceId),
      ),
    [destroyPendingIdsState, instances],
  )

  const displayedInstances = useMemo(
    () =>
      instances.map((row) =>
        destroyPendingIds.includes(row.instance_id) ? { ...row, status: 'terminating' } : row,
      ),
    [destroyPendingIds, instances],
  )

  const loadErrorMessage = useMemo(
    () => query.data?.pages.find((page) => !page.success)?.message || '',
    [query.data],
  )

  function markDestroyPending(instanceId: string) {
    setDestroyPendingIdsState((prev) =>
      prev.includes(instanceId) ? prev : [...prev, instanceId],
    )
  }

  return {
    query,
    instances,
    displayedInstances,
    destroyPendingIds,
    loadErrorMessage,
    markDestroyPending,
  }
}
