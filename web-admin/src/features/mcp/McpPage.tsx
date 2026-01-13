import { useMemo, useState } from 'react'
import { useQuery } from '@tanstack/react-query'
import { Plus, Trash2 } from 'lucide-react'
import { toast } from 'sonner'

import { deleteMcpServer, listMcpServers, upsertMcpServer, type McpServer } from '@/api/mcp'
import { Button } from '@/components/ui/button'
import { Card, CardContent, CardHeader, CardTitle } from '@/components/ui/card'
import { Dialog, DialogContent, DialogHeader } from '@/components/ui/dialog'
import { Input } from '@/components/ui/input'

type FormState = {
  server_id: string
  display_name: string
  command: string
  args: string
  cwd: string
  env: string
  allowed_tools: string
}

function parseEnvLines(s: string) {
  const out: Record<string, string> = {}
  for (const line of s.split('\n')) {
    const t = line.trim()
    if (!t) continue
    const idx = t.indexOf('=')
    if (idx <= 0) continue
    out[t.slice(0, idx).trim()] = t.slice(idx + 1).trim()
  }
  return out
}

function parseCommaList(s: string) {
  return s
    .split(',')
    .map((x) => x.trim())
    .filter(Boolean)
}

function errMsg(e: unknown) {
  if (e && typeof e === 'object' && 'message' in e) {
    const msg = (e as { message?: unknown }).message
    if (typeof msg === 'string' && msg) return msg
  }
  return 'request failed'
}

function fmtArgs(server: McpServer) {
  return (server.stdio?.args ?? []).join(',')
}

function fmtEnv(server: McpServer) {
  const env = server.stdio?.env ?? {}
  return Object.entries(env)
    .map(([k, v]) => `${k}=${v}`)
    .join('\n')
}

export default function McpPage() {
  const q = useQuery({
    queryKey: ['mcp-servers'],
    queryFn: listMcpServers,
    staleTime: 10_000,
  })

  const servers = useMemo(() => {
    if (!q.data?.success) return []
    return q.data.servers ?? []
  }, [q.data])

  const [open, setOpen] = useState(false)
  const [submitting, setSubmitting] = useState(false)
  const [form, setForm] = useState<FormState>({
    server_id: '',
    display_name: '',
    command: '',
    args: '',
    cwd: '',
    env: '',
    allowed_tools: '',
  })

  function fillFromServer(s: McpServer) {
    setForm({
      server_id: s.server_id,
      display_name: s.display_name ?? '',
      command: s.stdio?.command ?? '',
      args: fmtArgs(s),
      cwd: s.stdio?.cwd ?? '',
      env: fmtEnv(s),
      allowed_tools: (s.allowed_tools ?? []).join(','),
    })
    setOpen(true)
  }

  async function onSubmit() {
    if (!form.server_id.trim()) {
      toast.error('server_id is required')
      return
    }
    if (!form.command.trim()) {
      toast.error('command is required')
      return
    }

    setSubmitting(true)
    try {
      const resp = await upsertMcpServer({
        server_id: form.server_id.trim(),
        display_name: form.display_name.trim() || undefined,
        transport: 'stdio',
        stdio: {
          command: form.command.trim(),
          args: parseCommaList(form.args),
          cwd: form.cwd.trim() || undefined,
          env: parseEnvLines(form.env),
        },
        allowed_tools: parseCommaList(form.allowed_tools),
      })
      if (!resp.success) {
        toast.error(resp.message || 'upsert failed')
        return
      }
      toast.success('Saved')
      setOpen(false)
      q.refetch()
    } catch (e: unknown) {
      toast.error(errMsg(e) || 'upsert failed')
    } finally {
      setSubmitting(false)
    }
  }

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
              setForm({
                server_id: '',
                display_name: '',
                command: '',
                args: '',
                cwd: '',
                env: '',
                allowed_tools: '',
              })
              setOpen(true)
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
                  <tr key={s.server_id} className="border-t border-[hsl(var(--border))]">
                    <td className="px-3 py-2 font-mono text-xs">{s.server_id}</td>
                    <td className="px-3 py-2">{s.display_name || '-'}</td>
                    <td className="px-3 py-2">{s.transport}</td>
                    <td className="px-3 py-2 font-mono text-xs">{s.stdio?.command || '-'}</td>
                    <td className="px-3 py-2 font-mono text-xs">
                      {(s.allowed_tools ?? []).join(',') || '-'}
                    </td>
                    <td className="px-3 py-2">
                      <div className="flex items-center gap-2">
                        <Button variant="secondary" size="sm" onClick={() => fillFromServer(s)}>
                          Edit
                        </Button>
                        <Button
                          variant="destructive"
                          size="sm"
                          onClick={() => onDelete(s.server_id)}
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

      <Dialog open={open} onOpenChange={setOpen}>
        <DialogContent>
          <DialogHeader title={form.server_id ? 'Edit MCP Server' : 'Add MCP Server'} />

          <div className="grid grid-cols-2 gap-3">
            <div className="col-span-2">
              <Input
                placeholder="server_id"
                value={form.server_id}
                onChange={(e) => setForm((p) => ({ ...p, server_id: e.target.value }))}
              />
            </div>
            <div className="col-span-2">
              <Input
                placeholder="display_name"
                value={form.display_name}
                onChange={(e) => setForm((p) => ({ ...p, display_name: e.target.value }))}
              />
            </div>
            <div className="col-span-2">
              <Input
                placeholder="stdio.command"
                value={form.command}
                onChange={(e) => setForm((p) => ({ ...p, command: e.target.value }))}
              />
            </div>
            <div className="col-span-2">
              <Input
                placeholder="stdio.args (comma separated)"
                value={form.args}
                onChange={(e) => setForm((p) => ({ ...p, args: e.target.value }))}
              />
            </div>
            <div className="col-span-2">
              <Input
                placeholder="stdio.cwd"
                value={form.cwd}
                onChange={(e) => setForm((p) => ({ ...p, cwd: e.target.value }))}
              />
            </div>
            <div className="col-span-2">
              <textarea
                className="min-h-24 w-full rounded-[calc(var(--radius)-4px)] border border-[hsl(var(--input))] bg-[hsl(var(--background))] px-3 py-2 text-sm"
                placeholder="stdio.env (one key=value per line)"
                value={form.env}
                onChange={(e) => setForm((p) => ({ ...p, env: e.target.value }))}
              />
            </div>
            <div className="col-span-2">
              <Input
                placeholder="allowed_tools (comma separated patterns; required)"
                value={form.allowed_tools}
                onChange={(e) => setForm((p) => ({ ...p, allowed_tools: e.target.value }))}
              />
            </div>
          </div>

          <div className="mt-4 flex justify-end gap-2">
            <Button variant="secondary" onClick={() => setOpen(false)}>
              Cancel
            </Button>
            <Button onClick={onSubmit} disabled={submitting}>
              Save
            </Button>
          </div>
        </DialogContent>
      </Dialog>
    </div>
  )
}
