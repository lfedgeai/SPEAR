import { test, expect } from '@playwright/test';

test.describe('Task create modal', () => {
  test('selects object and inserts key from picker', async ({ page }) => {
    await page.goto('/admin');

    await page.getByTestId('nav-objects').click();
    const fileInput = page.getByTestId('objects-input');
    const unique = `${Date.now()}-${Math.random().toString(16).slice(2)}`;
    const filename = `artifact-${unique}.wasm`;
    await fileInput.setInputFiles({ name: filename, mimeType: 'application/wasm', buffer: Buffer.from('00', 'hex') });

    const uploadResp = page.waitForResponse((resp) => {
      return (
        resp.url().includes('/admin/api/objects') &&
        resp.request().method() === 'POST' &&
        resp.status() === 200
      )
    })
    await page.getByTestId('objects-upload').click();

    await expect(page.getByText('All uploads completed', { exact: true })).toBeVisible({ timeout: 10_000 });

    const uploaded = await uploadResp
    const body = (await uploaded.json()) as { success?: boolean; key?: string }
    expect(body.success).toBeTruthy()
    expect(body.key).toBeTruthy()
    const key = body.key as string

    await page.getByTestId('nav-tasks').click();
    await page.getByTestId('tasks-open-create').click();

    await page.getByTestId('task-executable-type').selectOption('wasm');
    await page.getByTestId('task-uri-scheme').selectOption('object');

    await page.getByTestId('task-choose-object').click();
    await expect(page.getByTestId(`object-picker-use-${key}`)).toBeVisible({ timeout: 10_000 });
    await page.getByTestId(`object-picker-use-${key}`).click();

    await expect(page.getByTestId('task-executable-object-key')).toHaveValue(key);
  });
});
