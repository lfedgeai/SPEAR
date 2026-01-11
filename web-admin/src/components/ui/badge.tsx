import * as React from 'react'

import { cn } from '@/lib/utils'

type Variant = 'default' | 'secondary' | 'success' | 'destructive'

export function Badge(
  props: React.HTMLAttributes<HTMLSpanElement> & { variant?: Variant },
) {
  const { className, variant = 'default', ...rest } = props
  return (
    <span
      className={cn(
        'inline-flex items-center rounded-full border px-2 py-0.5 text-xs font-medium',
        variant === 'default' &&
          'border-[hsl(var(--border))] bg-[hsl(var(--secondary))] text-[hsl(var(--secondary-foreground))]',
        variant === 'secondary' &&
          'border-[hsl(var(--border))] bg-transparent text-[hsl(var(--foreground))]',
        variant === 'success' && 'border-transparent bg-emerald-600 text-white',
        variant === 'destructive' &&
          'border-transparent bg-[hsl(var(--destructive))] text-[hsl(var(--destructive-foreground))]',
        className,
      )}
      {...rest}
    />
  )
}

