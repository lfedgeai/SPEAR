import { describe, expect, it } from 'vitest'

import { cn, isValidEndpointName, normalizeEndpointName } from '@/lib/utils'

describe('cn', () => {
  it('merges class names', () => {
    const maybe = undefined as string | undefined
    expect(cn('a', maybe, 'c')).toBe('a c')
  })
})

describe('normalizeEndpointName', () => {
  it('normalizes from task name', () => {
    expect(normalizeEndpointName('Hello World')).toBe('hello-world')
    expect(normalizeEndpointName('  Echo_01 ')).toBe('echo_01')
  })

  it('rejects invalid shapes via isValidEndpointName', () => {
    expect(isValidEndpointName('test')).toBe(true)
    expect(isValidEndpointName('/test/test')).toBe(false)
    expect(isValidEndpointName('tasks/test')).toBe(false)
  })
})
