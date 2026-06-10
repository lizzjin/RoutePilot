import { expect, test, type Page } from '@playwright/test'
import { csrfHeaders, login } from './helpers'

const routes = ['dashboard', 'models', 'logs', 'api-keys', 'users', 'quotas', 'enterprise', 'governance', 'operations', 'settings', 'guide']

async function expectContained(page: Page) {
  const overflow = await page.locator('#main-content').evaluate((main) => ({
    viewport: window.innerWidth,
    document: document.documentElement.scrollWidth,
    main: main.clientWidth,
    content: main.scrollWidth,
  }))
  expect(overflow.document).toBeLessThanOrEqual(overflow.viewport + 1)
  expect(overflow.content).toBeLessThanOrEqual(overflow.main + 1)
}

for (const theme of ['light', 'dark'] as const) {
  test(`${theme}: all routes fit desktop and mobile, and theme survives reload`, async ({ page }) => {
    test.setTimeout(120_000)
    await page.emulateMedia({ reducedMotion: 'reduce' })
    await login(page)
    await page.getByRole('button', { name: '切换界面主题' }).click()
    await page.getByRole('menuitem', { name: theme === 'light' ? '浅色' : '深色', exact: true }).click()
    for (const width of [1440, 390]) {
      await page.setViewportSize({ width, height: width === 390 ? 844 : 900 })
      for (const route of routes) {
        await page.goto(`/${route}`)
        await expect(page.locator('#main-content h1')).toBeVisible()
        await page.evaluate(() => document.fonts.ready)
        await expectContained(page)
        expect(await page.locator('html').evaluate((el) => el.classList.contains('dark'))).toBe(theme === 'dark')
      }
    }
  })
}

test('navigation breakpoint, focus containment, system theme and font fallback', async ({ page }) => {
  await login(page)
  for (const width of [768, 1023, 1024, 1920]) {
    await page.setViewportSize({ width, height: 1024 })
    await expectContained(page)
    await expect(page.getByRole('navigation', { name: '工作区', exact: true })).toBeVisible()
    await expect(page.getByRole('navigation', { name: '工作区页面', exact: true })).toBeVisible()
    await page.getByRole('navigation', { name: '工作区', exact: true }).getByRole('link', { name: '配置', exact: true }).click()
    await expect(page).toHaveURL(/models$/)
    await page.getByRole('navigation', { name: '工作区', exact: true }).getByRole('link', { name: '运行', exact: true }).click()
    await expect(page).toHaveURL(/dashboard$/)

  }
  await page.getByRole('button', { name: '切换界面主题' }).click()
  await page.getByRole('menuitem', { name: '跟随系统' }).click()
  await page.emulateMedia({ colorScheme: 'dark' })
  await expect(page.locator('html')).toHaveClass(/dark/)
  await page.emulateMedia({ colorScheme: 'light' })
  await expect(page.locator('html')).not.toHaveClass(/dark/)
  await page.route('**/fonts/*.woff2', route => route.abort())
  await page.reload()
  await expect(page.getByRole('heading', { name: '运行概览' })).toBeVisible()
  await expectContained(page)
})

test('virtual logs measure long rows, align columns and select the right record after scrolling and paging', async ({ page }) => {
  test.setTimeout(90_000)
  await login(page)
  await page.setViewportSize({ width: 1440, height: 900 })
  const logs = Array.from({ length: 205 }, (_, index) => ({
    id: `redesign-log-${index}`, requestId: `request-${index}-${'long-request-id-'.repeat(8)}`,
    timestamp: new Date().toISOString(), status: index % 3 === 0 ? 'error' : 'success', statusCode: index % 3 === 0 ? 502 : 200,
    provider: 'custom', protocol: 'openai-compat', clientProtocol: 'openai-chat-completions',
    userId: 'synthetic-user', username: 'layout-fixture', apiKeyGroup: 'long-group-name',
    model: `model-${index}-${'long-model-name-'.repeat(8)}`, resolvedModel: `resolved-${index}-${'long-model-name-'.repeat(8)}`,
    inputTokens: 123456789, outputTokens: 987654321, cacheReadTokens: 987654, cacheWriteTokens: 123456,
    latencyMs: 9000, firstByteLatencyMs: 500, costEstimate: 1234567.123456,
    billingMode: 'local-estimate+retry', reconciliationStatus: 'estimate_only',
    trafficClass: 'diagnostic', stream: 'stream', toolUseRequested: true,
    toolOutcome: 'very-long-tool-outcome-that-must-wrap-without-overlapping-the-next-record',
    errorMessage: index % 3 === 0 ? 'Long error evidence. '.repeat(30) : undefined,
  }))
  const summary = { totalRequests: logs.length, successRequests: 136, totalTokens: 999999999,
    totalCostEstimate: 1234567.1234, totalInputTokens: 123456789, totalOutputTokens: 987654321,
    totalCacheReadTokens: 987654, totalCacheWriteTokens: 123456, latencySampleCount: logs.length }
  await page.route('**/admin/logs?*', async route => {
    const url = new URL(route.request().url())
    const pageSize = Number(url.searchParams.get('pageSize')) || 50
    const current = Number(url.searchParams.get('page')) || 1
    const filtered = url.searchParams.get('search') ? logs.slice(0, 1) : logs
    await route.fulfill({ json: { logs: filtered.slice((current - 1) * pageSize, current * pageSize), total: filtered.length, summary } })
  })
  await page.route('**/admin/logs/redesign-log-*', route => {
    const id = new URL(route.request().url()).pathname.split('/').at(-1)
    return route.fulfill({ json: logs.find(log => log.id === id) })
  })
  await page.goto('/logs?pageSize=200')
  await expect(page.locator('[data-index="0"]')).toBeVisible()
  await page.evaluate(() => document.fonts.ready)
  const scroller = page.getByTestId('logs-scroll-viewport')
  const assertGeometry = async () => {
    const geometry = await page.locator('[data-index]').evaluateAll(elements => elements.map(el => {
      const bounds = el.getBoundingClientRect()
      return { top: bounds.top, bottom: bounds.bottom, height: bounds.height }
    }))
    expect(geometry.length).toBeGreaterThan(0)
    const gaps = geometry.slice(1).map((row, index) => Math.abs(row.top - geometry[index].bottom))
    expect(Math.max(0, ...gaps)).toBeLessThan(2)
    expect(geometry.some(row => row.height > 88)).toBeTruthy()

  }
  for (let offset = 0; offset < 30000; offset += 600) {
    await scroller.evaluate((el, top) => { el.scrollTop = top }, offset)
    // ResizeObserver and the virtualizer commit their measured sizes before
    // the following paint. Sample after those frames, with the font settled.
    await page.evaluate(() => new Promise<void>(resolve => requestAnimationFrame(() => requestAnimationFrame(() => resolve()))))
    await assertGeometry()
    if (await page.locator('[data-index="199"]').count()) break
  }
  await scroller.evaluate(el => { el.scrollTop = el.scrollHeight })
  const last = page.locator('[data-index="199"] button')
  await expect(last).toBeVisible()
  await last.focus()
  await page.keyboard.press('Enter')
  const drawer = page.getByRole('region', { name: '请求详情' })
  await expect(drawer).toBeVisible()
  await expect(drawer).toContainText(logs[199].requestId)
  await page.getByRole('button', { name: '返回请求列表', exact: true }).click()
  await expect(last).toBeFocused()
  await last.press('Space')
  await expect(drawer).toBeVisible()
  await page.getByRole('button', { name: '返回请求列表', exact: true }).click()
  await page.getByRole('button', { name: '下一页', exact: true }).click()
  await expect(page).toHaveURL(/page=2/)
  await expect(page.getByRole('button', { name: `查看 ${logs[200].resolvedModel} 请求详情` })).toBeVisible()
  await page.getByLabel('搜索请求日志').fill('fixture')
  await expect(page).not.toHaveURL(/page=2/)
  await expect(page.locator('[data-index]')).toHaveCount(1)
  // CSS zoom reproduces 200% layout geometry, including dynamic measurement.
  await page.locator('html').evaluate(el => { el.style.zoom = '2' })
  await expectContained(page)
  await page.locator('html').evaluate(el => { el.style.zoom = '' })
})

for (const role of ['user', 'viewer']) {
  test(`${role}: navigation, direct-route denial and write controls keep role boundaries`, async ({ page }) => {
    await login(page)
    const username = `e2e_redesign_${role}_${Date.now()}`
    const password = 'e2e-redesign-password-12345'
    const created = await page.request.post('/admin/users', { headers: csrfHeaders(), data: { username, email: `${username}@example.test`, password, role, status: 'active' } })
    expect(created.ok()).toBeTruthy()
    const { id } = await created.json() as { id: string }
    try {
      await page.getByRole('button', { name: '打开账户菜单' }).click()
      await page.getByRole('menuitem', { name: '退出登录' }).click()
      await page.locator('#username').fill(username)
      await page.locator('#password').fill(password)
      await page.getByRole('button', { name: '登录', exact: true }).click()
      await expect(page).toHaveURL(/dashboard$/)
      for (const denied of ['users', 'quotas', 'enterprise', 'governance', 'operations', 'settings']) {
        await expect(page.locator(`nav a[href="/${denied}"]`)).toHaveCount(0)
        await page.goto(`/${denied}`)
        await expect(page.getByRole('heading', { name: '无权访问此页面' })).toBeVisible()
      }
      await page.goto('/api-keys')
      await expect(page.getByRole('button', { name: '创建密钥', exact: true })).toHaveCount(0)
      await expectContained(page)
    } finally {
      await login(page)
      const removed = await page.request.delete(`/admin/users/${id}`, { headers: csrfHeaders() })
      expect(removed.ok()).toBeTruthy()
    }
  })
}

test('loading, first failure, retry, empty results and stale refresh remain distinguishable', async ({ page }) => {
  await login(page)
  await page.setViewportSize({ width: 390, height: 844 })
  let mode: 'loading' | 'error' | 'empty' = 'loading'
  let release: () => void = () => {}
  const pending = new Promise<void>(resolve => { release = resolve })
  await page.route('**/admin/logs?*', async route => {
    if (mode === 'loading') await pending
    if (mode === 'error') {
      await route.fulfill({ status: 500, json: { error: { message: '验收用网络故障' } } })
    } else {
      await route.fulfill({ json: { logs: [], total: 0, summary: { totalRequests: 0, successRequests: 0, totalTokens: 0 } } })
    }
  })
  await page.goto('/logs')
  await expect(page.getByRole('status', { name: '正在加载页面' })).toBeVisible()
  await expectContained(page)
  mode = 'error'
  release()
  await expect(page.getByRole('heading', { name: '请求日志加载失败' })).toBeVisible()
  mode = 'empty'
  await page.getByRole('button', { name: '重试', exact: true }).click()
  await expect(page.getByText('没有匹配的请求日志')).toBeVisible()
  await page.getByRole('button', { name: '自动刷新', exact: true }).click()
  await expect(page.getByRole('status')).toContainText('每 3 秒刷新当前结果')
  await expect(page.getByText('没有匹配的请求日志')).toBeVisible()
  mode = 'error'
  await expect(page.getByRole('alert')).toContainText('刷新失败，当前显示的是上次成功加载的数据', { timeout: 15_000 })
  await expect(page.getByText('没有匹配的请求日志')).toBeVisible()
  await expectContained(page)
})

test('Provider editor keeps its actions reachable and unsaved fields intact across viewport and theme changes', async ({ page }) => {
  await login(page)
  await page.goto('/models')
  await page.getByRole('tab', { name: 'Provider 与凭证', exact: true }).click()
  await page.locator('.workbench-directory').getByRole('button').filter({ hasText: 'custom' }).click()
  await page.getByTestId('provider-card-custom').getByRole('button', { name: '编辑', exact: true }).last().click()
  const dialog = page.getByRole('region', { name: '编辑供应商', exact: true })
  const name = dialog.getByLabel('显示名称', { exact: true })
  await name.fill('unsaved-layout-verification')
  for (const width of [390, 768, 1440]) {
    await page.setViewportSize({ width, height: 844 })
    await expect(dialog.getByRole('button', { name: '保存 Provider', exact: true })).toBeInViewport()
    await expect(dialog.getByRole('button', { name: '取消', exact: true })).toBeInViewport()
    await expect(name).toHaveValue('unsaved-layout-verification')
    await dialog.locator('#provider-static-headers').scrollIntoViewIfNeeded()
    await expect(dialog.locator('#provider-static-headers')).toBeInViewport()
    await expect(dialog.getByRole('button', { name: '保存 Provider', exact: true })).toBeInViewport()
  }
  await page.emulateMedia({ colorScheme: 'dark' })
  await expect(name).toHaveValue('unsaved-layout-verification')
  await dialog.getByRole('button', { name: '取消', exact: true }).click()
  await expect(dialog).toBeHidden()
})
