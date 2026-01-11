import { test, expect } from '@playwright/test';

test('deletes a file and updates list', async ({ page }) => {
  await page.goto('/admin');
  await page.getByTestId('nav-files').click();

  const fileInput = page.getByTestId('files-input');
  const payload = (globalThis as any).Buffer.from('bye');
  await fileInput.setInputFiles({ name: 'delete.txt', mimeType: 'text/plain', buffer: payload });
  await page.getByTestId('files-upload').click();

  const row = page.locator('[data-testid^="files-row-"]').filter({ hasText: 'delete.txt' }).first();
  await expect(row).toBeVisible({ timeout: 10_000 });

  const idText = await row.locator('button').locator('div').nth(1).textContent();
  expect(idText).toBeTruthy();
  const id = (idText || '').trim();

  await page.getByTestId(`files-delete-${id}`).click();
  await expect(page.getByText('Deleted', { exact: true })).toBeVisible({ timeout: 5_000 });

  await page.getByRole('button', { name: 'Refresh' }).click();
  await expect(page.getByTestId(`files-row-${id}`)).toHaveCount(0, { timeout: 30_000 });
});
