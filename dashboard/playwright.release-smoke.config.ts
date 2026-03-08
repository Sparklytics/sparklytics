import { defineConfig, devices } from '@playwright/test';

export default defineConfig({
  testDir: './e2e',
  testMatch: /release-smoke\.spec\.ts/,
  fullyParallel: false,
  forbidOnly: !!process.env.CI,
  retries: process.env.CI ? 1 : 0,
  workers: 1,
  timeout: 120_000,
  use: {
    baseURL: 'http://127.0.0.1:3101',
    trace: 'on-first-retry',
  },
  webServer: [
    {
      command: 'node scripts/start-fresh-selfhosted-backend.mjs',
      cwd: '..',
      url: 'http://127.0.0.1:3000/health',
      reuseExistingServer: false,
      timeout: 180_000,
    },
    {
      command: 'npx next dev --port 3101',
      cwd: '.',
      url: 'http://127.0.0.1:3101',
      reuseExistingServer: false,
      env: {
        ...process.env,
        SPARKLYTICS_AUTH: 'local',
      },
    },
  ],
  projects: [
    {
      name: 'chromium',
      use: { ...devices['Desktop Chrome'] },
    },
  ],
});
