import { defineConfig } from '@playwright/test';

export default defineConfig({
  testDir: './tests',
  timeout: 30_000,
  globalSetup: './global-setup.ts',
  use: {
    baseURL: 'http://127.0.0.1:8081',
    headless: true,
    permissions: ['clipboard-read', 'clipboard-write'],
  },
  webServer: {
    command: 'cargo run --bin sms -- --http-addr 127.0.0.1:8080 --enable-web-admin --web-admin-addr 127.0.0.1:8081',
    url: 'http://127.0.0.1:8081/admin',
    reuseExistingServer: false,
    timeout: 120_000,
  },
});
