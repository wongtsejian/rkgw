import { test, expect } from '@playwright/test'
import { Nav } from '../../helpers/selectors.js'
import { navigateTo } from '../../helpers/navigation.js'

test.describe('Navigation and layout', () => {
  test('layout renders sidebar, top-bar, and main content', async ({ page }) => {
    await navigateTo(page, '/')

    await expect(page.locator('nav.sidebar')).toBeAttached()
    await expect(page.locator('header.top-bar')).toBeVisible()
    await expect(page.locator('main#main-content')).toBeVisible()
  })

  test('sidebar nav links navigate and show active state', async ({ page }) => {
    await navigateTo(page, '/')

    // Dashboard link should be active on index
    const dashboardLink = page.locator(Nav.link, { hasText: 'dashboard' })
    await expect(dashboardLink).toHaveClass(/active/)

    // Click profile link
    const profileLink = page.locator(Nav.link, { hasText: 'profile' })
    await profileLink.click()
    await page.waitForLoadState('networkidle')
    await expect(profileLink).toHaveClass(/active/)
    expect(page.url()).toContain('/profile')
  })

  test('admin links are visible for admin user', async ({ page }) => {
    await navigateTo(page, '/')

    await expect(page.locator(Nav.link, { hasText: 'config' })).toBeVisible()
    await expect(page.locator(Nav.link, { hasText: 'guardrails' })).toBeVisible()
    await expect(page.locator(Nav.link, { hasText: 'admin' })).toBeVisible()
  })

  test('page title updates per route', async ({ page }) => {
    await navigateTo(page, '/')
    await expect(page.locator('span.page-title')).toContainText('dashboard')

    await navigateTo(page, '/profile')
    await expect(page.locator('span.page-title')).toContainText('profile')

    await navigateTo(page, '/config')
    await expect(page.locator('span.page-title')).toContainText('configuration')

    await navigateTo(page, '/admin')
    await expect(page.locator('span.page-title')).toContainText('administration')

    await navigateTo(page, '/guardrails')
    await expect(page.locator('span.page-title')).toContainText('guardrails')
  })

  test('logout button is visible and clickable', async ({ page }) => {
    await navigateTo(page, '/')

    const logoutBtn = page.locator(Nav.logout)
    await expect(logoutBtn).toBeVisible()
    await expect(logoutBtn).toBeEnabled()
  })
})
