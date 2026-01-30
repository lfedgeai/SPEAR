import { QueryClient, QueryClientProvider } from '@tanstack/react-query'
import { cleanup, fireEvent, render, screen, waitFor } from '@testing-library/react'
import { MemoryRouter, Routes, Route, useLocation } from 'react-router-dom'
import { afterEach, beforeEach, describe, expect, it, vi } from 'vitest'

import InstanceDetailPage from '@/features/instances/InstanceDetailPage'

const mockListInstanceExecutions = vi.fn()

vi.mock('@/api/instanceExecution', () => ({
  listInstanceExecutions: (input: unknown) => mockListInstanceExecutions(input),
}))

function LocationDisplay() {
  const loc = useLocation()
  return <div data-testid="location">{loc.pathname}</div>
}

function renderPage(initialPath: string) {
  const client = new QueryClient({
    defaultOptions: { queries: { retry: false, staleTime: 0 } },
  })
  return render(
    <QueryClientProvider client={client}>
      <MemoryRouter initialEntries={[initialPath]}>
        <LocationDisplay />
        <Routes>
          <Route path="/instances/:instanceId" element={<InstanceDetailPage />} />
          <Route path="/executions/:executionId" element={<div>execution page</div>} />
        </Routes>
      </MemoryRouter>
    </QueryClientProvider>,
  )
}

describe('InstanceDetailPage', () => {
  beforeEach(() => {
    mockListInstanceExecutions.mockReset()
    mockListInstanceExecutions.mockResolvedValue({
      success: true,
      executions: [
        {
          execution_id: 'e-1',
          task_id: 't-1',
          status: 'completed',
          started_at_ms: 1000,
          completed_at_ms: 2000,
          function_name: 'f1',
        },
      ],
      next_page_token: '',
    })
  })

  afterEach(() => {
    vi.clearAllMocks()
    cleanup()
  })

  it('shows inferred task link when executions exist', async () => {
    renderPage('/instances/i-1')

    await screen.findByText('e-1')
    const viewTask = screen.getByRole('button', { name: 'View task' })
    expect(viewTask).not.toBeNull()
  })

  it('navigates to execution on row click', async () => {
    renderPage('/instances/i-1')

    await screen.findByText('e-1')
    const link = screen.getByRole('link', { name: 'e-1' })
    fireEvent.click(link.closest('tr')!)

    await waitFor(() => {
      expect(screen.getByTestId('location').textContent).toBe('/executions/e-1')
    })
  })
})
