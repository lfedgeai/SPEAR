import * as DialogPrimitive from '@radix-ui/react-dialog'

import { cn } from '@/lib/utils'

export const Dialog = DialogPrimitive.Root
export const DialogTrigger = DialogPrimitive.Trigger
export const DialogClose = DialogPrimitive.Close

export function DialogContent(
  props: DialogPrimitive.DialogContentProps & { className?: string },
) {
  const { className, ...rest } = props
  return (
    <DialogPrimitive.Portal>
      <DialogPrimitive.Overlay className="fixed inset-0 bg-black/50" />
      <DialogPrimitive.Content
        className={cn(
          'fixed left-1/2 top-1/2 w-[min(820px,calc(100vw-24px))] -translate-x-1/2 -translate-y-1/2 rounded-[var(--radius)] border border-[hsl(var(--border))] bg-[hsl(var(--background))] p-5 text-[hsl(var(--foreground))] shadow-xl outline-none',
          className,
        )}
        {...rest}
      />
    </DialogPrimitive.Portal>
  )
}

export function DialogHeader(props: { title: string; description?: string }) {
  return (
    <div className="mb-4">
      <div className="text-base font-semibold">{props.title}</div>
      {props.description ? (
        <div className="mt-1 text-sm text-[hsl(var(--muted-foreground))]">
          {props.description}
        </div>
      ) : null}
    </div>
  )
}

