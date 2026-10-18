import { useMemo, useState } from 'react'
import { useInfiniteQuery } from '@tanstack/react-query'

import { listObjects } from '@/api/objects'
import { Button } from '@/components/ui/button'
import { Dialog, DialogContent, DialogHeader } from '@/components/ui/dialog'
import { Input } from '@/components/ui/input'
import { formatObjectSize, formatObjectTs } from '@/features/objects/object-format'
import type { ObjectSummary } from '@/features/objects/object-types'

/**
 * Reusable object picker for task creation and future forms.
 * 用于任务创建及后续表单复用的对象选择器。
 */
export default function ObjectPickerModal(props: {
  open: boolean
  onOpenChange: (open: boolean) => void
  onSelect: (object: ObjectSummary) => void
}) {
  const [query, setQuery] = useState('')

  const objectsQuery = useInfiniteQuery({
    queryKey: ['object-picker', query, props.open],
    queryFn: ({ pageParam }) =>
      listObjects({ prefix: query || undefined, limit: 60, pageToken: pageParam }),
    enabled: props.open,
    staleTime: 5_000,
    initialPageParam: undefined as string | undefined,
    getNextPageParam: (lastPage) => lastPage.nextPageToken,
  })

  const rows = useMemo(
    () => objectsQuery.data?.pages.flatMap((page) => page.items) ?? [],
    [objectsQuery.data],
  )

  return (
    <Dialog open={props.open} onOpenChange={props.onOpenChange}>
      <DialogContent>
        <DialogHeader
          title="Choose object"
          description="Pick an object from the catalog for the current form."
        />

        <div className="space-y-3">
          <Input
            value={query}
            onChange={(e) => setQuery(e.target.value)}
            placeholder="Filter by object key"
            data-testid="object-picker-filter"
          />

          <div className="overflow-hidden rounded-[var(--radius)] border border-[hsl(var(--border))] bg-[hsl(var(--background))]">
            <div className="grid grid-cols-12 border-b border-[hsl(var(--border))] bg-[hsl(var(--muted))] px-3 py-2 text-xs font-medium text-[hsl(var(--muted-foreground))]">
              <div className="col-span-6">Object</div>
              <div className="col-span-2">Size</div>
              <div className="col-span-2">Updated</div>
              <div className="col-span-2">Action</div>
            </div>

            {objectsQuery.isLoading ? (
              <div className="p-3 text-sm text-[hsl(var(--muted-foreground))]">Loading…</div>
            ) : objectsQuery.isError ? (
              <div className="p-3 text-sm text-[hsl(var(--muted-foreground))]">
                Failed to load objects
              </div>
            ) : rows.length === 0 ? (
              <div className="p-3 text-sm text-[hsl(var(--muted-foreground))]">No objects</div>
            ) : (
              <div className="max-h-[280px] overflow-auto">
                {rows.map((object) => (
                  <div
                    key={object.key}
                    className="grid grid-cols-12 items-center gap-2 border-b border-[hsl(var(--border))] px-3 py-2 text-sm last:border-b-0"
                  >
                    <div className="col-span-6 min-w-0">
                      <div className="truncate font-medium">{object.name || object.key}</div>
                      <div className="mt-1 truncate font-mono text-xs text-[hsl(var(--muted-foreground))]">
                        {object.key}
                      </div>
                    </div>
                    <div className="col-span-2 text-xs text-[hsl(var(--muted-foreground))]">
                      {formatObjectSize(object.size)}
                    </div>
                    <div className="col-span-2 text-xs text-[hsl(var(--muted-foreground))]">
                      {formatObjectTs(object.updatedAt)}
                    </div>
                    <div className="col-span-2 flex justify-end">
                      <Button
                        variant="ghost"
                        size="sm"
                        onClick={() => {
                          props.onSelect(object)
                          props.onOpenChange(false)
                        }}
                        data-testid={`object-picker-use-${object.key}`}
                      >
                        Use
                      </Button>
                    </div>
                  </div>
                ))}

                {objectsQuery.hasNextPage ? (
                  <div className="flex items-center justify-end border-t border-[hsl(var(--border))] bg-[hsl(var(--background))] px-3 py-2">
                    <Button
                      variant="secondary"
                      size="sm"
                      onClick={() => objectsQuery.fetchNextPage()}
                      disabled={objectsQuery.isFetching}
                      data-testid="object-picker-load-more"
                    >
                      Load more
                    </Button>
                  </div>
                ) : null}
              </div>
            )}
          </div>
        </div>
      </DialogContent>
    </Dialog>
  )
}
