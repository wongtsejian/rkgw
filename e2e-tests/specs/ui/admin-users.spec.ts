import { test, expect } from '@playwright/test'
import { Card, Table, Toast } from '../../helpers/selectors.js'
import { navigateTo, expectToastMessage } from '../../helpers/navigation.js'

test.describe('Admin user management', () => {
  test.fixme('admin creates user with password auth', async ({ page }) => {
    // Navigate to /admin
    // Click "Create User" button
    // Fill email, password, select role
    // Click submit
    // Expect new user appears in user list
  })

  test.fixme('admin creates user with force password change', async ({ page }) => {
    // Navigate to /admin
    // Click "Create User" button
    // Fill email, password, select role
    // Check "Force password change on first login" checkbox
    // Click submit
    // Expect success toast
  })

  test.fixme('admin resets user password', async ({ page }) => {
    // Navigate to /admin
    // Find target user in user list
    // Click reset password action
    // Expect confirmation dialog
    // Confirm reset
    // Expect temporary password displayed or success toast
  })

  test.fixme('user list shows auth method column', async ({ page }) => {
    // Navigate to /admin
    // Expect user table has "Auth Method" column
    // Expect values like "Google SSO", "Password", or "Password + 2FA"
  })

  test.fixme('user list shows 2FA status', async ({ page }) => {
    // Navigate to /admin
    // Expect 2FA status indicator per user (enabled/disabled)
  })

  test.fixme('admin toggles password auth in config', async ({ page }) => {
    // Navigate to /config or /admin settings
    // Find password auth toggle
    // Toggle on/off
    // Save config
    // Expect success confirmation
  })

  test.fixme('admin toggles 2FA requirement in config', async ({ page }) => {
    // Navigate to /config or /admin settings
    // Find 2FA requirement toggle
    // Toggle on/off
    // Save config
    // Expect success confirmation
  })

  test.fixme('create user form validates required fields', async ({ page }) => {
    // Click "Create User" without filling fields
    // Expect validation errors for email and password
  })

  test.fixme('create user form rejects duplicate email', async ({ page }) => {
    // Try to create user with existing email
    // Expect error about duplicate email
  })
})
