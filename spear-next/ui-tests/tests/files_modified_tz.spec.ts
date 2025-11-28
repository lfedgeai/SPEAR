import { test, expect } from '@playwright/test';

test('modified column shows human-readable date in chosen TZ', async ({ page }) => {
  await page.goto('/admin');
  // Set timezone to UTC for deterministic format
  await page.evaluate(() => { localStorage.setItem('ADMIN_TZ', 'UTC'); });
  await page.reload();

  await page.getByRole('menuitem', { name: 'Files' }).click();
  const fileInput = page.locator('input[type="file"]');
  await fileInput.setInputFiles({ name: 'date.txt', mimeType: 'text/plain', buffer: Buffer.from('date') });
  await page.getByRole('button', { name: /Upload/i }).click();
  await expect(page.getByText(/Uploaded:/)).toBeVisible({ timeout: 5000 });

  const firstModified = await page.locator('.ant-table-row').first().locator('td').nth(3).textContent();
  expect(firstModified).toMatch(/\d{4}-\d{2}-\d{2} \d{2}:\d{2}:\d{2}/);
});
