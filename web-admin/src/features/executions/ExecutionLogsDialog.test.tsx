import { QueryClient, QueryClientProvider } from '@tanstack/react-query'
import { fireEvent, render, screen } from '@testing-library/react'
import { beforeEach, describe, expect, it, vi } from 'vitest'

import ExecutionLogsDialog from '@/features/executions/ExecutionLogsDialog'

const mockGetExecutionLogs = vi.fn()

vi.mock('@/api/logs', () => ({
  getExecutionLogs: (input: unknown) => mockGetExecutionLogs(input),
}))

function renderWithQuery(ui: React.ReactNode) {
  const client = new QueryClient({
    defaultOptions: { queries: { retry: false, staleTime: 0 } },
  })
  return render(<QueryClientProvider client={client}>{ui}</QueryClientProvider>)
}

describe('ExecutionLogsDialog', () => {
  beforeEach(() => {
    localStorage.clear()
    mockGetExecutionLogs.mockReset()
    mockGetExecutionLogs.mockResolvedValue({
      success: true,
      execution_id: 'e1',
      lines: [
        {
          ts_ms: 1,
          seq: 1,
          stream: 'stdout',
          level: 'info',
          message: 'hello world',
        },
      ],
      next_cursor: '0',
      truncated: false,
      completed: true,
    })
  })

  it('toggles line wrap', async () => {
    renderWithQuery(
      <ExecutionLogsDialog open={true} onOpenChange={() => {}} executionId="e1" />,
    )

    const pre = await screen.findByTestId('execution-logs-pre')
    expect(pre.className).toContain('whitespace-pre')
    expect(pre.className).not.toContain('whitespace-pre-wrap')

    const wrapButton = screen.getByRole('button', { name: 'Wrap' })
    fireEvent.click(wrapButton)

    expect(pre.className).toContain('whitespace-pre-wrap')
    expect(screen.getByRole('button', { name: 'No wrap' })).not.toBeNull()
    expect(localStorage.getItem('ADMIN_EXECUTION_LOG_WRAP')).toBe('1')
  })

  it('reads wrap setting from localStorage', async () => {
    localStorage.setItem('ADMIN_EXECUTION_LOG_WRAP', '1')

    renderWithQuery(
      <ExecutionLogsDialog open={true} onOpenChange={() => {}} executionId="e1" />,
    )

    const pre = await screen.findByTestId('execution-logs-pre')
    expect(pre.className).toContain('whitespace-pre-wrap')
    expect(screen.getByRole('button', { name: 'No wrap' })).not.toBeNull()
  })
})
