import { test, expect } from '@playwright/test';

test('scheme selection prefills URI input', async ({ page }) => {
  await page.goto('/admin');
  await page.getByRole('menuitem', { name: 'Tasks' }).click();
  await page.getByRole('button', { name: 'Create Task' }).click();

  await page.selectOption('select[aria-label="No Executable"]', { label: 'WASM' });

  await page.selectOption('select[aria-label="Scheme"]', { label: 'https' });
  const uriInput = page.getByPlaceholder('Executable URI');
  await expect(uriInput).toHaveValue('https://');

  await page.selectOption('select[aria-label="Scheme"]', { label: 's3' });
  await expect(uriInput).toHaveValue('s3://');
});
