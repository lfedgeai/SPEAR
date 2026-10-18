import { describe, expect, it } from 'vitest'
import { formatDisconnectError } from './connection'

describe('formatDisconnectError', () => {
  it('returns reason when the close frame provides one', () => {
    expect(
      formatDisconnectError({
        code: 1011,
        reason: 'runtime failed',
        wasClean: false,
      }),
    ).toBe('websocket disconnected (code 1011): runtime failed')
  })

  it('describes unexpected close when no reason is present', () => {
    expect(
      formatDisconnectError({
        code: 1006,
        reason: '',
        wasClean: false,
      }),
    ).toBe('websocket disconnected (code 1006): connection closed unexpectedly')
  })

  it('does not surface an error for clean normal closure', () => {
    expect(
      formatDisconnectError({
        code: 1000,
        reason: '',
        wasClean: true,
      }),
    ).toBe('')
  })
})
