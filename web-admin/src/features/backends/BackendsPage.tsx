import { useMemo, useState } from 'react'
import { useQuery } from '@tanstack/react-query'
import { Search } from 'lucide-react'
import { useNavigate } from 'react-router-dom'
import { toast } from 'sonner'

import { listBackends } from '@/api/backends'
import { Badge } from '@/components/ui/badge'
import { Button } from '@/components/ui/button'
import { Card, CardContent, CardHeader, CardTitle } from '@/components/ui/card'
import { Input } from '@/components/ui/input'

function StatusBadge({ status }: { status: 'available' | 'unavailable' }) {
  if (status === 'available') return <Badge>{status}</Badge>
  return <Badge variant="destructive">{status}</Badge>
}

export default function BackendsPage() {
  const navigate = useNavigate()
  const [q, setQ] = useState('')
  const [status, setStatus] = useState<'available' | 'unavailable' | ''>('')

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
                      navigate(
                        `/backends/${encodeURIComponent(b.kind)}/${encodeURIComponent(b.name)}`,
                      )
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
    </div>
  )
}
