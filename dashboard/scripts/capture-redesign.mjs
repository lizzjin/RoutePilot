import { chromium } from 'playwright'
import { mkdir, readFile, writeFile } from 'node:fs/promises'
import path from 'node:path'
import { fileURLToPath } from 'node:url'

// Run only against the local VITE_ROUTEPILOT_MOCK=1 server. These artifacts
// demonstrate presentation, never backend behavior or production data.
const origin = process.env.REDESIGN_MOCK_URL || 'http://127.0.0.1:33012'
if (!['127.0.0.1', 'localhost'].includes(new URL(origin).hostname)) throw new Error('A local mock server is required')
const output = fileURLToPath(new URL('../../docs/assets/routepilot-redesign/', import.meta.url))
await mkdir(output, { recursive: true })
const browser = await chromium.launch()
const context = await browser.newContext({ reducedMotion: 'reduce', colorScheme: 'light', viewport: { width: 1440, height: 900 } })
const page = await context.newPage()
const detailsOnly = process.env.REDESIGN_DETAILS_ONLY === '1'
const prior = detailsOnly ? JSON.parse(await readFile(path.join(output, 'visual-audit.json'), 'utf8')) : null
const results = prior ? prior.results.filter(result => !/-form$|^logs-drawer-|^dashboard-section-/.test(result.name)) : []
const errors = prior ? prior.errors : []
page.on('pageerror', error => errors.push(error.message))
await page.goto(`${origin}/login`)
await page.locator('#username').fill('admin')
await page.locator('#password').fill('admin')
await page.getByRole('button', { name: '登录', exact: true }).click()
await page.waitForURL('**/dashboard')
await page.getByText('演示数据', { exact: true }).waitFor({ state: 'visible', timeout: 10_000 })

async function capture(name, theme, viewport, { screenshot = true, audit = true } = {}) {
  await page.evaluate(() => document.fonts.ready)
  const geometry = await page.evaluate(() => {
    const main = document.querySelector('#main-content') || document.documentElement
    return { width: main.clientWidth, scrollWidth: main.scrollWidth, theme: document.documentElement.classList.contains('dark') ? 'dark' : 'light' }
  })
  if (geometry.scrollWidth > geometry.width + 1) errors.push(`${name} ${theme} ${viewport.width}: horizontal overflow ${geometry.scrollWidth} > ${geometry.width}`)
  let violations = []
  if (process.env.AXE_SOURCE && audit) {
    await page.addScriptTag({ path: process.env.AXE_SOURCE })
    const axeResult = await page.evaluate(() => window.axe.run(document, { runOnly: { type: 'tag', values: ['wcag2a', 'wcag2aa', 'wcag21aa'] } }))
    violations = axeResult.violations.map(({ id, nodes }) => ({ id, nodes: nodes.map(({ target, failureSummary }) => ({ target, failureSummary })) }))
  }
  if (screenshot) await page.screenshot({ path: path.join(output, `after-${name}-${theme}-${viewport.width}.png`) })
  results.push({ name, theme, viewport, geometry, violations })
  await writeFile(path.join(output, 'visual-audit.json'), JSON.stringify({ tool: 'Playwright + optional axe-core WCAG 2/2.1 A/AA', mockOnly: true, results, errors }, null, 2) + '\n')
  console.log(`${name} ${theme} ${viewport.width}: ${violations.length} accessibility violations`)
}

const allRoutes = ['dashboard', 'models', 'logs', 'api-keys', 'users', 'quotas', 'enterprise', 'governance', 'operations', 'settings', 'guide', 'login', 'missing-page']
if (!detailsOnly) {
for (const theme of ['light', 'dark']) {
  await page.evaluate(value => localStorage.setItem('routepilot_theme', value), theme)
  for (const [width, height] of [[1440, 900], [390, 844], [768, 1024], [1024, 768], [1920, 1080]]) {
    const viewport = { width, height }
    await page.setViewportSize(viewport)
    const routes = width === 1440 || width === 390 ? allRoutes : ['dashboard', 'models', 'logs', 'login']
    for (const route of routes) {
      await page.goto(`${origin}/${route}`)
      await page.waitForTimeout(900)
      await capture(route, theme, viewport)
    }
  }
  await page.setViewportSize({ width: 1440, height: 900 })
  for (const route of ['models', 'settings', 'enterprise', 'governance']) {
    await page.goto(`${origin}/${route}`)
    await page.waitForTimeout(900)
    const tabs = await page.getByRole('tab').allTextContents()
    for (let index = 0; index < tabs.length; index++) {
      const tab = page.getByRole('tab', { name: tabs[index], exact: true })
      if (!await tab.isEnabled()) continue
      await tab.click()
      await page.waitForTimeout(350)
      await capture(`${route}-tab-${index}`, theme, { width: 1440, height: 900 })
    }
  }
}}

for (const theme of ['light', 'dark']) {
  await page.evaluate(value => localStorage.setItem('routepilot_theme', value), theme)
  for (const width of [1440, 390]) {
    const viewport = { width, height: width === 390 ? 844 : 900 }
    await page.setViewportSize(viewport)
    for (const [route, button] of [['api-keys', '创建密钥'], ['users', '新建用户'], ['quotas', '新建配额']]) {
      await page.goto(`${origin}/${route}`)
      await page.getByRole('button', { name: button, exact: true }).click()
      await page.getByRole('dialog').waitFor({ state: 'visible' })
      await capture(`${route}-form`, theme, viewport)
      const bounds = await page.getByRole('dialog').boundingBox()
      if (bounds.x < 0 || bounds.y < 0 || bounds.x + bounds.width > width + 1 || bounds.y + bounds.height > viewport.height + 1) errors.push(`${route} dialog exceeds viewport at ${width}`)
      await page.keyboard.press('Escape')
    }
    await page.goto(`${origin}/models`)
    await page.getByRole('tab', { name: 'Provider 与凭证', exact: true }).click()
    const provider = page.getByTestId('provider-card-deepseek')
    await provider.getByRole('button', { name: '编辑', exact: true }).last().click()
    await page.getByRole('dialog', { name: '编辑供应商', exact: true }).waitFor({ state: 'visible' })
    await capture('provider-form', theme, viewport)
    await page.keyboard.press('Escape')
    await page.goto(`${origin}/logs`)
    await page.getByRole('button', { name: /查看 .* 请求详情/ }).first().click()
    const drawer = page.getByRole('dialog', { name: '请求详情' })
    for (const [index, name] of ['概览', '协议 Trace', 'Tool Use'].entries()) {
      await drawer.getByRole('tab', { name, exact: true }).click()
      await capture(`logs-drawer-${index}`, theme, viewport)
    }
    await page.keyboard.press('Escape')
    if (width === 1440) {
      await page.goto(`${origin}/dashboard`)
      await page.getByRole('heading', { name: '仪表盘', exact: true }).waitFor()
      const main = page.locator('#main-content')
      const height = await main.evaluate(el => el.scrollHeight)
      for (let top = 650, index = 1; top < height; top += 650, index++) {
        await main.evaluate((el, offset) => { el.scrollTop = offset }, top)
        await capture(`dashboard-section-${index}`, theme, viewport)
      }
    }

  }
}

await writeFile(path.join(output, 'visual-audit.json'), JSON.stringify({ tool: 'Playwright + optional axe-core WCAG 2/2.1 A/AA', mockOnly: true, results, errors }, null, 2) + '\n')
await browser.close()
if (errors.length || results.some(result => result.violations.length)) {
  console.error(`Failed: ${errors.length} layout/runtime errors; ${results.filter(result => result.violations.length).length} states with accessibility violations`)
  process.exitCode = 1
}
