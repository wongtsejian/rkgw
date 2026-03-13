import { test, expect } from '@playwright/test'
import { Login } from '../../helpers/selectors.js'

test.describe('Password login page', () => {
  test.fixme('renders password login form with email and password fields', async ({ page }) => {
    // Navigate to ./login
    // Expect email input, password input, and submit button visible
  })

  test.fixme('shows both Google SSO and password login options', async ({ page }) => {
    // Navigate to ./login
    // Expect Google sign-in button AND password form both visible
    // (when both auth methods are enabled)
  })

  test.fixme('successful password login redirects to dashboard', async ({ page }) => {
    // Fill email and password fields
    // Click submit
    // Expect redirect to /_ui/ dashboard
  })

  test.fixme('login with 2FA prompts for TOTP code', async ({ page }) => {
    // Fill email and password for 2FA-enabled user
    // Click submit
    // Expect TOTP code input to appear
    // Fill valid TOTP code
    // Expect redirect to dashboard
  })

  test.fixme('shows error message for invalid credentials', async ({ page }) => {
    // Fill email and wrong password
    // Click submit
    // Expect error message visible (e.g. "Invalid email or password")
  })

  test.fixme('shows error message for invalid TOTP code', async ({ page }) => {
    // Complete password step for 2FA user
    // Enter invalid TOTP code
    // Expect error message about invalid code
  })

  test.fixme('shows rate limit message after too many attempts', async ({ page }) => {
    // Submit wrong password multiple times
    // Expect rate limit error message or disabled form
  })

  test.fixme('password field masks input', async ({ page }) => {
    // Navigate to ./login
    // Expect password input type="password"
  })

  test.fixme('form validates required fields before submit', async ({ page }) => {
    // Click submit with empty fields
    // Expect validation messages (required email, required password)
  })
})
