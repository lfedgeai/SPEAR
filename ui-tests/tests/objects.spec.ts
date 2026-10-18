import { test, expect } from '@playwright/test';

test.describe('Objects page', () => {
  test('uploads small file and shows object metadata', async ({ page }) => {
    await page.goto('/admin');
    await page.getByTestId('nav-objects').click();

    const fileInput = page.getByTestId('objects-input');
    await fileInput.setInputFiles({ name: 'hello.txt', mimeType: 'text/plain', buffer: Buffer.from('hello') });

    await page.getByTestId('objects-upload').click();

    const row = page.locator('[data-testid^="objects-row-"]').filter({ hasText: 'hello.txt' }).first();
    await expect(row).toBeVisible({ timeout: 10_000 });

    await expect(row.getByText('Ready')).toBeVisible();
    await expect(row.getByRole('link', { name: 'Download' })).toBeVisible();
  });
});
