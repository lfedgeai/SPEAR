import { useMemo, useState } from 'react'
import { useQuery } from '@tanstack/react-query'
import { Copy, Search } from 'lucide-react'
import { toast } from 'sonner'

import { listBackends, type AggregatedBackend } from '@/api/backends'
import { Badge } from '@/components/ui/badge'
import { Button } from '@/components/ui/button'
import { Card, CardContent, CardHeader, CardTitle } from '@/components/ui/card'
import { Dialog, DialogContent, DialogHeader } from '@/components/ui/dialog'
import { Input } from '@/components/ui/input'

function StatusBadge({ status }: { status: 'available' | 'unavailable' }) {
  if (status === 'available') return <Badge>{status}</Badge>
  return <Badge variant="destructive">{status}</Badge>
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

export default function BackendsPage() {
  const [q, setQ] = useState('')
  const [status, setStatus] = useState<'available' | 'unavailable' | ''>('')
  const [selected, setSelected] = useState<AggregatedBackend | null>(null)
  const [detailOpen, setDetailOpen] = useState(false)

  const query = useQuery({
    queryKey: ['backends', q, status],
    queryFn: () =>
      listBackends({
        q: q || undefined,
        status: (status || undefined) as 'available' | 'unavailable' | undefined,
        limit: 500,
        offset: 0,
      }),
    refetchInterval: 15_000,
  })

  const rows = useMemo(() => query.data?.backends || [], [query.data?.backends])
  const total = query.data?.total_count ?? rows.length
  const title = useMemo(() => `Backends (${rows.length}/${total})`, [rows.length, total])

  return (
    <div className="space-y-4">
      <div className="flex items-center justify-between">
        <div>
          <div className="text-lg font-semibold">Backends</div>
          <div className="text-sm text-[hsl(var(--muted-foreground))]">Available backends</div>
        </div>
        <div className="flex items-center gap-2">
          <Button
            variant="secondary"
            onClick={() => {
              query
                .refetch()
                .then(() => toast.success('Refreshed'))
                .catch((e) => toast.error((e as Error).message))
            }}
          >
            Refresh
          </Button>
        </div>
      </div>

      <Card>
        <CardHeader>
          <CardTitle>{title}</CardTitle>
        </CardHeader>
        <CardContent>
          <div className="mb-3 flex items-center gap-2">
            <div className="relative w-full max-w-md">
              <Search className="absolute left-2 top-2.5 h-4 w-4 text-[hsl(var(--muted-foreground))]" />
              <Input
                placeholder="Search name/kind"
                value={q}
                onChange={(e) => setQ(e.target.value)}
                className="pl-8"
              />
            </div>
            <select
              className="h-9 rounded-[calc(var(--radius)-4px)] border border-[hsl(var(--input))] bg-[hsl(var(--background))] px-3 text-sm"
              value={status}
              onChange={(e) => setStatus(e.target.value as typeof status)}
              aria-label="Status"
            >
              <option value="">All</option>
              <option value="available">available</option>
              <option value="unavailable">unavailable</option>
            </select>
          </div>

          {query.isError ? (
            <div className="text-sm text-[hsl(var(--destructive))]">{(query.error as Error).message}</div>
          ) : null}

          <div className="overflow-auto rounded-[var(--radius)] border border-[hsl(var(--border))]">
            <table className="w-full text-sm">
              <thead className="bg-[hsl(var(--secondary))]">
                <tr className="text-left">
                  <th className="px-3 py-2">Name</th>
                  <th className="px-3 py-2">Kind</th>
                  <th className="px-3 py-2">Ops</th>
                  <th className="px-3 py-2">Transports</th>
                  <th className="px-3 py-2">Available</th>
                </tr>
              </thead>
              <tbody>
                {rows.map((b) => (
                  <tr
                    key={`${b.name}::${b.kind}`}
                    className="cursor-pointer border-t border-[hsl(var(--border))] hover:bg-[hsl(var(--accent))]"
                    onClick={() => {
                      setSelected(b)
                      setDetailOpen(true)
                    }}
                  >
                    <td className="px-3 py-2 font-medium">{b.name}</td>
                    <td className="px-3 py-2 text-[hsl(var(--muted-foreground))]">{b.kind}</td>
                    <td className="px-3 py-2">
                      <div className="flex flex-wrap gap-1">
                        {(b.operations || []).slice(0, 6).map((op) => (
                          <Badge key={op} variant="secondary">
                            {op}
                          </Badge>
                        ))}
                        {(b.operations || []).length > 6 ? <Badge variant="secondary">...</Badge> : null}
                      </div>
                    </td>
                    <td className="px-3 py-2">
                      <div className="flex flex-wrap gap-1">
                        {(b.transports || []).map((t) => (
                          <Badge key={t} variant="secondary">
                            {t}
                          </Badge>
                        ))}
                      </div>
                    </td>
                    <td className="px-3 py-2">
                      <div className="flex items-center gap-2">
                        <StatusBadge status={b.available_nodes > 0 ? 'available' : 'unavailable'} />
                        <span className="text-[hsl(var(--muted-foreground))]">
                          {b.available_nodes}/{b.total_nodes}
                        </span>
                      </div>
                    </td>
                  </tr>
                ))}
              </tbody>
            </table>
          </div>
        </CardContent>
      </Card>

      <Dialog open={detailOpen} onOpenChange={setDetailOpen}>
        <DialogContent>
          <DialogHeader
            title={selected ? selected.name || 'Backend detail' : 'Backend detail'}
            description={selected ? selected.kind : undefined}
          />
          {selected ? (
            <div className="space-y-4">
              <div className="grid grid-cols-2 gap-3">
                <Card>
                  <CardHeader>
                    <CardTitle>Summary</CardTitle>
                  </CardHeader>
                  <CardContent className="space-y-2">
                    <div className="flex items-center justify-between text-sm">
                      <span className="text-[hsl(var(--muted-foreground))]">Available</span>
                      <span>
                        {selected.available_nodes}/{selected.total_nodes}
                      </span>
                    </div>
                    <div className="flex items-center justify-between text-sm">
                      <span className="text-[hsl(var(--muted-foreground))]">Ops</span>
                      <span>{selected.operations?.length ?? 0}</span>
                    </div>
                    <div className="flex items-center justify-between text-sm">
                      <span className="text-[hsl(var(--muted-foreground))]">Transports</span>
                      <span>{selected.transports?.length ?? 0}</span>
                    </div>
                    <div className="flex items-center justify-between text-sm">
                      <span className="text-[hsl(var(--muted-foreground))]">Nodes</span>
                      <span>{selected.nodes?.length ?? 0}</span>
                    </div>
                  </CardContent>
                </Card>

                <Card>
                  <CardHeader>
                    <CardTitle>Routing</CardTitle>
                  </CardHeader>
                  <CardContent className="space-y-2">
                    <div className="flex items-center justify-between text-sm">
                      <span className="text-[hsl(var(--muted-foreground))]">Status</span>
                      <StatusBadge
                        status={selected.available_nodes > 0 ? 'available' : 'unavailable'}
                      />
                    </div>
                    <div className="flex items-center justify-between text-sm">
                      <span className="text-[hsl(var(--muted-foreground))]">Name</span>
                      <span className="truncate">{selected.name}</span>
                    </div>
                    <div className="flex items-center justify-between text-sm">
                      <span className="text-[hsl(var(--muted-foreground))]">Kind</span>
                      <span className="truncate">{selected.kind}</span>
                    </div>
                  </CardContent>
                </Card>
              </div>

              <div className="flex items-center justify-between">
                <div className="text-sm text-[hsl(var(--muted-foreground))]">Raw JSON</div>
                {selected.name ? <CopyButton value={selected.name} /> : null}
              </div>
              <pre className="max-h-[260px] overflow-auto rounded-[var(--radius)] border border-[hsl(var(--border))] bg-[hsl(var(--secondary))] p-3 text-xs">
                {JSON.stringify(selected, null, 2)}
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
