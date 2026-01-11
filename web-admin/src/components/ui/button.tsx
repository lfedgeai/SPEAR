import * as React from 'react'

import { cn } from '@/lib/utils'

type Variant = 'default' | 'secondary' | 'ghost' | 'destructive'
type Size = 'sm' | 'md' | 'lg'

export function Button(
  props: React.ButtonHTMLAttributes<HTMLButtonElement> & {
    variant?: Variant
    size?: Size
  },
) {
  const { className, variant = 'default', size = 'md', ...rest } = props

  return (
    <button
      className={cn(
        'inline-flex items-center justify-center gap-2 whitespace-nowrap rounded-[calc(var(--radius)-2px)] text-sm font-medium transition-colors focus-visible:outline-none focus-visible:ring-2 focus-visible:ring-[hsl(var(--ring))] focus-visible:ring-offset-2 disabled:pointer-events-none disabled:opacity-50 ring-offset-[hsl(var(--background))]',
        variant === 'default' &&
          'bg-[hsl(var(--primary))] text-[hsl(var(--primary-foreground))] hover:opacity-90',
        variant === 'secondary' &&
          'bg-[hsl(var(--secondary))] text-[hsl(var(--secondary-foreground))] hover:opacity-90',
        variant === 'ghost' &&
          'bg-transparent text-[hsl(var(--foreground))] hover:bg-[hsl(var(--accent))]',
        variant === 'destructive' &&
          'bg-[hsl(var(--destructive))] text-[hsl(var(--destructive-foreground))] hover:opacity-90',
        size === 'sm' && 'h-8 px-3',
        size === 'md' && 'h-9 px-4',
        size === 'lg' && 'h-10 px-5',
        className,
      )}
      {...rest}
    />
  )
}

