import { Link, useParams } from 'react-router-dom'
import { useQuery } from '@tanstack/react-query'
import { Copy } from 'lucide-react'
import { toast } from 'sonner'

import { getNodeDetail } from '@/api/nodes'
import { Badge } from '@/components/ui/badge'
import { Button } from '@/components/ui/button'
import { Card, CardContent, CardHeader, CardTitle } from '@/components/ui/card'

function formatTs(ts: number) {
  if (!ts) return '-'
  const d = new Date(ts * 1000)
  return d.toLocaleString()
}

function StatusBadge({ status }: { status: string }) {
  const s = (status || '').toLowerCase()
  if (s === 'online' || s === 'active') return <Badge variant="success">{status}</Badge>
  return <Badge variant="destructive">{status || 'unknown'}</Badge>
}

function CopyButton({ value }: { value: string }) {
  return (
    <Button
      variant="secondary"
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

export default function NodeDetailPage() {
  const { uuid } = useParams()
  const id = uuid || ''

  const q = useQuery({
    queryKey: ['node-detail', id],
    queryFn: () => getNodeDetail(id),
    enabled: !!id,
    refetchInterval: 15_000,
  })

  const node = q.data?.node
  const resource = q.data?.resource

  return (
    <div className="space-y-4">
      <div className="flex items-center justify-between">
        <div>
          <div className="text-lg font-semibold">{node?.name || 'Node'}</div>
          <div className="text-sm text-[hsl(var(--muted-foreground))]">
            <Link to="/nodes" className="hover:underline">
              Nodes
            </Link>
            <span className="mx-2">/</span>
            <span className="font-mono text-xs">{id}</span>
          </div>
        </div>
        <div className="flex items-center gap-2">
          <Button variant="secondary" onClick={() => q.refetch()}>
            Refresh
          </Button>
          {id ? <CopyButton value={id} /> : null}
        </div>
      </div>

      {q.isLoading ? (
        <div className="text-sm text-[hsl(var(--muted-foreground))]">Loading…</div>
      ) : q.isError ? (
        <div className="text-sm text-[hsl(var(--muted-foreground))]">
          Failed to load node detail.
        </div>
      ) : !q.data?.found ? (
        <div className="text-sm text-[hsl(var(--muted-foreground))]">Not found</div>
      ) : (
        <div className="space-y-4">
          <div className="grid grid-cols-2 gap-3">
            <Card>
              <CardHeader>
                <CardTitle>Summary</CardTitle>
              </CardHeader>
              <CardContent className="space-y-2">
                <div className="flex items-center justify-between text-sm">
                  <span className="text-[hsl(var(--muted-foreground))]">Status</span>
                  <StatusBadge status={node?.status || ''} />
                </div>
                <div className="flex items-center justify-between text-sm">
                  <span className="text-[hsl(var(--muted-foreground))]">Address</span>
                  <span>
                    {node?.ip_address}:{node?.port}
                  </span>
                </div>
                <div className="flex items-center justify-between text-sm">
                  <span className="text-[hsl(var(--muted-foreground))]">Registered</span>
                  <span>{node?.registered_at ? formatTs(node.registered_at) : '-'}</span>
                </div>
                <div className="flex items-center justify-between text-sm">
                  <span className="text-[hsl(var(--muted-foreground))]">Last heartbeat</span>
                  <span>{node?.last_heartbeat ? formatTs(node.last_heartbeat) : '-'}</span>
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
                  <span>{resource?.cpu_usage_percent ?? '-'}%</span>
                </div>
                <div className="flex items-center justify-between text-sm">
                  <span className="text-[hsl(var(--muted-foreground))]">Memory</span>
                  <span>{resource?.memory_usage_percent ?? '-'}%</span>
                </div>
                <div className="flex items-center justify-between text-sm">
                  <span className="text-[hsl(var(--muted-foreground))]">Disk</span>
                  <span>{resource?.disk_usage_percent ?? '-'}%</span>
                </div>
              </CardContent>
            </Card>
          </div>

          <div className="flex items-center justify-between">
            <div className="text-sm text-[hsl(var(--muted-foreground))]">Raw JSON</div>
          </div>
          <pre className="max-h-[520px] overflow-auto rounded-[var(--radius)] border border-[hsl(var(--border))] bg-[hsl(var(--secondary))] p-3 text-xs">
            {JSON.stringify(q.data, null, 2)}
          </pre>
        </div>
      )}
    </div>
  )
}

