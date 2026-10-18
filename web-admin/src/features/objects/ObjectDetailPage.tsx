import { Link, useNavigate, useParams } from 'react-router-dom'
import { useQuery, useQueryClient } from '@tanstack/react-query'
import { Download, Trash2 } from 'lucide-react'
import { toast } from 'sonner'

import {
  deleteObject,
  getObjectDownloadUrl,
  headObject,
  pinObject,
  releaseObject,
  retainObject,
  unpinObject,
} from '@/api/objects'
import { Badge } from '@/components/ui/badge'
import { Button } from '@/components/ui/button'
import { Card, CardContent, CardHeader, CardTitle } from '@/components/ui/card'
import { formatObjectSize, formatObjectTs } from '@/features/objects/object-format'

/**
 * Object detail page backed by metadata-first loading.
 * 以元数据优先方式加载的对象详情页。
 */
export default function ObjectDetailPage() {
  const navigate = useNavigate()
  const qc = useQueryClient()
  const { key } = useParams()
  const objectKey = key || ''

  const objectQuery = useQuery({
    queryKey: ['object', objectKey],
    queryFn: () => headObject(objectKey),
    enabled: !!objectKey,
  })

  const detail = objectQuery.data

  async function runObjectAction(action: () => Promise<unknown>, successMessage: string) {
    try {
      await action()
      toast.success(successMessage)
      await Promise.all([
        qc.invalidateQueries({ queryKey: ['object', objectKey] }),
        qc.invalidateQueries({ queryKey: ['objects'] }),
      ])
    } catch (error) {
      toast.error((error as Error).message)
    }
  }

  return (
    <div className="space-y-4">
      <div className="flex items-center justify-between">
        <div>
          <div className="text-lg font-semibold">Object</div>
          <div className="text-sm text-[hsl(var(--muted-foreground))]">
            <Link to="/objects" className="hover:underline">
              Objects
            </Link>
            <span className="mx-2">/</span>
            <span className="font-mono text-xs">{objectKey}</span>
          </div>
        </div>
        <div className="flex items-center gap-2">
          <a
            className="inline-flex"
            href={getObjectDownloadUrl(objectKey)}
            target="_blank"
            rel="noreferrer"
          >
            <Button variant="secondary" size="sm">
              <Download className="h-4 w-4" />
              Download
            </Button>
          </a>
          {detail ? (
            <>
              <Button
                variant="secondary"
                size="sm"
                onClick={() =>
                  runObjectAction(
                    () => (detail.pinned ? unpinObject(objectKey) : pinObject(objectKey)),
                    detail.pinned ? 'Object unpinned' : 'Object pinned',
                  )
                }
              >
                {detail.pinned ? 'Unpin' : 'Pin'}
              </Button>
              <Button
                variant="secondary"
                size="sm"
                onClick={() => runObjectAction(() => retainObject(objectKey), 'Reference retained')}
              >
                Retain
              </Button>
              <Button
                variant="secondary"
                size="sm"
                onClick={() => runObjectAction(() => releaseObject(objectKey), 'Reference released')}
                disabled={(detail.refCount ?? 0) <= 0}
              >
                Release
              </Button>
            </>
          ) : null}
          <Button
            variant="destructive"
            size="sm"
            onClick={async () => {
              try {
                await deleteObject(objectKey)
                toast.success('Deleted')
                await qc.invalidateQueries({ queryKey: ['objects'] })
                navigate('/objects')
              } catch (error) {
                toast.error((error as Error).message)
              }
            }}
            disabled={(detail?.refCount ?? 0) > 0 || !!detail?.pinned}
          >
            <Trash2 className="h-4 w-4" />
            Delete
          </Button>
        </div>
      </div>

      <Card>
        <CardHeader>
          <div className="flex items-center justify-between">
            <CardTitle>Summary</CardTitle>
            <Button variant="secondary" size="sm" onClick={() => objectQuery.refetch()}>
              Refresh
            </Button>
          </div>
        </CardHeader>
        <CardContent>
          {objectQuery.isLoading ? (
            <div className="text-sm text-[hsl(var(--muted-foreground))]">Loading…</div>
          ) : objectQuery.isError || !detail ? (
            <div className="text-sm text-[hsl(var(--muted-foreground))]">
              Failed to load object metadata.
            </div>
          ) : (
            <div className="grid grid-cols-2 gap-3 text-sm">
              <div>
                <div className="text-xs text-[hsl(var(--muted-foreground))]">Key</div>
                <div className="mt-1 break-all font-mono">{detail.key}</div>
              </div>
              <div>
                <div className="text-xs text-[hsl(var(--muted-foreground))]">Name</div>
                <div className="mt-1">{detail.name || '-'}</div>
              </div>
              <div>
                <div className="text-xs text-[hsl(var(--muted-foreground))]">Size</div>
                <div className="mt-1">{formatObjectSize(detail.size)}</div>
              </div>
              <div>
                <div className="text-xs text-[hsl(var(--muted-foreground))]">State</div>
                <div className="mt-1">
                  <Badge variant={detail.state === 'deleting' ? 'secondary' : 'success'}>
                    {detail.state}
                  </Badge>
                </div>
              </div>
              <div>
                <div className="text-xs text-[hsl(var(--muted-foreground))]">Checksum</div>
                <div className="mt-1 font-mono text-xs">{detail.checksum || '-'}</div>
              </div>
              <div>
                <div className="text-xs text-[hsl(var(--muted-foreground))]">Reference Count</div>
                <div className="mt-1">{detail.refCount ?? '-'}</div>
              </div>
              <div>
                <div className="text-xs text-[hsl(var(--muted-foreground))]">Pinned</div>
                <div className="mt-1">{detail.pinned ? 'Yes' : 'No'}</div>
              </div>
              <div>
                <div className="text-xs text-[hsl(var(--muted-foreground))]">Created</div>
                <div className="mt-1">{formatObjectTs(detail.createdAt)}</div>
              </div>
              <div className="col-span-2">
                <div className="text-xs text-[hsl(var(--muted-foreground))]">Updated</div>
                <div className="mt-1">{formatObjectTs(detail.updatedAt)}</div>
              </div>
            </div>
          )}
        </CardContent>
      </Card>

      <Card>
        <CardHeader>
          <CardTitle>Metadata</CardTitle>
        </CardHeader>
        <CardContent>
          {objectQuery.isLoading ? (
            <div className="text-sm text-[hsl(var(--muted-foreground))]">Loading…</div>
          ) : objectQuery.isError || !detail ? (
            <div className="text-sm text-[hsl(var(--muted-foreground))]">
              Failed to load metadata.
            </div>
          ) : detail.metadata && Object.keys(detail.metadata).length > 0 ? (
            <pre className="max-h-[360px] overflow-auto rounded-[var(--radius)] border border-[hsl(var(--border))] bg-[hsl(var(--secondary))] p-3 text-xs">
              {JSON.stringify(detail.metadata, null, 2)}
            </pre>
          ) : (
            <div className="text-sm text-[hsl(var(--muted-foreground))]">No metadata</div>
          )}
        </CardContent>
      </Card>
    </div>
  )
}
