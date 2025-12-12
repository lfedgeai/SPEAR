import { test, expect } from '@playwright/test';

test('deletes a file and updates list', async ({ page }) => {
  await page.goto('/admin');
  await page.getByRole('menuitem', { name: 'Files' }).click();

  const fileInput = page.locator('input[type="file"]');
  await fileInput.setInputFiles({ name: 'delete.txt', mimeType: 'text/plain', buffer: Buffer.from('bye') });
  await page.getByRole('button', { name: /Upload/i }).click();
  await expect(page.getByText(/Uploaded:/)).toBeVisible({ timeout: 5000 });

  const rows = page.locator('.ant-table-row');
  const row = rows.filter({ has: page.getByRole('link', { name: 'Delete' }) }).first();
  await row.getByRole('link', { name: 'Delete' }).click();
  await expect(page.getByText('Deleted')).toBeVisible();
});
