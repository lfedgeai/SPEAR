import { useMemo, useState } from 'react'
import { useQuery } from '@tanstack/react-query'
import { Copy, Search, Server } from 'lucide-react'
import { toast } from 'sonner'

import { getNodeDetail, listNodes } from '@/api/nodes'
import type { NodeSummary } from '@/api/types'
import { Badge } from '@/components/ui/badge'
import { Button } from '@/components/ui/button'
import { Card, CardContent, CardHeader, CardTitle } from '@/components/ui/card'
import { Dialog, DialogContent, DialogHeader } from '@/components/ui/dialog'
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

function CopyButton({ value }: { value: string }) {
  return (
    <Button
      variant="ghost"
      size="sm"
      onClick={async () => {
        await navigator.clipboard.writeText(value)
        toast.success('Copied')
      }}
    >
      <Copy className="h-4 w-4" />
      Copy
    </Button>
  )
}

export default function NodesPage() {
  const [q, setQ] = useState('')
  const [selected, setSelected] = useState<NodeSummary | null>(null)
  const [detailOpen, setDetailOpen] = useState(false)

  const nodesQuery = useQuery({
    queryKey: ['nodes', q],
    queryFn: () =>
      listNodes({ q, sort_by: 'last_heartbeat', order: 'desc', limit: 200 }),
    refetchInterval: 15_000,
  })

  const selectedUuid = selected?.uuid
  const detailQuery = useQuery({
    queryKey: ['node-detail', selectedUuid],
    queryFn: () => getNodeDetail(selectedUuid!),
    enabled: !!selectedUuid && detailOpen,
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
                        setSelected(n)
                        setDetailOpen(true)
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

      <Dialog open={detailOpen} onOpenChange={setDetailOpen}>
        <DialogContent>
          <DialogHeader
            title={selected ? selected.name || 'Node detail' : 'Node detail'}
            description={selected ? selected.uuid : undefined}
          />
          {detailQuery.isLoading ? (
            <div className="text-sm text-[hsl(var(--muted-foreground))]">Loading…</div>
          ) : detailQuery.isError ? (
            <div className="text-sm text-[hsl(var(--muted-foreground))]">
              Failed to load node detail.
            </div>
          ) : detailQuery.data?.found ? (
            <div className="space-y-4">
              <div className="grid grid-cols-2 gap-3">
                <Card>
                  <CardHeader>
                    <CardTitle>Summary</CardTitle>
                  </CardHeader>
                  <CardContent className="space-y-2">
                    <div className="flex items-center justify-between text-sm">
                      <span className="text-[hsl(var(--muted-foreground))]">Status</span>
                      <StatusBadge status={detailQuery.data.node?.status || ''} />
                    </div>
                    <div className="flex items-center justify-between text-sm">
                      <span className="text-[hsl(var(--muted-foreground))]">Address</span>
                      <span>
                        {detailQuery.data.node?.ip_address}:{detailQuery.data.node?.port}
                      </span>
                    </div>
                    <div className="flex items-center justify-between text-sm">
                      <span className="text-[hsl(var(--muted-foreground))]">Registered</span>
                      <span>
                        {detailQuery.data.node?.registered_at
                          ? formatTs(detailQuery.data.node.registered_at)
                          : '-'}
                      </span>
                    </div>
                    <div className="flex items-center justify-between text-sm">
                      <span className="text-[hsl(var(--muted-foreground))]">Last heartbeat</span>
                      <span>
                        {detailQuery.data.node?.last_heartbeat
                          ? formatTs(detailQuery.data.node.last_heartbeat)
                          : '-'}
                      </span>
                    </div>
                  </CardContent>
                </Card>

                <Card>
                  <CardHeader>
                    <CardTitle>Resources</CardTitle>
                  </CardHeader>
                  <CardContent className="space-y-2">
                    <div className="flex items-center justify-between text-sm">
                      <span className="text-[hsl(var(--muted-foreground))]">CPU</span>
                      <span>{detailQuery.data.resource?.cpu_usage_percent ?? '-'}%</span>
                    </div>
                    <div className="flex items-center justify-between text-sm">
                      <span className="text-[hsl(var(--muted-foreground))]">Memory</span>
                      <span>
                        {detailQuery.data.resource?.memory_usage_percent ?? '-'}%
                      </span>
                    </div>
                    <div className="flex items-center justify-between text-sm">
                      <span className="text-[hsl(var(--muted-foreground))]">Disk</span>
                      <span>{detailQuery.data.resource?.disk_usage_percent ?? '-'}%</span>
                    </div>
                  </CardContent>
                </Card>
              </div>

              <div className="flex items-center justify-between">
                <div className="text-sm text-[hsl(var(--muted-foreground))]">Raw JSON</div>
                {selected?.uuid ? <CopyButton value={selected.uuid} /> : null}
              </div>
              <pre className="max-h-[260px] overflow-auto rounded-[var(--radius)] border border-[hsl(var(--border))] bg-[hsl(var(--secondary))] p-3 text-xs">
                {JSON.stringify(detailQuery.data, null, 2)}
              </pre>
            </div>
          ) : (
            <div className="text-sm text-[hsl(var(--muted-foreground))]">Not found</div>
          )}
        </DialogContent>
      </Dialog>
    </div>
  )
}

