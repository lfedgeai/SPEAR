import { useMemo, useState } from 'react'
import { Link, useParams } from 'react-router-dom'
import { useQuery } from '@tanstack/react-query'
import { Trash2 } from 'lucide-react'
import { toast } from 'sonner'

import { deleteMcpServer, getMcpServer, type McpServer } from '@/api/mcp'
import { Button } from '@/components/ui/button'
import { Card, CardContent, CardHeader, CardTitle } from '@/components/ui/card'
import McpEditDialog from '@/features/mcp/McpEditDialog'

export default function McpServerDetailPage() {
  const { serverId } = useParams()
  const id = serverId || ''

  const [editOpen, setEditOpen] = useState(false)

  const q = useQuery({
    queryKey: ['mcp-server', id],
    queryFn: () => getMcpServer(id),
    enabled: !!id,
    refetchInterval: 15_000,
  })

  const server: McpServer | null = useMemo(() => {
    if (!q.data?.success) return null
    if (!q.data.found) return null
    return q.data.server || null
  }, [q.data])

  return (
    <div className="space-y-4">
      <div className="flex items-center justify-between">
        <div>
          <div className="text-lg font-semibold">{server?.display_name || 'MCP Server'}</div>
          <div className="text-sm text-[hsl(var(--muted-foreground))]">
            <Link to="/mcp" className="hover:underline">
              MCP
            </Link>
            <span className="mx-2">/</span>
            <span className="font-mono text-xs">{id}</span>
          </div>
        </div>
        <div className="flex items-center gap-2">
          <Button variant="secondary" onClick={() => q.refetch()}>
            Refresh
          </Button>
          <Button
            variant="secondary"
            onClick={() => setEditOpen(true)}
            disabled={!server}
          >
            Edit
          </Button>
          <Button
            variant="destructive"
            onClick={async () => {
              try {
                const resp = await deleteMcpServer(id)
                if (!resp.success) {
                  toast.error(resp.message || 'delete failed')
                  return
                }
                toast.success('Deleted')
                window.location.hash = '#/mcp'
              } catch (e: unknown) {
                toast.error((e as Error).message || 'delete failed')
              }
            }}
          >
            <Trash2 className="h-4 w-4" />
            Delete
          </Button>
        </div>
      </div>

      {q.isLoading ? (
        <div className="text-sm text-[hsl(var(--muted-foreground))]">Loading…</div>
      ) : q.isError ? (
        <div className="text-sm text-[hsl(var(--muted-foreground))]">
          Failed to load MCP server.
        </div>
      ) : !q.data?.success ? (
        <div className="text-sm text-[hsl(var(--muted-foreground))]">
          {q.data?.message || 'Failed to load MCP server.'}
        </div>
      ) : !server ? (
        <div className="text-sm text-[hsl(var(--muted-foreground))]">Not found</div>
      ) : (
        <div className="space-y-4">
          <div className="grid grid-cols-2 gap-3">
            <Card>
              <CardHeader>
                <CardTitle>Summary</CardTitle>
              </CardHeader>
              <CardContent className="space-y-2 text-sm">
                <div className="flex items-center justify-between">
                  <span className="text-[hsl(var(--muted-foreground))]">server_id</span>
                  <span className="font-mono text-xs">{server.server_id}</span>
                </div>
                <div className="flex items-center justify-between">
                  <span className="text-[hsl(var(--muted-foreground))]">transport</span>
                  <span>{server.transport}</span>
                </div>
                <div className="flex items-center justify-between">
                  <span className="text-[hsl(var(--muted-foreground))]">tool_namespace</span>
                  <span className="font-mono text-xs">{server.tool_namespace || '-'}</span>
                </div>
                <div className="flex items-center justify-between">
                  <span className="text-[hsl(var(--muted-foreground))]">updated_at_ms</span>
                  <span className="font-mono text-xs">{server.updated_at_ms || '-'}</span>
                </div>
              </CardContent>
            </Card>

            <Card>
              <CardHeader>
                <CardTitle>Stdio</CardTitle>
              </CardHeader>
              <CardContent className="space-y-2 text-sm">
                <div className="flex items-center justify-between">
                  <span className="text-[hsl(var(--muted-foreground))]">command</span>
                  <span className="font-mono text-xs">{server.stdio?.command || '-'}</span>
                </div>
                <div className="flex items-center justify-between">
                  <span className="text-[hsl(var(--muted-foreground))]">cwd</span>
                  <span className="font-mono text-xs">{server.stdio?.cwd || '-'}</span>
                </div>
                <div className="flex items-center justify-between">
                  <span className="text-[hsl(var(--muted-foreground))]">args</span>
                  <span className="font-mono text-xs">
                    {(server.stdio?.args || []).join(',') || '-'}
                  </span>
                </div>
              </CardContent>
            </Card>
          </div>

          <Card>
            <CardHeader>
              <CardTitle>Raw JSON</CardTitle>
            </CardHeader>
            <CardContent>
              <pre className="max-h-[620px] overflow-auto rounded-[var(--radius)] border border-[hsl(var(--border))] bg-[hsl(var(--secondary))] p-3 text-xs">
                {JSON.stringify(server, null, 2)}
              </pre>
            </CardContent>
          </Card>
        </div>
      )}

      <McpEditDialog
        open={editOpen}
        onOpenChange={setEditOpen}
        initial={server}
        onSaved={() => {
          q.refetch()
        }}
      />
    </div>
  )
}
