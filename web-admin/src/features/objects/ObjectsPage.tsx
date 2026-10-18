import { useMemo, useRef, useState } from 'react'
import { useQuery, useQueryClient } from '@tanstack/react-query'
import { Download, Search, Trash2, Upload } from 'lucide-react'
import { useNavigate } from 'react-router-dom'
import { toast } from 'sonner'

import {
  deleteObject,
  getObjectDownloadUrl,
  listObjects,
  pinObject,
  putObject,
  releaseObject,
  retainObject,
  unpinObject,
} from '@/api/objects'
import { Badge } from '@/components/ui/badge'
import { Button } from '@/components/ui/button'
import { Card, CardContent, CardHeader, CardTitle } from '@/components/ui/card'
import { Input } from '@/components/ui/input'
import { formatObjectSize, formatObjectTs } from '@/features/objects/object-format'

/**
 * Object catalog list page.
 * 对象目录列表页。
 */
export default function ObjectsPage() {
  const navigate = useNavigate()
  const qc = useQueryClient()
  const inputRef = useRef<HTMLInputElement | null>(null)
  const [selectedFiles, setSelectedFiles] = useState<File[]>([])
  const [uploading, setUploading] = useState(false)
  const [query, setQuery] = useState('')

  const objectsQuery = useQuery({
    queryKey: ['objects', query],
    queryFn: () => listObjects({ prefix: query || undefined, limit: 200 }),
    staleTime: 5_000,
  })

  const rows = useMemo(() => objectsQuery.data?.items || [], [objectsQuery.data])

  async function runObjectAction(action: () => Promise<unknown>, successMessage: string) {
    try {
      await action()
      toast.success(successMessage)
      await qc.invalidateQueries({ queryKey: ['objects'] })
    } catch (error) {
      toast.error((error as Error).message)
    }
  }

  return (
    <div className="space-y-4">
      <div className="flex items-center justify-between">
        <div>
          <div className="text-lg font-semibold">Objects</div>
          <div className="text-sm text-[hsl(var(--muted-foreground))]">
            Browse object metadata and trigger object lifecycle operations.
          </div>
        </div>
        <Button variant="secondary" onClick={() => objectsQuery.refetch()}>
          Refresh
        </Button>
      </div>

      <Card>
        <CardHeader>
          <CardTitle>Upload</CardTitle>
        </CardHeader>
        <CardContent className="space-y-3">
          <div className="text-sm text-[hsl(var(--muted-foreground))]">
            Current backend compatibility mode derives the object key from the uploaded file.
          </div>
          <input
            ref={inputRef}
            type="file"
            multiple
            data-testid="objects-input"
            className="hidden"
            onChange={(event) => {
              const files = event.target.files ? Array.from(event.target.files) : []
              setSelectedFiles(files)
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
                ? 'No object selected'
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
                    for (const file of selectedFiles) {
                      try {
                        await putObject(file)
                        ok += 1
                      } catch (error) {
                        fail += 1
                        toast.error(`${file.name}: ${(error as Error).message}`)
                      }
                    }
                    qc.invalidateQueries({ queryKey: ['objects'] })
                    setSelectedFiles([])
                    if (ok > 0 && fail === 0) toast.success('All uploads completed')
                    if (ok > 0 && fail > 0) toast.warning('Uploads completed with failures')
                    if (ok === 0 && fail > 0) toast.error('All uploads failed')
                  } finally {
                    setUploading(false)
                  }
                }}
                disabled={uploading || selectedFiles.length === 0}
                data-testid="objects-upload"
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
          <CardTitle>Catalog</CardTitle>
        </CardHeader>
        <CardContent className="space-y-3">
          <div className="flex items-center gap-2">
            <div className="relative w-full max-w-md">
              <Search className="absolute left-3 top-1/2 h-4 w-4 -translate-y-1/2 text-[hsl(var(--muted-foreground))]" />
              <Input
                value={query}
                onChange={(e) => setQuery(e.target.value)}
                placeholder="Search object key"
                className="pl-9"
                data-testid="objects-search"
              />
            </div>
          </div>

          <div className="overflow-hidden rounded-[var(--radius)] border border-[hsl(var(--border))] bg-[hsl(var(--background))]">
            <div className="grid grid-cols-12 border-b border-[hsl(var(--border))] bg-[hsl(var(--muted))] px-3 py-2 text-xs font-medium text-[hsl(var(--muted-foreground))]">
              <div className="col-span-4">Object</div>
              <div className="col-span-2">Size</div>
              <div className="col-span-2">State</div>
              <div className="col-span-1">Refs</div>
              <div className="col-span-3">Updated / Actions</div>
            </div>

            {rows.length === 0 ? (
              <div className="p-6 text-sm text-[hsl(var(--muted-foreground))]">
                {objectsQuery.isLoading
                  ? 'Loading...'
                  : objectsQuery.isError
                    ? 'Failed to load objects'
                    : 'No objects'}
              </div>
            ) : (
              <div className="max-h-[560px] overflow-auto">
                {rows.map((object) => (
                  <div
                    key={object.key}
                    data-testid={`objects-row-${object.key}`}
                    className="grid grid-cols-12 items-center gap-2 border-b border-[hsl(var(--border))] px-3 py-2 text-sm last:border-b-0"
                  >
                    <button
                      type="button"
                      className="col-span-4 min-w-0 text-left"
                      onClick={() => navigate(`/objects/${encodeURIComponent(object.key)}`)}
                    >
                      <div className="truncate font-medium">{object.name || object.key}</div>
                      <div className="mt-1 truncate font-mono text-xs text-[hsl(var(--muted-foreground))]">
                        {object.key}
                      </div>
                    </button>
                    <div className="col-span-2 text-[hsl(var(--muted-foreground))]">
                      {formatObjectSize(object.size)}
                    </div>
                    <div className="col-span-2 text-[hsl(var(--muted-foreground))]">
                      <div className="flex items-center gap-2">
                        <Badge variant={object.state === 'deleting' ? 'secondary' : 'success'}>
                          {object.state}
                        </Badge>
                        {object.pinned ? <Badge variant="secondary">Pinned</Badge> : null}
                      </div>
                    </div>
                    <div className="col-span-1 text-[hsl(var(--muted-foreground))]">
                      {object.refCount ?? 0}
                    </div>
                    <div className="col-span-3 flex items-center justify-end gap-2">
                      <div className="mr-auto text-xs text-[hsl(var(--muted-foreground))]">
                        {formatObjectTs(object.updatedAt)}
                      </div>
                      <a
                        className="inline-flex"
                        href={getObjectDownloadUrl(object.key)}
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
                        onClick={() =>
                          runObjectAction(
                            () =>
                              object.pinned
                                ? unpinObject(object.key)
                                : pinObject(object.key),
                            object.pinned ? 'Object unpinned' : 'Object pinned',
                          )
                        }
                        data-testid={`objects-pin-${object.key}`}
                      >
                        {object.pinned ? 'Unpin' : 'Pin'}
                      </Button>
                      <Button
                        variant="ghost"
                        size="sm"
                        onClick={() =>
                          runObjectAction(() => retainObject(object.key), 'Reference retained')
                        }
                        data-testid={`objects-retain-${object.key}`}
                      >
                        Retain
                      </Button>
                      <Button
                        variant="ghost"
                        size="sm"
                        onClick={() =>
                          runObjectAction(() => releaseObject(object.key), 'Reference released')
                        }
                        disabled={(object.refCount ?? 0) <= 0}
                        data-testid={`objects-release-${object.key}`}
                      >
                        Release
                      </Button>
                      <Button
                        variant="destructive"
                        size="sm"
                        onClick={() => runObjectAction(() => deleteObject(object.key), 'Deleted')}
                        disabled={(object.refCount ?? 0) > 0 || !!object.pinned}
                        data-testid={`objects-delete-${object.key}`}
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
    </div>
  )
}
