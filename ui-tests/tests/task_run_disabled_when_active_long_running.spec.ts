import { test, expect } from '@playwright/test';

test('run is disabled when long-running task is active', async ({ page }) => {
  const taskId = 'long-running-active-task';

  await page.route('**/admin/api/tasks**', async (route) => {
    await route.fulfill({
      status: 200,
      contentType: 'application/json',
      body: JSON.stringify({
        total_count: 1,
        tasks: [
          {
            task_id: taskId,
            name: 'lr-active',
            status: 'active',
            priority: 'normal',
            node_uuid: '00000000-0000-0000-0000-000000000000',
            endpoint: '/tasks/lr-active',
            version: 'v1',
            execution_kind: 'long_running',
            registered_at: Math.floor(Date.now() / 1000),
            last_heartbeat: Math.floor(Date.now() / 1000),
          },
        ],
      }),
    });
  });

  await page.goto('/admin');
  await page.getByTestId('nav-tasks').click();

  await expect(page.getByTestId(`task-run-${taskId}`)).toBeDisabled();
});

