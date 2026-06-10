import { defineConfig } from '@playwright/test'

for (const proxyVariable of [
  'HTTP_PROXY',
  'HTTPS_PROXY',
  'ALL_PROXY',
  'http_proxy',
  'https_proxy',
  'all_proxy',
]) {
  delete process.env[proxyVariable]
}

const baseURL = process.env.PLAYWRIGHT_BASE_URL || 'http://127.0.0.1:33002'
const webServerURL = new URL(baseURL)
const webServerPort = webServerURL.port || (webServerURL.protocol === 'https:' ? '443' : '80')

export default defineConfig({
  testDir: './e2e',
  fullyParallel: false,
  workers: 1,
  timeout: 60_000,
  expect: {
    timeout: 10_000,
  },
  reporter: process.env.CI ? [['github'], ['html', { open: 'never' }]] : [['list'], ['html', { open: 'never' }]],
  use: {
    baseURL,
    launchOptions: {
      args: ['--no-proxy-server'],
    },
    trace: 'retain-on-failure',
    screenshot: 'only-on-failure',
    video: 'retain-on-failure',
  },
  webServer: process.env.PLAYWRIGHT_SKIP_WEBSERVER
    ? undefined
    : {
        command: `npm run dev -- --host 127.0.0.1 --port ${webServerPort}`,
        url: baseURL,
        reuseExistingServer: false,
        timeout: 120_000,
      },
})
