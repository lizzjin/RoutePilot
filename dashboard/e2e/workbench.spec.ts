import { expect, test } from '@playwright/test'
import { csrfHeaders, login, seedFailedGatewayRequest } from './helpers'

test('inline Provider editing protects navigation, browser back, and logout without writing', async ({ page }) => {
  await login(page)
  await page.getByRole('navigation', { name: '工作区', exact: true }).getByRole('link', { name: '配置', exact: true }).click()
  await page.getByRole('button', { name: '编辑 Provider', exact: true }).click()
  const editor = page.getByRole('region', { name: '编辑供应商', exact: true })
  await editor.getByLabel('显示名称', { exact: true }).fill('unsaved-workbench')
  await page.getByRole('navigation', { name: '工作区', exact: true }).getByRole('link', { name: '运行', exact: true }).click()
  await expect(page.getByRole('dialog', { name: '有未保存的修改，确认离开？' })).toBeVisible()
  await page.getByRole('button', { name: '留在此页', exact: true }).click()
  await expect(editor.getByLabel('显示名称', { exact: true })).toHaveValue('unsaved-workbench')
  await page.goBack()
  await expect(page.getByRole('dialog', { name: '有未保存的修改，确认离开？' })).toBeVisible()
  await page.getByRole('button', { name: '留在此页', exact: true }).click()
  await page.getByRole('button', { name: '打开账户菜单' }).click()
  await page.getByRole('menuitem', { name: '退出登录' }).click()
  await expect(page.getByRole('dialog', { name: '有未保存的修改，确认离开？' })).toBeVisible()
  await page.getByRole('button', { name: '留在此页', exact: true }).click()
  await expect(editor.getByLabel('显示名称', { exact: true })).toHaveValue('unsaved-workbench')
  await editor.getByRole('button', { name: '取消', exact: true }).click()
  await page.getByRole('navigation', { name: '工作区', exact: true }).getByRole('link', { name: '运行', exact: true }).click()
  await expect(page).toHaveURL(/dashboard$/)
})

test('one-time key handoff blocks browser back and retains the reveal until saved', async ({ page }) => {
  await login(page)
  const name = `e2e_handoff_${Date.now()}`
  await page.getByRole('navigation', { name: '工作区', exact: true }).getByRole('link', { name: '访问与治理' }).click()
  await page.getByRole('button', { name: '创建密钥', exact: true }).click()
  const dialog = page.getByRole('dialog', { name: '创建 API 密钥' })
  await dialog.locator('#create-key-user').click()
  await page.getByRole('option', { name: /admin/ }).first().click()
  await dialog.locator('#create-key-name').fill(name)
  await dialog.getByRole('button', { name: '创建密钥', exact: true }).click()
  await expect(dialog.locator('#new-api-key')).toHaveValue(/^sk-mp-/)
  await page.goBack()
  await expect(page.getByRole('dialog', { name: 'API 密钥交接尚未完成，仍要离开？' })).toBeVisible()
  await page.getByRole('button', { name: '留在此页', exact: true }).click()
  await expect(dialog.locator('#new-api-key')).toHaveValue(/^sk-mp-/)
  await dialog.getByRole('button', { name: '已保存，关闭' }).click()
  const keys = await (await page.request.get('/admin/api-keys')).json() as {id: string;name: string}[]
  const key = keys.find(item => item.name === name)
  if (key) expect((await page.request.delete(`/admin/api-keys/${key.id}`, { headers: csrfHeaders() })).ok()).toBeTruthy()
})

test('quota object selection edits the actual selected rule and preserves independent units', async ({ page }) => {
  await login(page)
  const me = await (await page.request.get('/admin/auth/me')).json() as { id: string; username: string }
  const created = await page.request.post('/admin/quotas', { headers: csrfHeaders(), data: { userId: me.id, username: me.username, quotaType: 'requests', limit: 100, period: 'daily' } })
  expect(created.ok()).toBeTruthy()
  const quota = await created.json() as { id: string }
  try {
    await page.setViewportSize({ width: 390, height: 844 })
    await page.goto('/quotas')
    await page.locator('.workbench-directory').getByRole('button').filter({ hasText: 'requests' }).first().click()
    await page.locator('.workbench-object-facts').getByRole('button', { name: /调整|编辑/ }).click()
    const editor = page.getByRole('region', { name: '调整配额限额', exact: true })
    await editor.getByLabel('限额', { exact: false }).fill('150')
    await editor.getByRole('button', { name: /保存/ }).click()
    await expect(editor).toBeHidden()
    const quotas = await (await page.request.get('/admin/quotas')).json() as { id:string; limit:number }[]
    expect(quotas.find(item => item.id === quota.id)?.limit).toBe(150)
  } finally { await page.request.delete(`/admin/quotas/${quota.id}`, { headers: csrfHeaders() }) }
})

test('selected request survives reload and browser history without changing filters', async ({ page }) => {
  await login(page)
  await seedFailedGatewayRequest(page)
  await seedFailedGatewayRequest(page)
  await page.goto('/logs?pageSize=50')
  const rows = page.locator('[data-log-id]')
  await expect(rows.first()).toBeVisible()
  const firstId = await rows.first().getAttribute('data-log-id')
  const secondId = await rows.nth(1).getAttribute('data-log-id')
  await rows.first().click()
  await expect(page).toHaveURL(new RegExp(`request=${firstId}`))
  await rows.nth(1).click()
  await expect(page).toHaveURL(new RegExp(`request=${secondId}`))
  await page.goBack()
  await expect(page).toHaveURL(new RegExp(`request=${firstId}`))
  await page.reload()
  await expect(page.getByRole('region', { name: '请求详情' })).toBeVisible()
  await expect(page.locator(`[data-log-id="${firstId}"]`)).toHaveAttribute('aria-pressed', 'true')
  expect(new URL(page.url()).searchParams.get('pageSize')).toBe('50')
})


test('user-associated key reveal keeps a stable leave guard and survives history navigation', async ({ page }) => {
  await login(page)
  await page.getByRole('navigation', { name: '工作区', exact: true }).getByRole('link', { name: '访问与治理' }).click()
  await page.getByRole('navigation', { name: '工作区页面', exact: true }).getByRole('link', { name: '用户管理' }).click()
  await page.locator('.workbench-object-facts').getByRole('button', { name: /密钥/ }).click()
  const dialog = page.getByRole('dialog', { name: /的 API 密钥/ })
  const name = `e2e_associated_${Date.now()}`
  await dialog.getByLabel('新密钥名称').fill(name)
  await dialog.getByRole('button', { name: '生成密钥', exact: true }).click()
  await expect(dialog.locator('#new-user-key')).toHaveValue(/^sk-mp-/)
  await page.goBack()
  await expect(page.getByRole('dialog', { name: 'API 密钥尚未确认保存，仍要离开？', exact: true })).toBeVisible()
  await page.getByRole('button', { name: '留在此页', exact: true }).click()
  await expect(dialog.locator('#new-user-key')).toHaveValue(/^sk-mp-/)
  await dialog.getByRole('button', { name: '已保存，返回列表' }).click()
  const keys = await (await page.request.get('/admin/api-keys')).json() as { id: string; name: string }[]
  const key = keys.find(item => item.name === name)
  expect(key).toBeTruthy()
  await page.request.delete(`/admin/api-keys/${key!.id}`, { headers: csrfHeaders() })
})
