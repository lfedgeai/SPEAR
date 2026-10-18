import { QueryClient, QueryClientProvider } from '@tanstack/react-query'
import { cleanup, fireEvent, render, screen, waitFor } from '@testing-library/react'
import { MemoryRouter, Routes, Route } from 'react-router-dom'
import { afterEach, beforeEach, describe, expect, it, vi } from 'vitest'

const mockListTasks = vi.fn()
const mockCreateTask = vi.fn()
const mockListObjects = vi.fn()
const mockListMcpServers = vi.fn()

vi.mock('@/api/tasks', () => ({
  listTasks: (input: unknown) => mockListTasks(input),
  createTask: (input: unknown) => mockCreateTask(input),
}))

vi.mock('@/api/objects', () => ({
  listObjects: (input: unknown) => mockListObjects(input),
}))

vi.mock('@/api/mcp', () => ({
  listMcpServers: () => mockListMcpServers(),
}))

vi.mock('@/features/tasks/DeleteTaskDialog', () => ({
  default: () => null,
}))

import TasksPage from '@/features/tasks/TasksPage'

function renderPage() {
  const client = new QueryClient({
    defaultOptions: { queries: { retry: false, staleTime: 0 } },
  })
  return render(
    <QueryClientProvider client={client}>
      <MemoryRouter initialEntries={['/tasks']}>
        <Routes>
          <Route path="/tasks" element={<TasksPage />} />
          <Route path="/tasks/:taskId" element={<div>task detail</div>} />
        </Routes>
      </MemoryRouter>
    </QueryClientProvider>,
  )
}

describe('TasksPage', () => {
  beforeEach(() => {
    mockListTasks.mockReset()
    mockCreateTask.mockReset()
    mockListObjects.mockReset()
    mockListMcpServers.mockReset()

    mockListTasks.mockResolvedValue({
      tasks: [
        {
          task_id: 't-1',
          name: 'demo-task',
          endpoint: 'demo',
          status: 'registered',
          priority: 'normal',
          desired_replicas: 3,
          active_instances: 1,
          ready_instances: 1,
          reconciling: true,
          underprovisioned: true,
          scheduling_strategy: 'spread',
          registered_at: 1_700_000_000,
          version: 'v1',
          last_heartbeat: 1_700_000_000,
        },
      ],
      total_count: 1,
    })
    mockListObjects.mockResolvedValue({ items: [], totalCount: 0 })
    mockListMcpServers.mockResolvedValue({ success: true, servers: [] })
    mockCreateTask.mockResolvedValue({ success: true, task_id: 'task-new' })
  })

  afterEach(() => {
    vi.clearAllMocks()
    cleanup()
  })

  it('renders replica convergence summary in task rows', async () => {
    renderPage()

    await screen.findByText('Observe workload specs, replica targets, and current replica convergence')
    await screen.findByText('Task / Endpoint')
    await screen.findByText('Replica state')
    await screen.findByText('demo-task')
    await screen.findByText('demo')
    await screen.findByText('reconciling')
    await screen.findByText('active 1 / ready 1')
    await screen.findByText('3 x spread')
  })

  it('submits object executable references as structured payloads', async () => {
    renderPage()

    fireEvent.click(await screen.findByTestId('tasks-open-create'))
    fireEvent.change(await screen.findByPlaceholderText('Name'), {
      target: { value: 'demo object task' },
    })
    fireEvent.change(screen.getByTestId('task-executable-type'), {
      target: { value: 'wasm' },
    })
    fireEvent.change(screen.getByTestId('task-executable-object-key'), {
      target: { value: 'artifacts/demo.wasm' },
    })

    fireEvent.click(screen.getByRole('button', { name: 'Create' }))

    await waitFor(() => {
      expect(mockCreateTask).toHaveBeenCalledWith(
        expect.objectContaining({
          name: 'demo object task',
          executable: expect.objectContaining({
            type: 'wasm',
            reference: {
              kind: 'object',
              object_key: 'artifacts/demo.wasm',
            },
          }),
        }),
      )
    })
  })
})
