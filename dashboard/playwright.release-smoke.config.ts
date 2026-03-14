import { defineConfig, devices } from '@playwright/test';

const backendOrigin = process.env.PLAYWRIGHT_RELEASE_BACKEND_URL ?? 'http://127.0.0.1:3000';

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
      url: `${backendOrigin}/health`,
      reuseExistingServer: false,
      timeout: 180_000,
    },
    {
      command: `npm run build && PORT=3101 BACKEND_ORIGIN=${backendOrigin} node serve-spa.mjs`,
      cwd: '.',
      url: 'http://127.0.0.1:3101',
      reuseExistingServer: false,
      timeout: 180_000,
      env: {
        ...process.env,
        SPARKLYTICS_AUTH: 'local',
        PLAYWRIGHT_RELEASE_BACKEND_URL: backendOrigin,
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
