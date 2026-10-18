import { test, expect } from '@playwright/test';

test('modified column renders a non-empty timestamp', async ({ page }) => {
  await page.goto('/admin');
  await page.getByTestId('nav-objects').click();

  const fileInput = page.getByTestId('objects-input');
  await fileInput.setInputFiles({ name: 'date.txt', mimeType: 'text/plain', buffer: Buffer.from('date') });
  await page.getByTestId('objects-upload').click();

  const row = page.locator('[data-testid^="objects-row-"]').filter({ hasText: 'date.txt' }).first();
  await expect(row).toBeVisible({ timeout: 10_000 });

  const modified = (await row.locator('div.col-span-3').first().textContent()) || '';
  expect(modified.trim()).not.toBe('');
  expect(modified.trim()).not.toBe('-');
});
