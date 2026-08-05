import { defineConfig, devices } from '@playwright/test'

// Keep this host aligned with src-tauri/tauri.conf.json and Vite's bind host.
// On hosted Linux runners, localhost and 127.0.0.1 can resolve to different
// address families, causing Playwright's web-server probe to time out.
const WEB_APP_URL = 'http://localhost:28727'

export default defineConfig({
  testDir: './tests/e2e',
  fullyParallel: true,
  forbidOnly: Boolean(process.env.CI),
  retries: process.env.CI ? 1 : 0,
  workers: process.env.CI ? 1 : undefined,
  reporter: process.env.CI ? [['list'], ['html', { open: 'never' }]] : 'list',
  use: {
    baseURL: WEB_APP_URL,
    trace: 'retain-on-failure',
    screenshot: 'only-on-failure',
  },
  projects: [
    {
      name: 'chromium',
      use: { ...devices['Desktop Chrome'] },
    },
  ],
  webServer: {
    command: 'pnpm dev',
    url: WEB_APP_URL,
    reuseExistingServer: !process.env.CI,
    timeout: 60_000,
  },
})
