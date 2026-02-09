import { Link, useParams } from 'react-router-dom'
import { useQuery } from '@tanstack/react-query'
import { Copy } from 'lucide-react'
import { toast } from 'sonner'

import { getAiModelDetail } from '@/api/ai-models'
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

export default function AiModelDetailPage() {
  const { hosting, provider, model } = useParams()
  const h = (hosting || '') as 'local' | 'remote' | ''
  const p = provider || ''
  const m = model || ''

  const q = useQuery({
    queryKey: ['ai-model-detail', h, p, m],
    queryFn: () => getAiModelDetail({ hosting: h || undefined, provider: p, model: m }),
    enabled: !!p && !!m,
    refetchInterval: 15_000,
  })

  const info = q.data?.model
  const title = info ? `${info.provider} / ${info.model}` : 'AI Model'

  return (
    <div className="space-y-4">
      <div className="flex items-center justify-between">
        <div>
          <div className="text-lg font-semibold">{title}</div>
          <div className="text-sm text-[hsl(var(--muted-foreground))]">
            <Link to={`/ai-models/${encodeURIComponent(h || 'remote')}`} className="hover:underline">
              AI Models
            </Link>
            <span className="mx-2">/</span>
            <span className="font-mono text-xs">{h || '-'}</span>
            <span className="mx-2">/</span>
            <span className="font-mono text-xs">{p}</span>
            <span className="mx-2">/</span>
            <span className="font-mono text-xs">{m}</span>
          </div>
        </div>
        <div className="flex items-center gap-2">
          <Button variant="secondary" onClick={() => q.refetch()}>
            Refresh
          </Button>
          {info ? <CopyButton value={`${info.provider}:${info.model}`} /> : null}
        </div>
      </div>

      {q.isLoading ? (
        <div className="text-sm text-[hsl(var(--muted-foreground))]">Loading…</div>
      ) : q.isError ? (
        <div className="text-sm text-[hsl(var(--muted-foreground))]">Failed to load.</div>
      ) : !q.data?.success ? (
        <div className="text-sm text-[hsl(var(--muted-foreground))]">
          {q.data?.message || 'Failed to load.'}
        </div>
      ) : !q.data.found || !info ? (
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
                  <StatusBadge status={info.available_nodes > 0 ? 'available' : 'unavailable'} />
                </div>
                <div className="flex items-center justify-between text-sm">
                  <span className="text-[hsl(var(--muted-foreground))]">Available</span>
                  <span>
                    {info.available_nodes}/{info.total_nodes}
                  </span>
                </div>
                <div className="flex items-center justify-between text-sm">
                  <span className="text-[hsl(var(--muted-foreground))]">Hosting</span>
                  <span className="font-mono text-xs">{info.hosting}</span>
                </div>
                <div className="flex items-center justify-between text-sm">
                  <span className="text-[hsl(var(--muted-foreground))]">Provider</span>
                  <span className="font-mono text-xs">{info.provider}</span>
                </div>
                <div className="flex items-center justify-between text-sm">
                  <span className="text-[hsl(var(--muted-foreground))]">Model</span>
                  <span className="font-mono text-xs">{info.model}</span>
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
                    {(info.operations || []).map((op) => (
                      <Badge key={op} variant="secondary">
                        {op}
                      </Badge>
                    ))}
                  </div>
                </div>
                <div>
                  <div className="text-xs text-[hsl(var(--muted-foreground))]">Transports</div>
                  <div className="mt-1 flex flex-wrap gap-1">
                    {(info.transports || []).map((t) => (
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
              <CardTitle>Instances</CardTitle>
            </CardHeader>
            <CardContent>
              {info.instances && info.instances.length > 0 ? (
                <div className="overflow-auto rounded-[var(--radius)] border border-[hsl(var(--border))]">
                  <table className="w-full text-sm">
                    <thead className="bg-[hsl(var(--muted))] text-left text-xs text-[hsl(var(--muted-foreground))]">
                      <tr>
                        <th className="px-3 py-2">Node</th>
                        <th className="px-3 py-2">Status</th>
                        <th className="px-3 py-2">Backend</th>
                        <th className="px-3 py-2">Kind</th>
                        <th className="px-3 py-2">Base URL</th>
                        <th className="px-3 py-2">Reason</th>
                      </tr>
                    </thead>
                    <tbody>
                      {info.instances.map((row, idx) => (
                        <tr key={`${row.node_uuid}-${idx}`} className="border-t border-[hsl(var(--border))]">
                          <td className="px-3 py-2 font-mono text-xs">
                            <Link to={`/nodes/${encodeURIComponent(row.node_uuid)}`} className="hover:underline">
                              {row.node_uuid}
                            </Link>
                          </td>
                          <td className="px-3 py-2">
                            <StatusBadge status={row.status} />
                          </td>
                          <td className="px-3 py-2 font-mono text-xs">{row.backend_name}</td>
                          <td className="px-3 py-2 font-mono text-xs">{row.kind}</td>
                          <td className="px-3 py-2 font-mono text-xs">{row.base_url || '-'}</td>
                          <td className="px-3 py-2 text-xs text-[hsl(var(--muted-foreground))]">
                            {row.status_reason || '-'}
                          </td>
                        </tr>
                      ))}
                    </tbody>
                  </table>
                </div>
              ) : (
                <div className="text-sm text-[hsl(var(--muted-foreground))]">No instances.</div>
              )}
            </CardContent>
          </Card>

          <div className="flex items-center justify-between">
            <div className="text-sm text-[hsl(var(--muted-foreground))]">Raw JSON</div>
          </div>
          <pre className="max-h-[520px] overflow-auto rounded-[var(--radius)] border border-[hsl(var(--border))] bg-[hsl(var(--secondary))] p-3 text-xs">
            {JSON.stringify(info, null, 2)}
          </pre>
        </div>
      )}
    </div>
  )
}

