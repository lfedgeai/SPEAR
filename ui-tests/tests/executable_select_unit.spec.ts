import { test, expect } from '@playwright/test';

test('executable type dropdown selects WASM and updates value', async ({ page }) => {
  await page.goto('/admin');
  await page.getByTestId('nav-tasks').click();
  await page.getByTestId('tasks-open-create').click();

  await expect(page.getByTestId('task-executable-type')).toBeVisible();
  await expect(page.getByTestId('task-node')).toBeVisible();
  await expect(page.getByTestId('task-node')).toHaveValue('');

  await page.getByTestId('task-executable-type').selectOption('wasm');
  await expect(page.getByTestId('task-executable-type')).toHaveValue('wasm');
});
