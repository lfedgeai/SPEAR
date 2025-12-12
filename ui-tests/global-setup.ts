import fs from 'fs';
import path from 'path';
import type { FullConfig } from '@playwright/test';

async function globalSetup(_config: FullConfig) {
  try {
    // Clean SMS embedded files directory to ensure deterministic UI tests
    const root = path.resolve(__dirname, '..');
    const dataDir = path.join(root, 'data', 'files');
    if (fs.existsSync(dataDir)) {
      fs.rmSync(dataDir, { recursive: true, force: true });
    }
  } catch (e) {
    // Ignore cleanup errors
  }
}

export default globalSetup;
