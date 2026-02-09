import { useEffect, useMemo, useState } from 'react'
import { useQueries, useQuery } from '@tanstack/react-query'

import { listNodes } from '@/api/nodes'
import { listNodeModelDeployments, type ModelDeploymentRecord } from '@/api/model-deployments'
import { Badge } from '@/components/ui/badge'
import { Button } from '@/components/ui/button'
import { Card, CardContent, CardHeader, CardTitle } from '@/components/ui/card'
import { cn } from '@/lib/utils'

function phaseLabel(phase: number): string {
  switch (phase) {
    case 1:
      return 'pending'
    case 2:
      return 'pulling'
    case 3:
      return 'starting'
    case 4:
      return 'ready'
    case 5:
      return 'failed'
    case 6:
      return 'deleting'
    default:
      return 'unspecified'
  }
}

function PhaseBadge({ phase }: { phase: number }) {
  const label = phaseLabel(phase)
  if (phase === 4) return <Badge variant="success">{label}</Badge>
  if (phase === 5) return <Badge variant="destructive">{label}</Badge>
  return <Badge variant="secondary">{label}</Badge>
}

function sortKey(r: ModelDeploymentRecord) {
  const t = r.status?.updated_at_ms ?? r.updated_at_ms ?? r.created_at_ms ?? 0
  return -t
}

export default function LocalDeploymentsPanel(props: {
  focusDeploymentId?: string
  defaultOpen?: boolean
  onClearFocus?: () => void
}) {
  const forcedOpen = !!props.focusDeploymentId || !!props.defaultOpen
  const [open, setOpen] = useState(false)
  const effectiveOpen = forcedOpen || open

  const nodesQuery = useQuery({
    queryKey: ['nodes', 'for-local-deployments-panel'],
    queryFn: () => listNodes({ limit: 500 }),
    refetchInterval: 30_000,
  })
  const nodes = useMemo(() => nodesQuery.data?.nodes ?? [], [nodesQuery.data?.nodes])

  const perNode = useQueries({
    queries: nodes.map((n) => ({
      queryKey: ['node-model-deployments', n.uuid],
      queryFn: () => listNodeModelDeployments({ node_uuid: n.uuid, limit: 200, offset: 0 }),
      enabled: nodes.length > 0,
      refetchInterval: effectiveOpen ? 3_000 : false,
    })),
  })

  const loading = nodesQuery.isLoading || perNode.some((q) => q.isLoading)
  const err = (nodesQuery.error as Error | undefined) || (perNode.find((q) => q.isError)?.error as Error | undefined)

  const all = useMemo(() => {
    const out: (ModelDeploymentRecord & { node_uuid: string })[] = []
    for (let i = 0; i < nodes.length; i++) {
      const n = nodes[i]
      const q = perNode[i]
      const deployments = q?.data?.deployments || []
      for (const d of deployments) {
        out.push({ ...d, node_uuid: n.uuid })
      }
    }
    return out
  }, [nodes, perNode])

  const nonReady = useMemo(() => {
    return all
      .filter((d) => (d.status?.phase ?? 0) !== 4)
      .sort((a, b) => sortKey(a) - sortKey(b))
  }, [all])

  const provisioningCount = useMemo(() => {
    return nonReady.filter((d) => {
      const p = d.status?.phase ?? 0
      return p !== 0 && p !== 5
    }).length
  }, [nonReady])

  const failedCount = useMemo(() => nonReady.filter((d) => (d.status?.phase ?? 0) === 5).length, [nonReady])

  useEffect(() => {
    if (!props.focusDeploymentId) return
    if (!effectiveOpen) return
    const el = document.getElementById(`deployment-${props.focusDeploymentId}`)
    if (el) el.scrollIntoView({ block: 'center' })
  }, [effectiveOpen, nonReady.length, props.focusDeploymentId])

  return (
    <Card>
      <CardHeader className="flex flex-row items-center justify-between gap-3">
        <div className="min-w-0">
          <CardTitle className="flex items-center gap-2">
            <span>Provisioning</span>
            <span className="text-sm font-normal text-[hsl(var(--muted-foreground))]">
              {provisioningCount} running / {failedCount} failed
            </span>
          </CardTitle>
        </div>
        <div className="flex items-center gap-2">
          <Button
            variant="secondary"
            size="sm"
            onClick={() => {
              if (forcedOpen) {
                props.onClearFocus?.()
                return
              }
              setOpen((v) => !v)
            }}
          >
            {effectiveOpen ? 'Hide' : 'Show'}
          </Button>
        </div>
      </CardHeader>
      {effectiveOpen ? (
        <CardContent>
          {err ? (
            <div className="rounded-[var(--radius)] border border-[hsl(var(--border))] bg-[hsl(var(--secondary))] px-3 py-2 text-sm text-[hsl(var(--destructive))]">
              {(err as Error).message}
            </div>
          ) : loading ? (
            <div className="rounded-[var(--radius)] border border-[hsl(var(--border))] bg-[hsl(var(--secondary))] px-3 py-2 text-sm text-[hsl(var(--muted-foreground))]">
              Loading deployments…
            </div>
          ) : nonReady.length === 0 ? (
            <div className="rounded-[var(--radius)] border border-[hsl(var(--border))] bg-[hsl(var(--secondary))] px-3 py-2 text-sm text-[hsl(var(--muted-foreground))]">
              No provisioning deployments.
            </div>
          ) : (
            <div className="overflow-auto rounded-[var(--radius)] border border-[hsl(var(--border))]">
              <table className="w-full text-sm">
                <thead className="bg-[hsl(var(--secondary))]">
                  <tr className="text-left text-xs text-[hsl(var(--muted-foreground))]">
                    <th className="px-3 py-2">Node</th>
                    <th className="px-3 py-2">Provider</th>
                    <th className="px-3 py-2">Model</th>
                    <th className="px-3 py-2">Phase</th>
                    <th className="px-3 py-2">Message</th>
                  </tr>
                </thead>
                <tbody>
                  {nonReady.map((d) => {
                    const provider = d.spec?.provider || '-'
                    const model = d.spec?.model || '-'
                    const phase = d.status?.phase ?? 0
                    const msg = d.status?.message || ''
                    const focused = props.focusDeploymentId === d.deployment_id
                    return (
                      <tr
                        key={d.deployment_id}
                        id={`deployment-${d.deployment_id}`}
                        className={cn(
                          'border-t border-[hsl(var(--border))] align-top',
                          focused ? 'bg-[hsl(var(--accent))]' : '',
                        )}
                      >
                        <td className="px-3 py-2 font-mono text-xs">{d.node_uuid}</td>
                        <td className="px-3 py-2 font-medium">{provider}</td>
                        <td className="px-3 py-2 font-mono text-xs">{model}</td>
                        <td className="px-3 py-2">
                          <PhaseBadge phase={phase} />
                        </td>
                        <td className="px-3 py-2 text-xs text-[hsl(var(--muted-foreground))]">
                          {msg}
                        </td>
                      </tr>
                    )
                  })}
                </tbody>
              </table>
            </div>
          )}
        </CardContent>
      ) : null}
    </Card>
  )
}
