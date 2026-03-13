import { test, expect } from '@playwright/test'
import { Form, Toast } from '../../helpers/selectors.js'
import { navigateTo, expectToastMessage } from '../../helpers/navigation.js'

test.describe('Password change flow', () => {
  test.fixme('forced password change redirect on first login', async ({ page }) => {
    // Login as user with force_password_change flag
    // Expect redirect to password change page instead of dashboard
    // Navigation should be restricted until password is changed
  })

  test.fixme('password change form renders with required fields', async ({ page }) => {
    // Navigate to password change page
    // Expect current password, new password, and confirm password fields
    // Expect submit button
  })

  test.fixme('validates new password meets strength requirements', async ({ page }) => {
    // Fill current password
    // Fill weak new password (e.g. '123')
    // Expect validation error about password strength
  })

  test.fixme('validates new password and confirm password match', async ({ page }) => {
    // Fill new password and different confirm password
    // Expect validation error about passwords not matching
  })

  test.fixme('rejects change when current password is wrong', async ({ page }) => {
    // Fill wrong current password
    // Fill valid new password + confirm
    // Click submit
    // Expect error message about incorrect current password
  })

  test.fixme('successful password change shows confirmation', async ({ page }) => {
    // Fill correct current password
    // Fill valid new password + confirm
    // Click submit
    // Expect success toast or redirect to profile/dashboard
  })

  test.fixme('successful forced change redirects to dashboard', async ({ page }) => {
    // Complete forced password change
    // Expect redirect to /_ui/ dashboard
    // Subsequent navigation should work normally
  })
})
