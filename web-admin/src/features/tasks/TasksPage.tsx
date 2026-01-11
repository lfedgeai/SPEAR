import { useEffect, useMemo, useState } from 'react'
import { useQuery } from '@tanstack/react-query'
import { Plus, Search } from 'lucide-react'
import { toast } from 'sonner'

import { createTask, getTaskDetail, listTasks } from '@/api/tasks'
import { createExecution } from '@/api/executions'
import { listNodes } from '@/api/nodes'
import { listFiles } from '@/api/files'
import type { TaskSummary } from '@/api/types'
import { Badge } from '@/components/ui/badge'
import { Button } from '@/components/ui/button'
import { Card, CardContent, CardHeader, CardTitle } from '@/components/ui/card'
import { Dialog, DialogContent, DialogHeader } from '@/components/ui/dialog'
import { Input } from '@/components/ui/input'
import { cn } from '@/lib/utils'

function formatTs(ts: number) {
  if (!ts) return '-'
  const d = new Date(ts * 1000)
  return d.toLocaleString()
}

function StatusBadge({ status }: { status: string }) {
  const s = (status || '').toLowerCase()
  if (s === 'active' || s === 'registered') return <Badge>{status}</Badge>
  if (s === 'inactive') return <Badge variant="secondary">{status}</Badge>
  return <Badge variant="destructive">{status || 'unknown'}</Badge>
}

type CreateForm = {
  name: string
  description: string
  priority: string
  execution_kind: 'short_running' | 'long_running'
  node_uuid: string
  endpoint: string
  version: string
  capabilities: string
  executable_type: string
  executable_uri: string
  executable_name: string
  checksum: string
  args: string
  env: string
}

function endpointFromName(name: string) {
  const s = name
    .toLowerCase()
    .replace(/[^a-z0-9]+/g, '-')
    .replace(/^-+|-+$/g, '')
  return s ? `/tasks/${s}` : ''
}

type UriScheme = 'sms+file' | 'https' | 's3' | 'minio'

function schemePrefix(s: UriScheme) {
  if (s === 'https') return 'https://'
  if (s === 's3') return 's3://'
  if (s === 'minio') return 'minio://'
  return 'sms+file://'
}

function parseCsv(v: string) {
  return v
    .split(',')
    .map((s) => s.trim())
    .filter(Boolean)
}

function parseEnv(v: string) {
  const lines = v
    .split('\n')
    .map((s) => s.trim())
    .filter(Boolean)
  const out: Record<string, string> = {}
  for (const line of lines) {
    const i = line.indexOf('=')
    if (i > 0) out[line.slice(0, i).trim()] = line.slice(i + 1).trim()
  }
  return out
}

function CreateTaskDialog(props: {
  open: boolean
  onOpenChange: (v: boolean) => void
  onCreated: () => void
}) {
  const nodesQuery = useQuery({
    queryKey: ['nodes-for-task'],
    queryFn: () => listNodes({ limit: 200, sort_by: 'last_heartbeat', order: 'desc' }),
    staleTime: 15_000,
  })

  const [scheme, setScheme] = useState<UriScheme>('sms+file')
  const [pickerOpen, setPickerOpen] = useState(false)
  const [fileQ, setFileQ] = useState('')
  const [fileOffset, setFileOffset] = useState(0)
  const fileLimit = 60
  const [fileRows, setFileRows] = useState<import('@/api/types').FileItem[]>([])
  const [fileTotal, setFileTotal] = useState<number | null>(null)
  const filesQuery = useQuery({
    queryKey: ['files-for-task', fileQ, fileOffset, pickerOpen],
    queryFn: () => listFiles({ q: fileQ || undefined, limit: fileLimit, offset: fileOffset }),
    enabled: pickerOpen,
    staleTime: 5_000,
  })

  useEffect(() => {
    if (!pickerOpen) return
    if (!filesQuery.data) return
    setFileTotal(filesQuery.data.total_count ?? null)
    if (fileOffset === 0) {
      setFileRows(filesQuery.data.files || [])
    } else {
      setFileRows((cur) => [...cur, ...(filesQuery.data.files || [])])
    }
  }, [filesQuery.data, fileOffset, pickerOpen])

  useEffect(() => {
    if (!pickerOpen) return
    setFileOffset(0)
  }, [fileQ, pickerOpen])

  const [form, setForm] = useState<CreateForm>({
    name: '',
    description: '',
    priority: 'normal',
    execution_kind: 'short_running',
    node_uuid: '',
    endpoint: '',
    version: 'v1',
    capabilities: '',
    executable_type: 'no-executable',
    executable_uri: '',
    executable_name: '',
    checksum: '',
    args: '',
    env: '',
  })

  const [endpointTouched, setEndpointTouched] = useState(false)

  const nodes = nodesQuery.data?.nodes || []
  const canSubmit = form.name && form.endpoint && form.version

  const [runAfterCreate, setRunAfterCreate] = useState(true)

  return (
    <Dialog open={props.open} onOpenChange={props.onOpenChange}>
      <DialogContent>
        <DialogHeader title="Create task" description="Register a task in SMS" />
        <div className="grid grid-cols-2 gap-3">
          <div className="col-span-2">
            <Input
              placeholder="Name"
              value={form.name}
              onChange={(e) => {
                const nextName = e.target.value
                setForm((f) => ({
                  ...f,
                  name: nextName,
                  endpoint: endpointTouched ? f.endpoint : endpointFromName(nextName),
                }))
              }}
            />
          </div>
          <div className="col-span-2">
            <Input
              placeholder="Description"
              value={form.description}
              onChange={(e) => setForm((f) => ({ ...f, description: e.target.value }))}
            />
          </div>

          <select
            className="h-9 w-full rounded-[calc(var(--radius)-4px)] border border-[hsl(var(--input))] bg-[hsl(var(--background))] px-3 text-sm"
            value={form.priority}
            onChange={(e) => setForm((f) => ({ ...f, priority: e.target.value }))}
          >
            <option value="low">low</option>
            <option value="normal">normal</option>
            <option value="high">high</option>
            <option value="urgent">urgent</option>
          </select>

          <select
            className="h-9 w-full rounded-[calc(var(--radius)-4px)] border border-[hsl(var(--input))] bg-[hsl(var(--background))] px-3 text-sm"
            value={form.execution_kind}
            onChange={(e) =>
              setForm((f) => ({
                ...f,
                execution_kind: e.target.value as CreateForm['execution_kind'],
              }))
            }
          >
            <option value="short_running">short_running</option>
            <option value="long_running">long_running</option>
          </select>

          <div className="col-span-2">
            <select
              className="h-9 w-full rounded-[calc(var(--radius)-4px)] border border-[hsl(var(--input))] bg-[hsl(var(--background))] px-3 text-sm"
              value={form.node_uuid}
              onChange={(e) => setForm((f) => ({ ...f, node_uuid: e.target.value }))}
              data-testid="task-node"
              aria-label="Pinned node"
            >
              <option value="">Auto schedule</option>
              {nodes.map((n) => (
                <option key={n.uuid} value={n.uuid}>
                  {(n.name ? `${n.name} ` : '') + n.uuid}
                </option>
              ))}
            </select>
          </div>

          <label className="col-span-2 flex items-center justify-between rounded-[var(--radius)] border border-[hsl(var(--border))] bg-[hsl(var(--muted))] px-3 py-2">
            <div>
              <div className="text-sm font-medium">Run after create</div>
              <div className="text-xs text-[hsl(var(--muted-foreground))]">
                Auto-schedule execution via SMS placement
              </div>
            </div>
            <input
              type="checkbox"
              checked={runAfterCreate}
              onChange={(e) => setRunAfterCreate(e.target.checked)}
              data-testid="task-run-after-create"
            />
          </label>

          <div className="col-span-2">
            <Input
              placeholder="Endpoint"
              value={form.endpoint}
              onChange={(e) => {
                setEndpointTouched(true)
                setForm((f) => ({ ...f, endpoint: e.target.value }))
              }}
            />
          </div>
          <div className="col-span-2">
            <Input
              placeholder="Version"
              value={form.version}
              onChange={(e) => setForm((f) => ({ ...f, version: e.target.value }))}
            />
          </div>
          <div className="col-span-2">
            <Input
              placeholder="Capabilities (comma separated)"
              value={form.capabilities}
              onChange={(e) =>
                setForm((f) => ({ ...f, capabilities: e.target.value }))
              }
            />
          </div>

          <div className="col-span-2">
            <select
              className="h-9 w-full rounded-[calc(var(--radius)-4px)] border border-[hsl(var(--input))] bg-[hsl(var(--background))] px-3 text-sm"
              value={form.executable_type}
              onChange={(e) => {
                const nextType = e.target.value
                setForm((f) => ({
                  ...f,
                  executable_type: nextType,
                  executable_uri:
                    nextType === 'no-executable'
                      ? ''
                      : f.executable_uri || schemePrefix(scheme),
                }))
                setPickerOpen(false)
              }}
              data-testid="task-executable-type"
              aria-label="Executable Type"
            >
              <option value="no-executable">no-executable</option>
              <option value="binary">binary</option>
              <option value="script">script</option>
              <option value="container">container</option>
              <option value="wasm">wasm</option>
              <option value="process">process</option>
            </select>
          </div>

          {form.executable_type !== 'no-executable' ? (
            <>
              <div className="col-span-2 grid grid-cols-3 gap-2">
                <select
                  className="h-9 w-full rounded-[calc(var(--radius)-4px)] border border-[hsl(var(--input))] bg-[hsl(var(--background))] px-3 text-sm"
                  value={scheme}
                  onChange={(e) => {
                    const next = e.target.value as UriScheme
                    setScheme(next)
                    setForm((f) => {
                      const nextPrefix = schemePrefix(next)
                      const knownPrefixes = [
                        schemePrefix('sms+file'),
                        schemePrefix('https'),
                        schemePrefix('s3'),
                        schemePrefix('minio'),
                      ]
                      if (!f.executable_uri || knownPrefixes.includes(f.executable_uri)) {
                        return { ...f, executable_uri: nextPrefix }
                      }
                      return f
                    })
                    setPickerOpen(false)
                  }}
                  data-testid="task-uri-scheme"
                  aria-label="Scheme"
                >
                  <option value="sms+file">sms+file</option>
                  <option value="https">https</option>
                  <option value="s3">s3</option>
                  <option value="minio">minio</option>
                </select>
                <div className="col-span-2">
                  <Input
                    placeholder="Executable URI"
                    value={form.executable_uri}
                    onChange={(e) =>
                      setForm((f) => ({ ...f, executable_uri: e.target.value }))
                    }
                    data-testid="task-executable-uri"
                  />
                </div>
              </div>

              {scheme === 'sms+file' ? (
                <div className="col-span-2">
                  <div className="flex items-center justify-between rounded-[var(--radius)] border border-[hsl(var(--border))] bg-[hsl(var(--muted))] px-3 py-2">
                    <div className="text-xs text-[hsl(var(--muted-foreground))]">
                      Pick an embedded file and insert sms+file:// URI
                    </div>
                    <Button
                      variant="secondary"
                      size="sm"
                      onClick={() => setPickerOpen((v) => !v)}
                      data-testid="task-choose-local"
                    >
                      Choose Local
                    </Button>
                  </div>

                  {pickerOpen ? (
                    <div className="mt-2 overflow-hidden rounded-[var(--radius)] border border-[hsl(var(--border))] bg-[hsl(var(--background))]">
                      <div className="border-b border-[hsl(var(--border))] bg-[hsl(var(--background))] p-2">
                        <Input
                          value={fileQ}
                          onChange={(e) => setFileQ(e.target.value)}
                          placeholder="Filter by name or id"
                          data-testid="task-file-filter"
                        />
                      </div>
                      <div className="grid grid-cols-12 border-b border-[hsl(var(--border))] bg-[hsl(var(--muted))] px-3 py-2 text-xs font-medium text-[hsl(var(--muted-foreground))]">
                        <div className="col-span-7">Name / ID</div>
                        <div className="col-span-3">Modified</div>
                        <div className="col-span-2">Action</div>
                      </div>
                      {filesQuery.isLoading ? (
                        <div className="p-3 text-sm text-[hsl(var(--muted-foreground))]">
                          Loading...
                        </div>
                      ) : filesQuery.isError ? (
                        <div className="p-3 text-sm text-[hsl(var(--muted-foreground))]">
                          Failed to load files
                        </div>
                      ) : fileRows.length === 0 ? (
                        <div className="p-3 text-sm text-[hsl(var(--muted-foreground))]">
                          No files
                        </div>
                      ) : (
                        <div className="max-h-[220px] overflow-auto">
                          {fileRows.map((f) => (
                            <div
                              key={f.id}
                              className="grid grid-cols-12 items-center gap-2 border-b border-[hsl(var(--border))] px-3 py-2 text-sm last:border-b-0"
                            >
                              <div className="col-span-7 min-w-0">
                                <div className="truncate font-medium">
                                  {f.name || '(unknown)'}
                                </div>
                                <div className="mt-1 truncate text-xs text-[hsl(var(--muted-foreground))]">
                                  {f.id}
                                </div>
                              </div>
                              <div className="col-span-3 text-xs text-[hsl(var(--muted-foreground))]">
                                {formatTs(f.modified_at)}
                              </div>
                              <div className="col-span-2 flex justify-end">
                                <Button
                                  variant="ghost"
                                  size="sm"
                                  onClick={() => {
                                    setForm((cur) => ({
                                      ...cur,
                                      executable_uri: `sms+file://${f.id}`,
                                      executable_name: f.name || cur.executable_name,
                                    }))
                                    setPickerOpen(false)
                                  }}
                                  data-testid={`task-use-file-${f.id}`}
                                >
                                  Use
                                </Button>
                              </div>
                            </div>
                          ))}
                          {fileTotal !== null && fileRows.length < fileTotal ? (
                            <div className="flex items-center justify-end border-t border-[hsl(var(--border))] bg-[hsl(var(--background))] px-3 py-2">
                              <Button
                                variant="secondary"
                                size="sm"
                                onClick={() => setFileOffset((v) => v + fileLimit)}
                                disabled={filesQuery.isFetching}
                                data-testid="task-files-load-more"
                              >
                                Load more
                              </Button>
                            </div>
                          ) : null}
                        </div>
                      )}
                    </div>
                  ) : null}
                </div>
              ) : null}

              <div className="col-span-2">
                <Input
                  placeholder="Executable name (optional)"
                  value={form.executable_name}
                  onChange={(e) =>
                    setForm((f) => ({ ...f, executable_name: e.target.value }))
                  }
                />
              </div>
              <div className="col-span-2">
                <Input
                  placeholder="Checksum sha256 (optional)"
                  value={form.checksum}
                  onChange={(e) =>
                    setForm((f) => ({ ...f, checksum: e.target.value }))
                  }
                />
              </div>
              <div className="col-span-2">
                <Input
                  placeholder="Args (comma separated)"
                  value={form.args}
                  onChange={(e) => setForm((f) => ({ ...f, args: e.target.value }))}
                />
              </div>
              <div className="col-span-2">
                <textarea
                  className="h-24 w-full resize-none rounded-[calc(var(--radius)-4px)] border border-[hsl(var(--input))] bg-[hsl(var(--background))] px-3 py-2 text-sm"
                  placeholder="Env (key=value per line)"
                  value={form.env}
                  onChange={(e) => setForm((f) => ({ ...f, env: e.target.value }))}
                />
              </div>
            </>
          ) : null}
        </div>

        <div className="mt-4 flex justify-end gap-2">
          <Button variant="secondary" onClick={() => props.onOpenChange(false)}>
            Cancel
          </Button>
          <Button
            disabled={!canSubmit}
            onClick={async () => {
              try {
                const payload = {
                  name: form.name,
                  description: form.description || undefined,
                  priority: form.priority,
                  execution_kind: form.execution_kind,
                  node_uuid: form.node_uuid || undefined,
                  endpoint: form.endpoint,
                  version: form.version,
                  capabilities: parseCsv(form.capabilities),
                  executable:
                    form.executable_type === 'no-executable'
                      ? undefined
                      : {
                          type: form.executable_type,
                          uri: form.executable_uri || undefined,
                          name: form.executable_name || undefined,
                          checksum_sha256: form.checksum || undefined,
                          args: parseCsv(form.args),
                          env: parseEnv(form.env),
                        },
                }
                const res = await createTask(payload)
                if (!res.success) throw new Error(res.message || 'Create failed')
                toast.success(`Task created: ${res.task_id || ''}`)

                const shouldRun =
                  runAfterCreate &&
                  form.executable_type !== 'no-executable' &&
                  !!res.task_id
                if (shouldRun) {
                  try {
                    const exec = await createExecution({
                      task_id: res.task_id!,
                      node_uuid: form.node_uuid || undefined,
                      execution_mode: 'async',
                      max_candidates: 3,
                    })
                    if (exec.success) {
                      toast.success(`Execution scheduled on ${exec.node_uuid || ''}`)
                    } else {
                      toast.warning(exec.message || 'Execution not started')
                    }
                  } catch (e) {
                    toast.warning((e as Error).message)
                  }
                }

                props.onCreated()
                props.onOpenChange(false)
              } catch (e) {
                toast.error((e as Error).message)
              }
            }}
          >
            Create
          </Button>
        </div>
      </DialogContent>
    </Dialog>
  )
}

export default function TasksPage() {
  const [q, setQ] = useState('')
  const [creating, setCreating] = useState(false)
  const [selected, setSelected] = useState<TaskSummary | null>(null)
  const [detailOpen, setDetailOpen] = useState(false)
  const [runningTaskId, setRunningTaskId] = useState<string | null>(null)

  const tasksQuery = useQuery({
    queryKey: ['tasks', q],
    queryFn: () => listTasks({ q, sort_by: 'registered_at', order: 'desc', limit: 200 }),
    refetchInterval: 15_000,
  })

  const selectedId = selected?.task_id
  const detailQuery = useQuery({
    queryKey: ['task-detail', selectedId],
    queryFn: () => getTaskDetail(selectedId!),
    enabled: !!selectedId && detailOpen,
  })

  const rows = tasksQuery.data?.tasks || []
  const total = tasksQuery.data?.total_count ?? 0
  const title = useMemo(() => `Tasks (${rows.length}/${total})`, [rows.length, total])

  function runDisabledReason(task: TaskSummary) {
    const ek = (task.execution_kind || '').toLowerCase()
    const st = (task.status || '').toLowerCase()
    if (ek === 'long_running' && st === 'active') return 'Already running'
    return null
  }

  async function runTask(task: TaskSummary) {
    if (runningTaskId) return
    setRunningTaskId(task.task_id)
    try {
      const exec = await createExecution({
        task_id: task.task_id,
        node_uuid: task.node_uuid || undefined,
        execution_mode: 'async',
        max_candidates: 3,
      })
      if (exec.success) {
        toast.success(`Execution scheduled on ${exec.node_uuid || ''}`)
      } else {
        toast.warning(exec.message || 'Execution not started')
      }
    } catch (e) {
      toast.error((e as Error).message)
    } finally {
      setRunningTaskId(null)
    }
  }

  return (
    <div className="space-y-4">
      <div className="flex items-center justify-between">
        <div>
          <div className="text-lg font-semibold">Tasks</div>
          <div className="text-sm text-[hsl(var(--muted-foreground))]">
            Registered tasks
          </div>
        </div>
        <div className="flex items-center gap-2">
          <Button variant="secondary" onClick={() => tasksQuery.refetch()}>
            Refresh
          </Button>
          <Button onClick={() => setCreating(true)} data-testid="tasks-open-create">
            <Plus className="h-4 w-4" />
            Create
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
              <Search className="absolute left-3 top-1/2 h-4 w-4 -translate-y-1/2 text-[hsl(var(--muted-foreground))]" />
              <Input
                value={q}
                onChange={(e) => setQ(e.target.value)}
                placeholder="Search task/name/pinned-node/endpoint"
                className="pl-9"
              />
            </div>
          </div>

          <div className="overflow-hidden rounded-[var(--radius)] border border-[hsl(var(--border))] bg-[hsl(var(--background))]">
            <div className="grid grid-cols-12 border-b border-[hsl(var(--border))] bg-[hsl(var(--muted))] px-3 py-2 text-xs font-medium text-[hsl(var(--muted-foreground))]">
              <div className="col-span-3">Name / Task ID</div>
              <div className="col-span-2">Status</div>
              <div className="col-span-2">Priority</div>
              <div className="col-span-2">Pinned node</div>
              <div className="col-span-3">Registered / Run</div>
            </div>

            {rows.length === 0 ? (
              <div className="p-6 text-sm text-[hsl(var(--muted-foreground))]">
                {tasksQuery.isLoading
                  ? 'Loading...'
                  : tasksQuery.isError
                    ? 'Failed to load tasks'
                    : 'No tasks'}
              </div>
            ) : (
              <div className="max-h-[560px] overflow-auto">
                {rows.map((t) => (
                  <div
                    key={t.task_id}
                    onClick={() => {
                      setSelected(t)
                      setDetailOpen(true)
                    }}
                    className={cn(
                      'grid w-full grid-cols-12 items-center gap-2 px-3 py-2 text-left text-sm hover:bg-[hsl(var(--accent))]',
                      'border-b border-[hsl(var(--border))] last:border-b-0',
                    )}
                    role="button"
                    tabIndex={0}
                    onKeyDown={(e) => {
                      if (e.key === 'Enter' || e.key === ' ') {
                        e.preventDefault()
                        setSelected(t)
                        setDetailOpen(true)
                      }
                    }}
                    data-testid={`task-row-${t.task_id}`}
                  >
                    <div className="col-span-3 min-w-0">
                      <div className="truncate font-medium">{t.name}</div>
                      <div className="mt-1 truncate text-xs text-[hsl(var(--muted-foreground))]">
                        {t.task_id}
                      </div>
                    </div>
                    <div className="col-span-2">
                      <StatusBadge status={t.status} />
                    </div>
                    <div className="col-span-2 text-sm text-[hsl(var(--muted-foreground))]">
                      {t.priority}
                    </div>
                    <div className="col-span-2 truncate text-sm text-[hsl(var(--muted-foreground))]">
                      {t.node_uuid || '-'}
                    </div>
                  <div className="col-span-3 flex items-center justify-between gap-2 text-sm text-[hsl(var(--muted-foreground))]">
                    <div className="min-w-0 truncate">{formatTs(t.registered_at)}</div>
                    <Button
                      variant="secondary"
                      size="sm"
                      disabled={
                        runningTaskId === t.task_id || !!runDisabledReason(t)
                      }
                      onClick={(e) => {
                        e.preventDefault()
                        e.stopPropagation()
                        runTask(t)
                      }}
                      data-testid={`task-run-${t.task_id}`}
                      title={runDisabledReason(t) || undefined}
                    >
                      Run
                    </Button>
                  </div>
                  </div>
                ))}
              </div>
            )}
          </div>
        </CardContent>
      </Card>

      <CreateTaskDialog
        open={creating}
        onOpenChange={setCreating}
        onCreated={() => tasksQuery.refetch()}
      />

      <Dialog open={detailOpen} onOpenChange={setDetailOpen}>
        <DialogContent>
          <DialogHeader
            title={selected ? selected.name : 'Task detail'}
            description={selected ? selected.task_id : undefined}
          />
          {detailQuery.isLoading ? (
            <div className="text-sm text-[hsl(var(--muted-foreground))]">Loading…</div>
          ) : detailQuery.isError ? (
            <div className="text-sm text-[hsl(var(--muted-foreground))]">
              Failed to load task detail.
            </div>
          ) : detailQuery.data?.found ? (
            <pre className="max-h-[520px] overflow-auto rounded-[var(--radius)] border border-[hsl(var(--border))] bg-[hsl(var(--secondary))] p-3 text-xs">
              {JSON.stringify(detailQuery.data, null, 2)}
            </pre>
          ) : (
            <div className="text-sm text-[hsl(var(--muted-foreground))]">Not found</div>
          )}
        </DialogContent>
      </Dialog>
    </div>
  )
}
