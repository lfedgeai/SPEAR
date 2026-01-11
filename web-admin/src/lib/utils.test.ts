import { describe, expect, it } from 'vitest'

import { cn } from '@/lib/utils'

describe('cn', () => {
  it('merges class names', () => {
    const maybe = undefined as string | undefined
    expect(cn('a', maybe, 'c')).toBe('a c')
  })
})

