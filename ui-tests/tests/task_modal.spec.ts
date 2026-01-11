import { test, expect } from '@playwright/test';

test.describe('Task create modal', () => {
  test('selects sms+file and inserts URI from picker', async ({ page }) => {
    await page.goto('/admin');

    await page.getByTestId('nav-files').click();
    const fileInput = page.getByTestId('files-input');
    const unique = `${Date.now()}-${Math.random().toString(16).slice(2)}`;
    const filename = `artifact-${unique}.wasm`;
    await fileInput.setInputFiles({ name: filename, mimeType: 'application/wasm', buffer: Buffer.from('00', 'hex') });

    const uploadResp = page.waitForResponse((resp) => {
      return (
        resp.url().includes('/admin/api/files') &&
        !resp.url().includes('presign-upload') &&
        resp.request().method() === 'POST' &&
        resp.status() === 200
      )
    })
    await page.getByTestId('files-upload').click();

    await expect(page.getByText('All uploads completed', { exact: true })).toBeVisible({ timeout: 10_000 });

    const uploaded = await uploadResp
    const body = (await uploaded.json()) as { success?: boolean; id?: string }
    expect(body.success).toBeTruthy()
    expect(body.id).toBeTruthy()
    const id = body.id as string
    const uri = `sms+file://${id}`

    await page.getByTestId('nav-tasks').click();
    await page.getByTestId('tasks-open-create').click();

    await page.getByTestId('task-executable-type').selectOption('wasm');
    await page.getByTestId('task-uri-scheme').selectOption('sms+file');

    await page.getByTestId('task-choose-local').click();
    await expect(page.getByTestId(`task-use-file-${id}`)).toBeVisible({ timeout: 10_000 });
    await page.getByTestId(`task-use-file-${id}`).click();

    await expect(page.getByTestId('task-executable-uri')).toHaveValue(uri);
  });
});
