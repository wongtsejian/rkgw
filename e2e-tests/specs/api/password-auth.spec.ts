import { test, expect } from '@playwright/test';

test.describe('Password authentication API', () => {
  test.fixme('login with valid credentials returns session cookie', async ({ request }) => {
    // POST /_ui/api/auth/login with { email, password }
    // Expect 200 with Set-Cookie header containing kgw_session
  });

  test.fixme('login with wrong password returns 401', async ({ request }) => {
    // POST /_ui/api/auth/login with { email, password: 'wrong' }
    // Expect 401 with error message
  });

  test.fixme('login with non-existent user returns 401', async ({ request }) => {
    // POST /_ui/api/auth/login with unknown email
    // Expect 401 (should not reveal whether user exists)
  });

  test.fixme('rate limiting after repeated failed attempts', async ({ request }) => {
    // POST /_ui/api/auth/login with wrong password N times
    // Expect 429 after threshold is reached
  });

  test.fixme('login with valid credentials + 2FA returns 2FA challenge', async ({ request }) => {
    // POST /_ui/api/auth/login for user with TOTP enabled
    // Expect 200 with { requires_2fa: true, challenge_token: '...' }
  });

  test.fixme('complete 2FA with valid TOTP code', async ({ request }) => {
    // POST /_ui/api/auth/2fa/verify with { challenge_token, code }
    // Expect 200 with session cookie
  });

  test.fixme('complete 2FA with invalid TOTP code returns 401', async ({ request }) => {
    // POST /_ui/api/auth/2fa/verify with { challenge_token, code: '000000' }
    // Expect 401
  });

  test.fixme('use recovery code when TOTP unavailable', async ({ request }) => {
    // POST /_ui/api/auth/2fa/verify with { challenge_token, recovery_code }
    // Expect 200 with session cookie
    // Recovery code should be consumed (single-use)
  });

  test.fixme('used recovery code cannot be reused', async ({ request }) => {
    // POST /_ui/api/auth/2fa/verify with already-used recovery code
    // Expect 401
  });
});

test.describe('Password change API', () => {
  test.fixme('change password with valid current password', async ({ request }) => {
    // PUT /_ui/api/auth/password with { current_password, new_password }
    // Expect 200
  });

  test.fixme('change password with wrong current password returns 401', async ({ request }) => {
    // PUT /_ui/api/auth/password with { current_password: 'wrong', new_password }
    // Expect 401
  });

  test.fixme('change password rejects weak new password', async ({ request }) => {
    // PUT /_ui/api/auth/password with { current_password, new_password: '123' }
    // Expect 400 with validation error
  });
});

test.describe('Admin user management API', () => {
  test.fixme('admin creates user with password', async ({ request }) => {
    // POST /_ui/api/admin/users with { email, password, role }
    // Expect 201 with user object
  });

  test.fixme('admin creates user with force_password_change flag', async ({ request }) => {
    // POST /_ui/api/admin/users with { email, password, role, force_password_change: true }
    // Expect 201, user must change password on first login
  });

  test.fixme('admin resets user password', async ({ request }) => {
    // POST /_ui/api/admin/users/:id/reset-password
    // Expect 200 with temporary password
  });

  test.fixme('non-admin cannot create users', async ({ request }) => {
    // POST /_ui/api/admin/users as regular user
    // Expect 403
  });

  test.fixme('non-admin cannot reset passwords', async ({ request }) => {
    // POST /_ui/api/admin/users/:id/reset-password as regular user
    // Expect 403
  });
});
