import { test, expect } from '@playwright/test';

test('can run a task manually after creating it', async ({ page }) => {
  await page.goto('/admin');
  await page.getByTestId('nav-tasks').click();
  await page.getByTestId('tasks-open-create').click();

  await expect(page.getByTestId('task-executable-type')).toBeVisible();

  const unique = `${Date.now()}-${Math.random().toString(16).slice(2)}`;
  await page.getByRole('textbox', { name: 'Name' }).fill(`manual-run-${unique}`);
  await page.getByPlaceholder(/^Endpoint$/).fill(`/e2e/${unique}`);
  await page.getByPlaceholder(/^Version$/).fill('v1');
  await page.getByTestId('task-executable-type').selectOption('no-executable');

  const createRespPromise = page.waitForResponse((resp) => {
    return resp.url().includes('/admin/api/tasks') && resp.request().method() === 'POST';
  });
  await page.getByRole('button', { name: 'Create' }).click();

  const createResp = await createRespPromise;
  expect(createResp.status()).toBe(200);
  const createBody = await createResp.json();
  expect(createBody.success).toBeTruthy();
  const taskId = createBody.task_id as string;
  expect(taskId).toBeTruthy();

  await expect(page.getByTestId(`task-row-${taskId}`)).toBeVisible();

  const execRespPromise = page.waitForResponse((resp) => {
    return resp.url().includes('/admin/api/executions') && resp.request().method() === 'POST';
  });
  await page.getByTestId(`task-run-${taskId}`).click();

  const execResp = await execRespPromise;
  expect(execResp.status()).toBe(200);
});

