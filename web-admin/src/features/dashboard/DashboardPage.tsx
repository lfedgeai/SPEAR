import type { ReactNode } from 'react'
import { useQuery } from '@tanstack/react-query'
import { Activity, Files, Server, Workflow } from 'lucide-react'

import { getStats } from '@/api/stats'
import { Button } from '@/components/ui/button'
import { Card, CardContent, CardHeader, CardTitle } from '@/components/ui/card'

function StatCard(props: {
  title: string
  value: number
  icon: ReactNode
}) {
  return (
    <Card>
      <CardHeader>
        <CardTitle>{props.title}</CardTitle>
      </CardHeader>
      <CardContent>
        <div className="flex items-center justify-between">
          <div className="text-2xl font-semibold tracking-tight">
            {Number.isFinite(props.value) ? props.value : '-'}
          </div>
          <div className="rounded-[calc(var(--radius)-2px)] bg-[hsl(var(--muted))] p-2 text-[hsl(var(--foreground))]">
            {props.icon}
          </div>
        </div>
      </CardContent>
    </Card>
  )
}

export default function DashboardPage() {
  const stats = useQuery({ queryKey: ['stats'], queryFn: getStats, refetchInterval: 15_000 })
  const s = stats.data || {
    total_count: 0,
    online_count: 0,
    offline_count: 0,
    recent_60s_count: 0,
  }

  return (
    <div className="space-y-6">
      <div className="flex items-center justify-between">
        <div>
          <div className="text-lg font-semibold">Dashboard</div>
          <div className="text-sm text-[hsl(var(--muted-foreground))]">
            Overview
          </div>
        </div>
        <Button variant="secondary" onClick={() => stats.refetch()}>
          Refresh
        </Button>
      </div>

      <div className="grid gap-4 md:grid-cols-2 lg:grid-cols-4">
        <StatCard title="Nodes" value={s.total_count} icon={<Server className="h-5 w-5" />} />
        <StatCard
          title="Online"
          value={s.online_count}
          icon={<Activity className="h-5 w-5" />}
        />
        <StatCard
          title="Offline"
          value={s.offline_count}
          icon={<Workflow className="h-5 w-5" />}
        />
        <StatCard
          title="Recent (60s)"
          value={s.recent_60s_count}
          icon={<Files className="h-5 w-5" />}
        />
      </div>

      <Card>
        <CardHeader>
          <CardTitle>Widgets</CardTitle>
        </CardHeader>
        <CardContent>
          <div className="rounded-[var(--radius)] border border-dashed border-[hsl(var(--border))] bg-[hsl(var(--background))] p-6 text-sm text-[hsl(var(--muted-foreground))]">
            Dashboard is widget-based. Add/remove widgets without changing layout.
          </div>
        </CardContent>
      </Card>
    </div>
  )
}
