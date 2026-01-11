import { test, expect } from '@playwright/test';

test.describe('Files page', () => {
  test('uploads small file and copies URI', async ({ page }) => {
    await page.goto('/admin');
    await page.getByTestId('nav-files').click();

    const fileInput = page.getByTestId('files-input');
    await fileInput.setInputFiles({ name: 'hello.txt', mimeType: 'text/plain', buffer: Buffer.from('hello') });

    await page.getByTestId('files-upload').click();

    const row = page.locator('[data-testid^="files-row-"]').filter({ hasText: 'hello.txt' }).first();
    await expect(row).toBeVisible({ timeout: 10_000 });

    const idText = await row.locator('button').locator('div').nth(1).textContent();
    expect(idText).toBeTruthy();
    const id = (idText || '').trim();

    await row.getByRole('button', { name: 'Copy URI' }).click();

    // Clipboard may require a small delay
    await page.waitForTimeout(100);
    const clip = await page.evaluate(async () => {
      try { return await navigator.clipboard.readText(); } catch { return ''; }
    });
    expect(clip).toBe(`sms+file://${id}`);
  });
});
