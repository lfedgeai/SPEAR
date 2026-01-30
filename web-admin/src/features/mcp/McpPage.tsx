import { useMemo, useState } from 'react'
import { useQuery } from '@tanstack/react-query'
import { Plus, Trash2 } from 'lucide-react'
import { useNavigate } from 'react-router-dom'
import { toast } from 'sonner'

import { deleteMcpServer, listMcpServers, type McpServer } from '@/api/mcp'
import { Button } from '@/components/ui/button'
import { Card, CardContent, CardHeader, CardTitle } from '@/components/ui/card'
import McpEditDialog from '@/features/mcp/McpEditDialog'

function errMsg(e: unknown) {
  if (e && typeof e === 'object' && 'message' in e) {
    const msg = (e as { message?: unknown }).message
    if (typeof msg === 'string' && msg) return msg
  }
  return 'request failed'
}

export default function McpPage() {
  const navigate = useNavigate()
  const q = useQuery({
    queryKey: ['mcp-servers'],
    queryFn: listMcpServers,
    staleTime: 10_000,
  })

  const servers = useMemo(() => {
    if (!q.data?.success) return []
    return q.data.servers ?? []
  }, [q.data])

  const [editOpen, setEditOpen] = useState(false)
  const [editing, setEditing] = useState<McpServer | null>(null)

  async function onDelete(serverId: string) {
    try {
      const resp = await deleteMcpServer(serverId)
      if (!resp.success) {
        toast.error(resp.message || 'delete failed')
        return
      }
      toast.success('Deleted')
      q.refetch()
    } catch (e: unknown) {
      toast.error(errMsg(e) || 'delete failed')
    }
  }

  return (
    <div className="space-y-4">
      <Card>
        <CardHeader className="flex flex-row items-center justify-between">
          <CardTitle>MCP Servers</CardTitle>
          <Button
            size="sm"
            onClick={() => {
              setEditing(null)
              setEditOpen(true)
            }}
          >
            <Plus className="h-4 w-4" />
            Add
          </Button>
        </CardHeader>
        <CardContent>
          {!q.data?.success && q.data?.message ? (
            <div className="text-sm text-[hsl(var(--muted-foreground))]">{q.data.message}</div>
          ) : null}
          <div className="overflow-auto rounded-[var(--radius)] border border-[hsl(var(--border))]">
            <table className="w-full text-sm">
              <thead className="bg-[hsl(var(--secondary))]">
                <tr className="text-left">
                  <th className="px-3 py-2">server_id</th>
                  <th className="px-3 py-2">display</th>
                  <th className="px-3 py-2">transport</th>
                  <th className="px-3 py-2">command</th>
                  <th className="px-3 py-2">allowed_tools</th>
                  <th className="px-3 py-2">actions</th>
                </tr>
              </thead>
              <tbody>
                {servers.map((s) => (
                  <tr
                    key={s.server_id}
                    className="cursor-pointer border-t border-[hsl(var(--border))] hover:bg-[hsl(var(--accent))]"
                    onClick={() => navigate(`/mcp/${encodeURIComponent(s.server_id)}`)}
                  >
                    <td className="px-3 py-2 font-mono text-xs">{s.server_id}</td>
                    <td className="px-3 py-2">{s.display_name || '-'}</td>
                    <td className="px-3 py-2">{s.transport}</td>
                    <td className="px-3 py-2 font-mono text-xs">{s.stdio?.command || '-'}</td>
                    <td className="px-3 py-2 font-mono text-xs">
                      {(s.allowed_tools ?? []).join(',') || '-'}
                    </td>
                    <td className="px-3 py-2">
                      <div className="flex items-center gap-2">
                        <Button
                          variant="secondary"
                          size="sm"
                          onClick={(e) => {
                            e.preventDefault()
                            e.stopPropagation()
                            setEditing(s)
                            setEditOpen(true)
                          }}
                        >
                          Quick edit
                        </Button>
                        <Button
                          variant="destructive"
                          size="sm"
                          onClick={(e) => {
                            e.preventDefault()
                            e.stopPropagation()
                            onDelete(s.server_id)
                          }}
                        >
                          <Trash2 className="h-4 w-4" />
                          Delete
                        </Button>
                      </div>
                    </td>
                  </tr>
                ))}
                {servers.length === 0 ? (
                  <tr>
                    <td
                      colSpan={6}
                      className="px-3 py-6 text-center text-sm text-[hsl(var(--muted-foreground))]"
                    >
                      No servers
                    </td>
                  </tr>
                ) : null}
              </tbody>
            </table>
          </div>
        </CardContent>
      </Card>

      <McpEditDialog
        open={editOpen}
        onOpenChange={setEditOpen}
        initial={editing}
        onSaved={() => q.refetch()}
      />
    </div>
  )
}
