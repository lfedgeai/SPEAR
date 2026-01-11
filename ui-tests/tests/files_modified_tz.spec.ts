import { test, expect } from '@playwright/test';

test('modified column renders a non-empty timestamp', async ({ page }) => {
  await page.goto('/admin');
  await page.getByTestId('nav-files').click();

  const fileInput = page.getByTestId('files-input');
  await fileInput.setInputFiles({ name: 'date.txt', mimeType: 'text/plain', buffer: Buffer.from('date') });
  await page.getByTestId('files-upload').click();

  const row = page.locator('[data-testid^="files-row-"]').filter({ hasText: 'date.txt' }).first();
  await expect(row).toBeVisible({ timeout: 10_000 });

  const modified = (await row.locator('div.col-span-3').first().textContent()) || '';
  expect(modified.trim()).not.toBe('');
  expect(modified.trim()).not.toBe('-');
});
