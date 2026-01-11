import { test, expect } from '@playwright/test';

test('scheme selection prefills URI input', async ({ page }) => {
  await page.goto('/admin');
  await page.getByTestId('nav-tasks').click();
  await page.getByTestId('tasks-open-create').click();

  await page.getByTestId('task-executable-type').selectOption('wasm');

  await page.getByTestId('task-uri-scheme').selectOption('https');
  await expect(page.getByTestId('task-executable-uri')).toHaveValue('https://');

  await page.getByTestId('task-uri-scheme').selectOption('s3');
  await expect(page.getByTestId('task-executable-uri')).toHaveValue('s3://');
});
