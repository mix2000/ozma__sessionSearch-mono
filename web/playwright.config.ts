import { defineConfig, devices } from '@playwright/test';

/* Порты явные и свободные: не 4477 (рабочий экземпляр), не 38000/48000 (builder).
 * Оба сервера поднимает и гасит сам Playwright. */
const MOCK_PORT = process.env.E2E_MOCK_PORT ?? '4597';
const WEB_PORT = process.env.E2E_WEB_PORT ?? '5197';
const BASE_URL = `http://127.0.0.1:${WEB_PORT}`;

export default defineConfig({
  testDir: './e2e',
  fullyParallel: false,
  forbidOnly: !!process.env.CI,
  retries: process.env.CI ? 1 : 0,
  workers: 1,
  reporter: process.env.CI ? [['list'], ['html', { open: 'never' }]] : 'list',
  use: {
    baseURL: BASE_URL,
    locale: 'ru-RU',
    trace: 'on-first-retry',
    screenshot: 'only-on-failure',
  },
  projects: [{ name: 'chromium', use: { ...devices['Desktop Chrome'] } }],
  webServer: [
    {
      command: 'node mock/mock-api.mjs',
      env: { MOCK_PORT },
      url: `http://127.0.0.1:${MOCK_PORT}/api/agents/transcripts/status`,
      reuseExistingServer: false,
      timeout: 30_000,
    },
    {
      command: `npx vite --port ${WEB_PORT} --strictPort`,
      env: { SESSION_SEARCH_API: `http://127.0.0.1:${MOCK_PORT}` },
      url: BASE_URL,
      reuseExistingServer: false,
      timeout: 60_000,
    },
  ],
});
