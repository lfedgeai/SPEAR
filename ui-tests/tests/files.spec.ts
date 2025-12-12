import { test, expect } from '@playwright/test';

test.describe('Files page', () => {
  test('uploads small file and copies URI', async ({ page }) => {
    await page.goto('/admin');
    await page.getByRole('menuitem', { name: 'Files' }).click();

    const fileInput = page.locator('input[type="file"]');
    await fileInput.setInputFiles({ name: 'hello.txt', mimeType: 'text/plain', buffer: Buffer.from('hello') });
    await page.getByRole('button', { name: /Upload/i }).click();

    await expect(page.getByText(/Uploaded:/)).toBeVisible({ timeout: 5000 });

    const firstRow = page.locator('.ant-table-row').first();
    const idCellText = await firstRow.locator('td').first().textContent();
    expect(idCellText).toBeTruthy();

    const copyLink = firstRow.getByRole('link', { name: 'Copy URI' });
    await copyLink.click();

    // Clipboard may require a small delay
    await page.waitForTimeout(100);
    const clip = await page.evaluate(async () => {
      try { return await navigator.clipboard.readText(); } catch { return ''; }
    });
    expect(clip).toMatch(/^sms\+file:\/\//);
  });
});
