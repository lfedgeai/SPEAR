import { Link, useParams } from 'react-router-dom'
import { useQuery } from '@tanstack/react-query'
import { Copy } from 'lucide-react'
import { toast } from 'sonner'

import { getBackendDetail } from '@/api/backends'
import { Badge } from '@/components/ui/badge'
import { Button } from '@/components/ui/button'
import { Card, CardContent, CardHeader, CardTitle } from '@/components/ui/card'

function StatusBadge({ status }: { status: 'available' | 'unavailable' }) {
  if (status === 'available') return <Badge variant="success">{status}</Badge>
  return <Badge variant="destructive">{status}</Badge>
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

export default function BackendDetailPage() {
  const { kind, name } = useParams()
  const k = kind || ''
  const n = name || ''

  const q = useQuery({
    queryKey: ['backend-detail', k, n],
    queryFn: () => getBackendDetail({ kind: k, name: n }),
    enabled: !!k && !!n,
    refetchInterval: 15_000,
  })

  const backend = q.data?.backend
  const title = backend ? `${backend.name}` : 'Backend'

  return (
    <div className="space-y-4">
      <div className="flex items-center justify-between">
        <div>
          <div className="text-lg font-semibold">{title}</div>
          <div className="text-sm text-[hsl(var(--muted-foreground))]">
            <Link to="/backends" className="hover:underline">
              Backends
            </Link>
            <span className="mx-2">/</span>
            <span className="font-mono text-xs">{k}</span>
            <span className="mx-2">/</span>
            <span className="font-mono text-xs">{n}</span>
          </div>
        </div>
        <div className="flex items-center gap-2">
          <Button variant="secondary" onClick={() => q.refetch()}>
            Refresh
          </Button>
          {backend ? <CopyButton value={`${backend.kind}:${backend.name}`} /> : null}
        </div>
      </div>

      {q.isLoading ? (
        <div className="text-sm text-[hsl(var(--muted-foreground))]">Loading…</div>
      ) : q.isError ? (
        <div className="text-sm text-[hsl(var(--muted-foreground))]">
          Failed to load backend detail.
        </div>
      ) : !q.data?.success ? (
        <div className="text-sm text-[hsl(var(--muted-foreground))]">
          {q.data?.message || 'Failed to load backend detail.'}
        </div>
      ) : !q.data.found || !backend ? (
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
                  <StatusBadge
                    status={backend.available_nodes > 0 ? 'available' : 'unavailable'}
                  />
                </div>
                <div className="flex items-center justify-between text-sm">
                  <span className="text-[hsl(var(--muted-foreground))]">Available</span>
                  <span>
                    {backend.available_nodes}/{backend.total_nodes}
                  </span>
                </div>
                <div className="flex items-center justify-between text-sm">
                  <span className="text-[hsl(var(--muted-foreground))]">Kind</span>
                  <span className="font-mono text-xs">{backend.kind}</span>
                </div>
                <div className="flex items-center justify-between text-sm">
                  <span className="text-[hsl(var(--muted-foreground))]">Name</span>
                  <span className="font-mono text-xs">{backend.name}</span>
                </div>
              </CardContent>
            </Card>

            <Card>
              <CardHeader>
                <CardTitle>Capabilities</CardTitle>
              </CardHeader>
              <CardContent className="space-y-3">
                <div>
                  <div className="text-xs text-[hsl(var(--muted-foreground))]">Ops</div>
                  <div className="mt-1 flex flex-wrap gap-1">
                    {(backend.operations || []).map((op) => (
                      <Badge key={op} variant="secondary">
                        {op}
                      </Badge>
                    ))}
                  </div>
                </div>
                <div>
                  <div className="text-xs text-[hsl(var(--muted-foreground))]">Transports</div>
                  <div className="mt-1 flex flex-wrap gap-1">
                    {(backend.transports || []).map((t) => (
                      <Badge key={t} variant="secondary">
                        {t}
                      </Badge>
                    ))}
                  </div>
                </div>
              </CardContent>
            </Card>
          </div>

          <Card>
            <CardHeader>
              <CardTitle>Nodes</CardTitle>
            </CardHeader>
            <CardContent>
              {backend.nodes && backend.nodes.length > 0 ? (
                <div className="overflow-auto rounded-[var(--radius)] border border-[hsl(var(--border))]">
                  <table className="w-full text-sm">
                    <thead className="bg-[hsl(var(--muted))] text-left text-xs text-[hsl(var(--muted-foreground))]">
                      <tr>
                        <th className="px-3 py-2">Node</th>
                        <th className="px-3 py-2">Status</th>
                        <th className="px-3 py-2">Weight</th>
                        <th className="px-3 py-2">Priority</th>
                        <th className="px-3 py-2">Base URL</th>
                        <th className="px-3 py-2">Reason</th>
                      </tr>
                    </thead>
                    <tbody>
                      {backend.nodes.map((row) => (
                        <tr
                          key={row.node_uuid}
                          className="border-t border-[hsl(var(--border))]"
                        >
                          <td className="px-3 py-2 font-mono text-xs">
                            <Link
                              to={`/nodes/${encodeURIComponent(row.node_uuid)}`}
                              className="hover:underline"
                            >
                              {row.node_uuid}
                            </Link>
                          </td>
                          <td className="px-3 py-2">
                            <StatusBadge status={row.status} />
                          </td>
                          <td className="px-3 py-2 text-xs text-[hsl(var(--muted-foreground))]">
                            {row.weight ?? '-'}
                          </td>
                          <td className="px-3 py-2 text-xs text-[hsl(var(--muted-foreground))]">
                            {row.priority ?? '-'}
                          </td>
                          <td className="px-3 py-2 font-mono text-xs">
                            {row.base_url || '-'}
                          </td>
                          <td className="px-3 py-2 text-xs text-[hsl(var(--muted-foreground))]">
                            {row.status_reason || '-'}
                          </td>
                        </tr>
                      ))}
                    </tbody>
                  </table>
                </div>
              ) : (
                <div className="text-sm text-[hsl(var(--muted-foreground))]">No nodes.</div>
              )}
            </CardContent>
          </Card>

          <div className="flex items-center justify-between">
            <div className="text-sm text-[hsl(var(--muted-foreground))]">Raw JSON</div>
          </div>
          <pre className="max-h-[520px] overflow-auto rounded-[var(--radius)] border border-[hsl(var(--border))] bg-[hsl(var(--secondary))] p-3 text-xs">
            {JSON.stringify(backend, null, 2)}
          </pre>
        </div>
      )}
    </div>
  )
}

