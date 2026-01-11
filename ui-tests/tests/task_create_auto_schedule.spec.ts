import { test, expect } from '@playwright/test';

test('create task supports auto schedule without choosing node', async ({ page }) => {
  await page.goto('/admin');
  await page.getByTestId('nav-tasks').click();
  await page.getByTestId('tasks-open-create').click();

  await expect(page.getByTestId('task-executable-type')).toBeVisible();
  await expect(page.getByTestId('task-node')).toBeVisible();
  await expect(page.getByTestId('task-node')).toHaveValue('');

  const unique = `${Date.now()}-${Math.random().toString(16).slice(2)}`;
  await page.getByRole('textbox', { name: 'Name' }).fill(`auto-${unique}`);
  await page.getByPlaceholder(/^Endpoint$/).fill(`/e2e/${unique}`);
  await page.getByPlaceholder(/^Version$/).fill('v1');

  const respPromise = page.waitForResponse((resp) => {
    return resp.url().includes('/admin/api/tasks') && resp.request().method() === 'POST';
  });
  await page.getByRole('button', { name: 'Create' }).click();

  const resp = await respPromise;
  expect(resp.status()).toBe(200);
  const body = await resp.json();
  expect(body.success).toBeTruthy();
  expect(body.task_id).toBeTruthy();
});
