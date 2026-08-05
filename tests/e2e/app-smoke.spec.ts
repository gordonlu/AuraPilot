import { expect, test } from '@playwright/test'

test('starts the application shell and opens the world skin picker', async ({ page }) => {
  const pageErrors: string[] = []
  page.on('pageerror', (error) => pageErrors.push(error.message))

  await page.goto('/')

  await expect(page.locator('.app-shell')).toBeVisible()
  await expect(page.getByRole('button', { name: '添加项目' })).toBeVisible()

  const worldSkinButton = page.locator('.sidebar-footer .footer-control').first()
  await expect(worldSkinButton).toBeVisible()
  await worldSkinButton.click()

  await expect(page.getByRole('dialog', { name: '选择世界皮肤' })).toBeVisible()
  await page.getByRole('button', { name: '关闭世界皮肤选择器' }).click()
  await expect(page.getByRole('dialog', { name: '选择世界皮肤' })).toBeHidden()

  expect(pageErrors).toEqual([])
})
