import { Link, useParams } from 'react-router-dom'
import { useQuery } from '@tanstack/react-query'
import { Copy, Download, Trash2 } from 'lucide-react'
import { toast } from 'sonner'

import { deleteFile, getFileMeta } from '@/api/files'
import { Button } from '@/components/ui/button'
import { Card, CardContent, CardHeader, CardTitle } from '@/components/ui/card'

async function copyText(v: string) {
  await navigator.clipboard.writeText(v)
  toast.success('Copied')
}

export default function FileDetailPage() {
  const { id } = useParams()
  const fileId = id || ''

  const metaQuery = useQuery({
    queryKey: ['file-meta', fileId],
    queryFn: () => getFileMeta(fileId),
    enabled: !!fileId,
  })

  return (
    <div className="space-y-4">
      <div className="flex items-center justify-between">
        <div>
          <div className="text-lg font-semibold">File</div>
          <div className="text-sm text-[hsl(var(--muted-foreground))]">
            <Link to="/files" className="hover:underline">
              Files
            </Link>
            <span className="mx-2">/</span>
            <span className="font-mono text-xs">{fileId}</span>
          </div>
        </div>
        <div className="flex items-center gap-2">
          <a
            className="inline-flex"
            href={`/admin/api/files/${encodeURIComponent(fileId)}`}
            target="_blank"
            rel="noreferrer"
          >
            <Button variant="secondary" size="sm">
              <Download className="h-4 w-4" />
              Download
            </Button>
          </a>
          <Button
            variant="secondary"
            size="sm"
            onClick={() => copyText(`smsfile://${fileId}`)}
          >
            <Copy className="h-4 w-4" />
            Copy URI
          </Button>
          <Button
            variant="destructive"
            size="sm"
            onClick={async () => {
              try {
                await deleteFile(fileId)
                toast.success('Deleted')
              } catch (e) {
                toast.error((e as Error).message)
              }
            }}
          >
            <Trash2 className="h-4 w-4" />
            Delete
          </Button>
        </div>
      </div>

      <Card>
        <CardHeader>
          <div className="flex items-center justify-between">
            <CardTitle>Meta</CardTitle>
            <Button variant="secondary" size="sm" onClick={() => metaQuery.refetch()}>
              Refresh
            </Button>
          </div>
        </CardHeader>
        <CardContent>
          {metaQuery.isLoading ? (
            <div className="text-sm text-[hsl(var(--muted-foreground))]">Loading…</div>
          ) : metaQuery.isError ? (
            <div className="text-sm text-[hsl(var(--muted-foreground))]">
              Failed to load meta.
            </div>
          ) : (
            <pre className="max-h-[620px] overflow-auto rounded-[var(--radius)] border border-[hsl(var(--border))] bg-[hsl(var(--secondary))] p-3 text-xs">
              {JSON.stringify(metaQuery.data, null, 2)}
            </pre>
          )}
        </CardContent>
      </Card>
    </div>
  )
}
