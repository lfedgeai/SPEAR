import { useMemo, useRef, useState } from 'react'
import { useQuery, useQueryClient } from '@tanstack/react-query'
import { Copy, Download, Trash2, Upload } from 'lucide-react'
import { toast } from 'sonner'

import { deleteFile, getFileMeta, listFiles, uploadFile } from '@/api/files'
import type { FileItem } from '@/api/types'
import { Button } from '@/components/ui/button'
import { Card, CardContent, CardHeader, CardTitle } from '@/components/ui/card'
import { Dialog, DialogContent, DialogHeader } from '@/components/ui/dialog'
import { cn } from '@/lib/utils'

function formatTs(ts: number) {
  if (!ts) return '-'
  const d = new Date(ts * 1000)
  return d.toLocaleString()
}

function bytesHuman(n: number) {
  if (!Number.isFinite(n) || n <= 0) return '0 B'
  const units = ['B', 'KB', 'MB', 'GB', 'TB']
  let v = n
  let i = 0
  while (v >= 1024 && i < units.length - 1) {
    v /= 1024
    i++
  }
  return `${v.toFixed(i === 0 ? 0 : 1)} ${units[i]}`
}

async function copyText(v: string) {
  await navigator.clipboard.writeText(v)
  toast.success('Copied')
}

export default function FilesPage() {
  const qc = useQueryClient()
  const inputRef = useRef<HTMLInputElement | null>(null)
  const [selectedFiles, setSelectedFiles] = useState<File[]>([])
  const [uploading, setUploading] = useState(false)
  const [detailId, setDetailId] = useState<string | null>(null)
  const [detailOpen, setDetailOpen] = useState(false)

  const filesQuery = useQuery({
    queryKey: ['files'],
    queryFn: () => listFiles(),
    staleTime: 5_000,
  })

  const detailQuery = useQuery({
    queryKey: ['file-meta', detailId],
    queryFn: () => getFileMeta(detailId!),
    enabled: !!detailId && detailOpen,
  })

  const rows = useMemo(() => filesQuery.data?.files || [], [filesQuery.data])

  return (
    <div className="space-y-4">
      <div className="flex items-center justify-between">
        <div>
          <div className="text-lg font-semibold">Files</div>
          <div className="text-sm text-[hsl(var(--muted-foreground))]">
            Embedded file storage
          </div>
        </div>
        <Button variant="secondary" onClick={() => filesQuery.refetch()}>
          Refresh
        </Button>
      </div>

      <Card>
        <CardHeader>
          <CardTitle>Upload</CardTitle>
        </CardHeader>
        <CardContent>
          <input
            ref={inputRef}
            type="file"
            multiple
            data-testid="files-input"
            className="hidden"
            onChange={(e) => {
              const fs = e.target.files ? Array.from(e.target.files) : []
              setSelectedFiles(fs)
            }}
          />

          <div className="flex flex-wrap items-center gap-2">
            <Button
              variant="secondary"
              onClick={() => inputRef.current?.click()}
              disabled={uploading}
            >
              Choose files
            </Button>
            <div className="text-sm text-[hsl(var(--muted-foreground))]">
              {selectedFiles.length === 0
                ? 'No file selected'
                : selectedFiles.length === 1
                  ? selectedFiles[0].name
                  : `${selectedFiles.length} files selected`}
            </div>
            <div className="ml-auto flex items-center gap-2">
              <Button
                variant="secondary"
                onClick={() => setSelectedFiles([])}
                disabled={uploading || selectedFiles.length === 0}
              >
                Clear
              </Button>
              <Button
                onClick={async () => {
                  if (selectedFiles.length === 0) return
                  try {
                    setUploading(true)
                    let ok = 0
                    let fail = 0
                    for (const f of selectedFiles) {
                      try {
                        await uploadFile(f)
                        ok++
                      } catch (e) {
                        fail++
                        toast.error(`${f.name}: ${(e as Error).message}`)
                      }
                    }
                    qc.invalidateQueries({ queryKey: ['files'] })
                    setSelectedFiles([])
                    if (ok > 0 && fail === 0) toast.success('All uploads completed')
                    if (ok > 0 && fail > 0) toast.warning('Uploads completed with failures')
                    if (ok === 0 && fail > 0) toast.error('All uploads failed')
                  } finally {
                    setUploading(false)
                  }
                }}
                disabled={uploading || selectedFiles.length === 0}
                data-testid="files-upload"
              >
                <Upload className="h-4 w-4" />
                {uploading ? 'Uploading…' : 'Upload'}
              </Button>
            </div>
          </div>
        </CardContent>
      </Card>

      <Card>
        <CardHeader>
          <CardTitle>Files ({rows.length})</CardTitle>
        </CardHeader>
        <CardContent>
          <div className="overflow-hidden rounded-[var(--radius)] border border-[hsl(var(--border))] bg-[hsl(var(--background))]">
            <div className="grid grid-cols-12 border-b border-[hsl(var(--border))] bg-[hsl(var(--muted))] px-3 py-2 text-xs font-medium text-[hsl(var(--muted-foreground))]">
              <div className="col-span-4">Name / ID</div>
              <div className="col-span-2">Size</div>
              <div className="col-span-3">Modified</div>
              <div className="col-span-3">Actions</div>
            </div>

            {rows.length === 0 ? (
              <div className="p-6 text-sm text-[hsl(var(--muted-foreground))]">
                {filesQuery.isLoading
                  ? 'Loading...'
                  : filesQuery.isError
                    ? 'Failed to load files'
                    : 'No files'}
              </div>
            ) : (
              <div className="max-h-[560px] overflow-auto">
                {rows.map((f: FileItem) => (
                  <div
                    key={f.id}
                    data-testid={`files-row-${f.id}`}
                    className={cn(
                      'grid grid-cols-12 items-center gap-2 px-3 py-2 text-sm',
                      'border-b border-[hsl(var(--border))] last:border-b-0',
                    )}
                  >
                    <button
                      type="button"
                      className="col-span-4 min-w-0 text-left"
                      onClick={() => {
                        setDetailId(f.id)
                        setDetailOpen(true)
                      }}
                    >
                      <div className="truncate font-medium">{f.name || '(unknown)'}</div>
                      <div className="mt-1 truncate text-xs text-[hsl(var(--muted-foreground))]">
                        {f.id}
                      </div>
                    </button>
                    <div className="col-span-2 text-[hsl(var(--muted-foreground))]">
                      {bytesHuman(f.len)}
                    </div>
                    <div className="col-span-3 text-[hsl(var(--muted-foreground))]">
                      {formatTs(f.modified_at)}
                    </div>
                    <div className="col-span-3 flex items-center justify-end gap-2">
                      <a
                        className="inline-flex"
                        href={`/admin/api/files/${encodeURIComponent(f.id)}`}
                        target="_blank"
                        rel="noreferrer"
                      >
                        <Button variant="ghost" size="sm">
                          <Download className="h-4 w-4" />
                          Download
                        </Button>
                      </a>
                      <Button
                        variant="ghost"
                        size="sm"
                        onClick={() => copyText(`sms+file://${f.id}`)}
                        data-testid={`files-copy-uri-${f.id}`}
                      >
                        <Copy className="h-4 w-4" />
                        Copy URI
                      </Button>
                      <Button
                        variant="destructive"
                        size="sm"
                        onClick={async () => {
                          try {
                            await deleteFile(f.id)
                            toast.success('Deleted')
                            qc.invalidateQueries({ queryKey: ['files'] })
                          } catch (e) {
                            toast.error((e as Error).message)
                          }
                        }}
                        data-testid={`files-delete-${f.id}`}
                      >
                        <Trash2 className="h-4 w-4" />
                        Delete
                      </Button>
                    </div>
                  </div>
                ))}
              </div>
            )}
          </div>
        </CardContent>
      </Card>

      <Dialog open={detailOpen} onOpenChange={setDetailOpen}>
        <DialogContent>
          <DialogHeader title="File meta" description={detailId || undefined} />
          {detailQuery.isLoading ? (
            <div className="text-sm text-[hsl(var(--muted-foreground))]">Loading…</div>
          ) : detailQuery.isError ? (
            <div className="text-sm text-[hsl(var(--muted-foreground))]">
              Failed to load meta.
            </div>
          ) : (
            <pre className="max-h-[520px] overflow-auto rounded-[var(--radius)] border border-[hsl(var(--border))] bg-[hsl(var(--secondary))] p-3 text-xs">
              {JSON.stringify(detailQuery.data, null, 2)}
            </pre>
          )}
        </DialogContent>
      </Dialog>
    </div>
  )
}
