import { useMemo, useState } from 'react'
import { useQuery } from '@tanstack/react-query'
import { Download, RefreshCw } from 'lucide-react'

import { getExecutionLogs } from '@/api/logs'
import { Button } from '@/components/ui/button'
import { Dialog, DialogContent, DialogHeader } from '@/components/ui/dialog'

const WRAP_STORAGE_KEY = 'ADMIN_EXECUTION_LOG_WRAP'

export default function ExecutionLogsDialog(props: {
  open: boolean
  onOpenChange: (v: boolean) => void
  executionId: string
}) {
  const id = props.executionId
  const [wrapLines, setWrapLines] = useState(() => {
    return localStorage.getItem(WRAP_STORAGE_KEY) === '1'
  })
  const logsQuery = useQuery({
    queryKey: ['execution-logs', id],
    queryFn: () => getExecutionLogs({ execution_id: id, cursor: '0', limit: 2000 }),
    enabled: props.open && !!id,
    staleTime: 1_000,
  })

  const rendered = useMemo(() => {
    const lines = logsQuery.data?.lines || []
    return lines
      .map((l) => `${l.ts_ms}\t${l.stream}\t${l.level}\t${l.message}`)
      .join('\n')
  }, [logsQuery.data])

  return (
    <Dialog open={props.open} onOpenChange={props.onOpenChange}>
      <DialogContent className="max-h-[80vh] overflow-auto">
        <div className="flex items-start justify-between gap-3">
          <DialogHeader title="Execution logs" description={id} />
          <div className="mt-1 flex items-center gap-2">
            <Button
              variant="secondary"
              size="sm"
              aria-pressed={wrapLines}
              onClick={() => {
                setWrapLines((prev) => {
                  const next = !prev
                  localStorage.setItem(WRAP_STORAGE_KEY, next ? '1' : '0')
                  return next
                })
              }}
            >
              {wrapLines ? 'No wrap' : 'Wrap'}
            </Button>
            <Button
              variant="secondary"
              size="sm"
              onClick={() => logsQuery.refetch()}
              disabled={logsQuery.isFetching}
            >
              <RefreshCw className="h-4 w-4" />
              Refresh
            </Button>
            {id ? (
              <a
                className="inline-flex"
                href={`/admin/api/executions/${encodeURIComponent(id)}/logs/download?format=text`}
                target="_blank"
                rel="noreferrer"
              >
                <Button variant="secondary" size="sm">
                  <Download className="h-4 w-4" />
                  Download
                </Button>
              </a>
            ) : null}
          </div>
        </div>

        {!logsQuery.data ? (
          <div className="text-sm text-[hsl(var(--muted-foreground))]">
            {logsQuery.isLoading ? 'Loading…' : 'No data'}
          </div>
        ) : logsQuery.data.lines.length === 0 ? (
          <div className="text-sm text-[hsl(var(--muted-foreground))]">
            Logs not available yet.
          </div>
        ) : (
          <div className="space-y-3">
            {logsQuery.data.truncated ? (
              <div className="text-sm text-[hsl(var(--muted-foreground))]">
                Log is truncated.
              </div>
            ) : null}
            <pre
              data-testid="execution-logs-pre"
              className={[
                'max-h-[60vh] overflow-auto rounded-[var(--radius)] border border-[hsl(var(--border))] bg-[hsl(var(--secondary))] p-3 text-xs',
                wrapLines ? 'whitespace-pre-wrap break-words' : 'whitespace-pre',
              ].join(' ')}
            >
              {rendered}
            </pre>
          </div>
        )}
      </DialogContent>
    </Dialog>
  )
}
