import { expect, test } from '@playwright/test'
import { login, requireE2EEnv } from './helpers'

test.describe('enterprise operations', () => {
  test.beforeEach(async ({ page }) => {
    await login(page, requireE2EEnv())
  })

  test('loads ledger evidence and opens budget details without writing state', async ({ page }) => {
    const enterpriseWrites: string[] = []
    page.on('request', (request) => {
      const url = new URL(request.url())
      if (url.pathname.startsWith('/admin/enterprise/') && request.method() !== 'GET') {
        enterpriseWrites.push(`${request.method()} ${url.pathname}`)
      }
    })

    const overviewResponse = page.waitForResponse((response) => (
      new URL(response.url()).pathname === '/admin/enterprise/overview' && response.ok()
    ))
    const budgetResponse = page.waitForResponse((response) => (
      new URL(response.url()).pathname === '/admin/enterprise/budget' && response.ok()
    ))
    const requestsResponse = page.waitForResponse((response) => (
      new URL(response.url()).pathname === '/admin/enterprise/requests' && response.ok()
    ))

    await page.goto('/enterprise')
    await Promise.all([overviewResponse, budgetResponse, requestsResponse])

    await expect(page.getByRole('heading', { name: '运行账本', exact: true })).toBeVisible()
    await expect(page.getByRole('region', { name: '运行账本概览' })).toBeVisible()
    await expect(page.getByRole('region', { name: '预算控制' })).toContainText('事务预算控制')
    await expect(page.getByRole('region', { name: '受治理请求记录' })).toContainText('Gateway Requests')
    await expect(page.getByLabel('搜索运行账本')).toBeVisible()

    await page.getByRole('button', { name: '管理' }).click()
    const budgetDialog = page.getByRole('dialog', { name: '事务预算与证据' })
    await expect(budgetDialog).toBeVisible()
    await expect(budgetDialog.getByText('推理硬上限')).toBeVisible()
    await expect(budgetDialog.getByText('人工账务调整')).toBeVisible()
    await expect(budgetDialog.getByText('最近证据事件')).toBeVisible()

    await page.keyboard.press('Escape')
    await expect(budgetDialog).toBeHidden()
    expect(enterpriseWrites).toEqual([])
  })

  test('labels the free small-team approval workflow as optional', async ({ page }) => {
    await page.goto('/governance')

    await expect(page.getByRole('heading', { name: '治理与变更审批' })).toBeVisible()
    await expect(page.getByText('可选双人复核', { exact: true })).toBeVisible()
    await expect(page.getByText(/免费小团队模式允许管理员直接执行并保留审计/)).toBeVisible()

    const suffix = Date.now().toString(36)
    const target = `org_e2e/prj_${suffix}/env_test`
    await page.getByLabel('目标标识').fill(target)
    await page.getByLabel('业务原因与回滚依据').fill('verify one-administrator small-team policy application')
    await page.getByLabel('精确变更载荷（JSON）').fill(JSON.stringify({
      organizationId: 'org_e2e',
      projectId: `prj_${suffix}`,
      environmentId: 'env_test',
      maximumMode: 'local_strict',
      defaultClassification: 'unknown',
      allowedProviders: [],
      allowedModels: [],
      allowedRegions: ['local'],
      allowedApiVersions: ['openai-compatible-v1'],
      cloudEnabled: false,
    }, null, 2))
    const createdResponsePromise = page.waitForResponse((response) => (
      new URL(response.url()).pathname === '/admin/governance/change-requests'
      && response.request().method() === 'POST'
    ))
    await page.getByRole('button', { name: '记录变更意图' }).click()
    const createdResponse = await createdResponsePromise
    expect(createdResponse.ok()).toBeTruthy()
    const change = await createdResponse.json() as { id: string }

    const row = page.getByTestId(`governance-change-${change.id}`)
    await expect(row).toContainText(target)
    const appliedResponsePromise = page.waitForResponse((response) => (
      new URL(response.url()).pathname === `/admin/governance/change-requests/${change.id}/apply`
      && response.request().method() === 'POST'
    ))
    await row.getByRole('button', { name: '直接应用' }).click()
    const appliedResponse = await appliedResponsePromise
    expect(appliedResponse.ok()).toBeTruthy()
    await expect(row.getByText('已应用', { exact: true })).toBeVisible()
  })
})
