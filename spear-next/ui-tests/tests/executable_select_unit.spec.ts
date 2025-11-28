import { test, expect } from '@playwright/test';

test('executable type dropdown selects WASM and updates value', async ({ page }) => {
  await page.goto('/admin');
  await page.getByRole('menuitem', { name: 'Tasks' }).click();
  await page.getByRole('button', { name: 'Create Task' }).click();

  await page.selectOption('select[aria-label="No Executable"]', { label: 'WASM' });
  await expect(page.locator('select[aria-label="No Executable"]')).toHaveValue('wasm');
});
