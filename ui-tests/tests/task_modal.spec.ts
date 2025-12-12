import { test, expect } from '@playwright/test';

test.describe('Task create modal', () => {
  test('selects sms+file and inserts URI from picker', async ({ page }) => {
    await page.goto('/admin');
    await page.getByRole('menuitem', { name: 'Tasks' }).click();
    await page.getByRole('button', { name: 'Create Task' }).click();

    await page.selectOption('select[aria-label="No Executable"]', { label: 'WASM' });

    await page.selectOption('select[aria-label="Scheme"]', { label: 'sms+file' });

    await page.getByRole('button', { name: 'Choose Local' }).click();
    const useLink = page.getByRole('link', { name: 'Use' }).first();
    await useLink.click();

    const uriInput = page.getByPlaceholder('Executable URI');
    await expect(uriInput).toHaveValue(/^sms\+file:\/\//);
  });
});
