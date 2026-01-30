import { useMemo } from 'react'
import { Link, useNavigate, useParams } from 'react-router-dom'
import { useInfiniteQuery, useQuery } from '@tanstack/react-query'

import { getTaskDetail } from '@/api/tasks'
import { listTaskInstances } from '@/api/instanceExecution'
import type { InstanceSummary } from '@/api/types'
import { Badge } from '@/components/ui/badge'
import { Button } from '@/components/ui/button'
import { Card, CardContent, CardHeader, CardTitle } from '@/components/ui/card'
import { cn } from '@/lib/utils'

function formatMs(ts: number) {
  if (!ts) return '-'
  return new Date(ts).toLocaleString()
}

function InstanceStatusBadge({ status }: { status: string }) {
  const s = (status || '').toLowerCase()
  if (s === 'running') return <Badge variant="success">running</Badge>
  if (s === 'idle') return <Badge>idle</Badge>
  if (s === 'terminating') return <Badge variant="secondary">terminating</Badge>
  if (s === 'terminated') return <Badge variant="secondary">terminated</Badge>
  return <Badge variant="destructive">{status || 'unknown'}</Badge>
}

export default function TaskDetailPage() {
  const { taskId } = useParams()
  const id = taskId || ''
  const navigate = useNavigate()

  const taskQuery = useQuery({
    queryKey: ['task-detail', id],
    queryFn: () => getTaskDetail(id),
    enabled: !!id,
  })

  const instancesQuery = useInfiniteQuery({
    queryKey: ['task-instances', id],
    queryFn: ({ pageParam }) =>
      listTaskInstances({
        task_id: id,
        limit: 100,
        page_token: pageParam || undefined,
      }),
    enabled: !!id,
    initialPageParam: '',
    getNextPageParam: (lastPage) => {
      if (!lastPage.success) return undefined
      return lastPage.next_page_token || undefined
    },
    refetchInterval: 15_000,
  })

  const instances: InstanceSummary[] = useMemo(() => {
    const pages = instancesQuery.data?.pages || []
    const all: InstanceSummary[] = []
    for (const p of pages) {
      if (!p.success) continue
      all.push(...(p.instances || []))
    }
    return all
  }, [instancesQuery.data])

  const title = useMemo(() => `Task ${id}`, [id])

  return (
    <div className="space-y-4">
      <div className="flex items-center justify-between">
        <div>
          <div className="text-lg font-semibold">{title}</div>
          <div className="text-sm text-[hsl(var(--muted-foreground))]">
            <Link
              to="/tasks"
              className="text-[hsl(var(--muted-foreground))] hover:underline"
            >
              Tasks
            </Link>
            <span className="mx-2">/</span>
            <span className="font-mono text-xs">{id}</span>
          </div>
        </div>
        <div className="flex items-center gap-2">
          <Button variant="secondary" onClick={() => taskQuery.refetch()}>
            Refresh
          </Button>
        </div>
      </div>

      <Card>
        <CardHeader>
          <div className="flex items-center justify-between">
            <CardTitle>Overview</CardTitle>
            <Button variant="secondary" onClick={() => taskQuery.refetch()}>
              Refresh
            </Button>
          </div>
        </CardHeader>
        <CardContent>
          {taskQuery.isLoading ? (
            <div className="text-sm text-[hsl(var(--muted-foreground))]">Loading…</div>
          ) : taskQuery.isError ? (
            <div className="text-sm text-[hsl(var(--muted-foreground))]">
              Failed to load task detail.
            </div>
          ) : taskQuery.data?.found ? (
            <pre className="max-h-[420px] overflow-auto rounded-[var(--radius)] border border-[hsl(var(--border))] bg-[hsl(var(--secondary))] p-3 text-xs">
              {JSON.stringify(taskQuery.data, null, 2)}
            </pre>
          ) : (
            <div className="text-sm text-[hsl(var(--muted-foreground))]">Not found</div>
          )}
        </CardContent>
      </Card>

      <Card>
        <CardHeader>
          <div className="flex items-center justify-between">
            <CardTitle>Active instances</CardTitle>
            <Button
              variant="secondary"
              onClick={() => {
                instancesQuery.refetch()
              }}
            >
              Refresh
            </Button>
          </div>
        </CardHeader>
        <CardContent>
          {!instancesQuery.data ? (
            <div className="text-sm text-[hsl(var(--muted-foreground))]">
              {instancesQuery.isLoading ? 'Loading…' : 'No data'}
            </div>
          ) : instancesQuery.data.pages.some((p) => !p.success) ? (
            <div className="text-sm text-[hsl(var(--muted-foreground))]">
              {instancesQuery.data.pages.find((p) => !p.success)?.message ||
                'Failed to load instances'}
            </div>
          ) : instances.length === 0 ? (
            <div className="text-sm text-[hsl(var(--muted-foreground))]">
              No active instances.
            </div>
          ) : (
            <div className="overflow-auto rounded-[var(--radius)] border border-[hsl(var(--border))]">
              <table className="w-full text-sm">
                <thead className="bg-[hsl(var(--muted))] text-left text-xs text-[hsl(var(--muted-foreground))]">
                  <tr>
                    <th className="px-3 py-2">Instance</th>
                    <th className="px-3 py-2">Node</th>
                    <th className="px-3 py-2">Status</th>
                    <th className="px-3 py-2">Last seen</th>
                    <th className="px-3 py-2">Current execution</th>
                    <th className="px-3 py-2"></th>
                  </tr>
                </thead>
                <tbody>
                  {instances.map((row) => (
                    <tr
                      key={row.instance_id}
                      className={cn(
                        'cursor-pointer border-t border-[hsl(var(--border))] hover:bg-[hsl(var(--accent))]',
                      )}
                      onClick={() =>
                        navigate(`/instances/${encodeURIComponent(row.instance_id)}`)
                      }
                      role="button"
                      tabIndex={0}
                      onKeyDown={(e) => {
                        if (e.key === 'Enter' || e.key === ' ') {
                          e.preventDefault()
                          navigate(`/instances/${encodeURIComponent(row.instance_id)}`)
                        }
                      }}
                    >
                      <td className="px-3 py-2 font-mono text-xs">
                        <Link
                          to={`/instances/${encodeURIComponent(row.instance_id)}`}
                          className="hover:underline"
                          onClick={(e) => e.stopPropagation()}
                        >
                          {row.instance_id}
                        </Link>
                      </td>
                      <td className="px-3 py-2 font-mono text-xs">
                        {row.node_uuid ? (
                          <Link
                            to={`/nodes/${encodeURIComponent(row.node_uuid)}`}
                            className="hover:underline"
                            onClick={(e) => e.stopPropagation()}
                          >
                            {row.node_uuid}
                          </Link>
                        ) : (
                          '-'
                        )}
                      </td>
                      <td className="px-3 py-2">
                        <InstanceStatusBadge status={row.status} />
                      </td>
                      <td className="px-3 py-2 text-xs text-[hsl(var(--muted-foreground))]">
                        {formatMs(row.last_seen_ms)}
                      </td>
                      <td className="px-3 py-2 font-mono text-xs">
                        {row.current_execution_id ? (
                          <Link
                            to={`/executions/${encodeURIComponent(row.current_execution_id)}`}
                            className="hover:underline"
                            onClick={(e) => e.stopPropagation()}
                          >
                            {row.current_execution_id}
                          </Link>
                        ) : (
                          '-'
                        )}
                      </td>
                      <td className="px-3 py-2 text-right">
                        <Link
                          to={`/instances/${encodeURIComponent(row.instance_id)}`}
                          className="text-xs text-[hsl(var(--muted-foreground))] hover:underline"
                          onClick={(e) => e.stopPropagation()}
                        >
                          View
                        </Link>
                      </td>
                    </tr>
                  ))}
                </tbody>
              </table>
            </div>
          )}

          {instancesQuery.hasNextPage ? (
            <div className="mt-3">
              <Button
                variant="secondary"
                onClick={() => instancesQuery.fetchNextPage()}
                disabled={!instancesQuery.hasNextPage || instancesQuery.isFetchingNextPage}
              >
                {instancesQuery.isFetchingNextPage ? 'Loading…' : 'Load more'}
              </Button>
            </div>
          ) : null}
        </CardContent>
      </Card>
    </div>
  )
}
