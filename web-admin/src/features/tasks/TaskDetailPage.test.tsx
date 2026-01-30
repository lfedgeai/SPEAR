import { QueryClient, QueryClientProvider } from '@tanstack/react-query'
import { cleanup, fireEvent, render, screen, waitFor } from '@testing-library/react'
import { MemoryRouter, Routes, Route, useLocation } from 'react-router-dom'
import { afterEach, beforeEach, describe, expect, it, vi } from 'vitest'

import TaskDetailPage from '@/features/tasks/TaskDetailPage'

const mockGetTaskDetail = vi.fn()
const mockListTaskInstances = vi.fn()

vi.mock('@/api/tasks', () => ({
  getTaskDetail: (taskId: string) => mockGetTaskDetail(taskId),
}))

vi.mock('@/api/instanceExecution', () => ({
  listTaskInstances: (input: unknown) => mockListTaskInstances(input),
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
          <Route path="/tasks/:taskId" element={<TaskDetailPage />} />
          <Route path="/instances/:instanceId" element={<div>instance page</div>} />
        </Routes>
      </MemoryRouter>
    </QueryClientProvider>,
  )
}

describe('TaskDetailPage', () => {
  beforeEach(() => {
    mockGetTaskDetail.mockReset()
    mockListTaskInstances.mockReset()

    mockGetTaskDetail.mockResolvedValue({
      found: true,
      task: { name: 'demo-task' },
    })

    mockListTaskInstances.mockImplementation(
      (input: { task_id: string; page_token?: string }) => {
        if (!input.page_token) {
          return Promise.resolve({
            success: true,
            instances: [
              {
                instance_id: 'i-1',
                node_uuid: 'n-1',
                status: 'running',
                last_seen_ms: 1000,
                current_execution_id: 'e-1',
              },
            ],
            next_page_token: 'p2',
          })
        }
        return Promise.resolve({
          success: true,
          instances: [
            {
              instance_id: 'i-2',
              node_uuid: 'n-2',
              status: 'idle',
              last_seen_ms: 2000,
              current_execution_id: '',
            },
          ],
          next_page_token: '',
        })
      },
    )
  })

  afterEach(() => {
    vi.clearAllMocks()
    cleanup()
  })

  it('renders task overview and instances paging', async () => {
    renderPage('/tasks/t-1')

    await screen.findByText(/"found": true/)
    await screen.findByText('i-1')

    const loadMore = await screen.findByRole('button', { name: 'Load more' })
    fireEvent.click(loadMore)

    await screen.findByText('i-2')
  })

  it('navigates to instance on row click', async () => {
    renderPage('/tasks/t-1')

    await screen.findByText('i-1')

    const cell = screen.getByRole('link', { name: 'i-1' })
    fireEvent.click(cell.closest('tr')!)

    await waitFor(() => {
      expect(screen.getByTestId('location').textContent).toBe('/instances/i-1')
    })
  })
})
