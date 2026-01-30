import { useMemo, useState } from 'react'
import { useQuery } from '@tanstack/react-query'
import { Search, Server } from 'lucide-react'
import { useNavigate } from 'react-router-dom'

import { listNodes } from '@/api/nodes'
import { Badge } from '@/components/ui/badge'
import { Button } from '@/components/ui/button'
import { Card, CardContent, CardHeader, CardTitle } from '@/components/ui/card'
import { Input } from '@/components/ui/input'
import { cn } from '@/lib/utils'

function formatTs(ts: number) {
  if (!ts) return '-'
  const d = new Date(ts * 1000)
  return d.toLocaleString()
}

function ageSeconds(ts: number) {
  if (!ts) return null
  const now = Date.now() / 1000
  return Math.max(0, Math.floor(now - ts))
}

function StatusBadge({ status }: { status: string }) {
  const s = (status || '').toLowerCase()
  if (s === 'online' || s === 'active') return <Badge variant="success">{status}</Badge>
  return <Badge variant="destructive">{status || 'unknown'}</Badge>
}

export default function NodesPage() {
  const navigate = useNavigate()
  const [q, setQ] = useState('')

  const nodesQuery = useQuery({
    queryKey: ['nodes', q],
    queryFn: () =>
      listNodes({ q, sort_by: 'last_heartbeat', order: 'desc', limit: 200 }),
    refetchInterval: 15_000,
  })

  const rows = nodesQuery.data?.nodes || []
  const total = nodesQuery.data?.total_count ?? 0
  const title = useMemo(() => `Nodes (${rows.length}/${total})`, [rows.length, total])

  return (
    <div className="space-y-4">
      <div className="flex items-center justify-between">
        <div>
          <div className="text-lg font-semibold">Nodes</div>
          <div className="text-sm text-[hsl(var(--muted-foreground))]">
            Registered nodes
          </div>
        </div>
        <Button variant="secondary" onClick={() => nodesQuery.refetch()}>
          Refresh
        </Button>
      </div>

      <Card>
        <CardHeader>
          <CardTitle>{title}</CardTitle>
        </CardHeader>
        <CardContent>
          <div className="mb-3 flex items-center gap-2">
            <div className="relative w-full max-w-md">
              <Search className="absolute left-3 top-1/2 h-4 w-4 -translate-y-1/2 text-[hsl(var(--muted-foreground))]" />
              <Input
                value={q}
                onChange={(e) => setQ(e.target.value)}
                placeholder="Search uuid/ip/name/metadata"
                className="pl-9"
              />
            </div>
            <div className="ml-auto text-xs text-[hsl(var(--muted-foreground))]">
              Auto refresh 15s
            </div>
          </div>

          <div className="overflow-hidden rounded-[var(--radius)] border border-[hsl(var(--border))] bg-[hsl(var(--background))]">
            <div className="grid grid-cols-12 border-b border-[hsl(var(--border))] bg-[hsl(var(--muted))] px-3 py-2 text-xs font-medium text-[hsl(var(--muted-foreground))]">
              <div className="col-span-4">Name / UUID</div>
              <div className="col-span-2">Status</div>
              <div className="col-span-3">Last heartbeat</div>
              <div className="col-span-3">Address</div>
            </div>

            {rows.length === 0 ? (
              <div className="p-6 text-sm text-[hsl(var(--muted-foreground))]">
                {nodesQuery.isLoading
                  ? 'Loading...'
                  : nodesQuery.isError
                    ? 'Failed to load nodes'
                    : 'No nodes'}
              </div>
            ) : (
              <div className="max-h-[560px] overflow-auto">
                {rows.map((n) => {
                  const age = ageSeconds(n.last_heartbeat)
                  return (
                    <button
                      key={n.uuid}
                      type="button"
                      onClick={() => {
                        navigate(`/nodes/${encodeURIComponent(n.uuid)}`)
                      }}
                      className={cn(
                        'grid w-full grid-cols-12 items-center gap-2 px-3 py-2 text-left text-sm hover:bg-[hsl(var(--accent))]',
                        'border-b border-[hsl(var(--border))] last:border-b-0',
                      )}
                    >
                      <div className="col-span-4 min-w-0">
                        <div className="flex items-center gap-2">
                          <Server className="h-4 w-4 text-[hsl(var(--muted-foreground))]" />
                          <div className="truncate font-medium">{n.name || '-'}</div>
                        </div>
                        <div className="mt-1 truncate text-xs text-[hsl(var(--muted-foreground))]">
                          {n.uuid}
                        </div>
                      </div>
                      <div className="col-span-2">
                        <StatusBadge status={n.status} />
                      </div>
                      <div className="col-span-3">
                        <div className="text-sm">{formatTs(n.last_heartbeat)}</div>
                        <div className="text-xs text-[hsl(var(--muted-foreground))]">
                          {age === null ? '-' : `${age}s ago`}
                        </div>
                      </div>
                      <div className="col-span-3 text-sm text-[hsl(var(--muted-foreground))]">
                        {n.ip_address}:{n.port}
                      </div>
                    </button>
                  )
                })}
              </div>
            )}
          </div>
        </CardContent>
      </Card>
    </div>
  )
}
