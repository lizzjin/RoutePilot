import assert from 'node:assert/strict'
import { mkdtemp, rm, writeFile } from 'node:fs/promises'
import { tmpdir } from 'node:os'
import { fileURLToPath } from 'node:url'
import { chromium } from 'playwright'

// A disposable extension invokes Chromium's native tab zoom. Unlike CSS zoom
// or deviceScaleFactor alone, this exercises the browser's layout viewport.
const origin = process.env.REDESIGN_MOCK_URL || 'http://127.0.0.1:33012'
assert(['localhost', '127.0.0.1'].includes(new URL(origin).hostname))
const extension = await mkdtemp(`${tmpdir()}/routepilot-zoom-`)
const output = fileURLToPath(new URL('../../docs/assets/routepilot-redesign/', import.meta.url))
await writeFile(`${extension}/manifest.json`, JSON.stringify({ manifest_version: 3, name: 'Local zoom acceptance', version: '1.0', permissions: ['tabs'], background: { service_worker: 'worker.js' } }))
await writeFile(`${extension}/worker.js`, 'chrome.runtime.onInstalled.addListener(() => {});')
let context
try {
  context = await chromium.launchPersistentContext('', {
    channel: 'chromium', headless: true, reducedMotion: 'reduce', viewport: { width: 1440, height: 900 },
    args: [`--disable-extensions-except=${extension}`, `--load-extension=${extension}`],
  })
  const worker = context.serviceWorkers()[0] || await context.waitForEvent('serviceworker')
  const page = await context.newPage()
  await page.goto(`${origin}/login`)
  await page.locator('#username').fill('admin')
  await page.locator('#password').fill('admin')
  await page.getByRole('button', { name: '登录', exact: true }).click()
  await page.waitForURL('**/dashboard')
  await page.getByText('演示数据', { exact: true }).waitFor()
  const zoom = await worker.evaluate(async url => {
    const [tab] = await chrome.tabs.query({ url: `${url}/*` })
    await chrome.tabs.setZoom(tab.id, 2)
    return chrome.tabs.getZoom(tab.id)
  }, origin)
  assert.equal(zoom, 2)
  const results = []
  for (const theme of ['light', 'dark']) {
    await page.evaluate(value => localStorage.setItem('routepilot_theme', value), theme)
    for (const route of ['dashboard', 'models', 'logs', 'api-keys', 'users', 'quotas', 'enterprise', 'governance', 'operations', 'settings', 'guide', 'login', 'missing-page']) {
      await page.goto(`${origin}/${route}`)
      await page.locator('h1').first().waitFor()
      await page.evaluate(() => document.fonts.ready)
      const geometry = await page.evaluate(() => {
        const main = document.querySelector('#main-content') || document.documentElement
        return { viewport: innerWidth, document: document.documentElement.scrollWidth, main: main.clientWidth, content: main.scrollWidth }
      })
      assert.equal(geometry.viewport, 720)
      assert(geometry.document <= geometry.viewport + 1, `${route}: document overflow`)
      assert(geometry.content <= geometry.main + 1, `${route}: content overflow`)
      results.push({ route, theme, zoom, geometry })
      if (['dashboard', 'login'].includes(route)) await page.screenshot({ path: `${output}/after-${route}-${theme}-native-zoom-200.png` })
    }
    await page.goto(`${origin}/models`)
    await page.getByRole('tab', { name: 'Provider 与凭证', exact: true }).click()
    await page.getByTestId('provider-card-deepseek').getByRole('button', { name: '编辑', exact: true }).last().click()
    const dialog = page.getByRole('dialog', { name: '编辑供应商', exact: true })
    await dialog.waitFor()
    const motion = await dialog.evaluate(el => ({ reduced: matchMedia('(prefers-reduced-motion: reduce)').matches, animation: getComputedStyle(el).animationDuration, transition: getComputedStyle(el).transitionDuration }))
    assert(motion.reduced)
    assert(motion.animation.split(',').every(value => parseFloat(value) <= 0.001))
    assert(motion.transition.split(',').every(value => parseFloat(value) <= 0.001))
    const save = await dialog.getByRole('button', { name: '保存 Provider', exact: true }).boundingBox()
    const height = await page.evaluate(() => innerHeight)
    assert(save.y >= 0 && save.y + save.height <= height, 'Provider save action must remain in the zoomed viewport')
    results.push({ route: 'provider-form', theme, zoom, motion, saveActionVisible: true })
    await page.screenshot({ path: `${output}/after-provider-form-${theme}-native-zoom-200.png` })
    await page.keyboard.press('Escape')
  }
  await writeFile(`${output}/zoom-motion-audit.json`, JSON.stringify({ mockOnly: true, nativeBrowserZoom: true, results }, null, 2) + '\n')
  console.log(`${results.length} native 200% zoom and reduced-motion states passed`)
} finally {
  await context?.close()
  await rm(extension, { recursive: true, force: true })
}
