import { expect, test } from '@playwright/test';

test('local auth first launch guides the user through setup, login, and onboarding handoff', async ({ page }) => {
  let setupComplete = false;
  let loggedIn = false;

  await page.route('**/api/**', async (route) => {
    const url = new URL(route.request().url());
    const path = url.pathname;
    const method = route.request().method();

    const json = (status: number, payload: unknown) =>
      route.fulfill({
        status,
        contentType: 'application/json',
        body: JSON.stringify(payload),
      });

    if (path === '/api/auth/status') {
      return json(200, {
        mode: 'local',
        setup_required: !setupComplete,
        authenticated: loggedIn,
        password_change_required: false,
      });
    }

    if (path === '/api/auth/setup' && method === 'POST') {
      setupComplete = true;
      return json(201, { data: { ok: true } });
    }

    if (path === '/api/auth/login' && method === 'POST') {
      loggedIn = true;
      return json(200, { data: { ok: true } });
    }

    if (path === '/api/websites' && method === 'GET') {
      return json(200, { data: [] });
    }

    return json(404, {
      error: { code: 'not_found', message: 'Not found', field: null },
    });
  });

  await page.goto('/setup');

  await expect(page.getByRole('heading', { name: 'Set up your instance' })).toBeVisible();
  await expect(page.getByText(/enter the bootstrap password from install time/i)).toBeVisible();

  await page.getByLabel(/Bootstrap password/i).fill('install-secret');
  await page.getByLabel(/^Password$/).fill('correct horse battery staple');
  await page.getByLabel(/^Confirm password$/).fill('correct horse battery staple');
  await page.getByRole('button', { name: /create admin account/i }).click();

  await expect(page.getByRole('heading', { name: 'Sign in' })).toBeVisible();
  await page.getByLabel(/^Password$/).fill('correct horse battery staple');
  await page.getByRole('button', { name: 'Sign in' }).click();

  await expect(page.getByRole('heading', { name: /welcome to sparklytics/i })).toBeVisible({
    timeout: 10000,
  });
  await expect(page.getByRole('heading', { name: /add your website/i })).toBeVisible();
  await expect(page.getByText(/start with the domain you want to verify today/i)).toBeVisible();
  await expect(page.getByLabel('Website name')).toBeVisible();
  await expect(page.getByLabel('Domain')).toBeVisible();
  await expect(page.getByRole('button', { name: 'Create website' })).toBeVisible();
});

test('forced password guards redirect /settings and /force-password correctly', async ({ page }) => {
  let status = {
    mode: 'local',
    setup_required: false,
    authenticated: true,
    password_change_required: true,
  };

  await page.route('**/api/**', async (route) => {
    const url = new URL(route.request().url());

    if (url.pathname === '/api/auth/status') {
      await route.fulfill({
        status: 200,
        contentType: 'application/json',
        body: JSON.stringify(status),
      });
      return;
    }

    await route.fulfill({
      status: 404,
      contentType: 'application/json',
      body: JSON.stringify({ error: { code: 'not_found', message: 'Not found', field: null } }),
    });
  });

  await page.goto('/settings');
  await page.waitForURL(/\/force-password\/?$/);
  await expect(page.getByRole('heading', { name: /change password before continuing/i })).toBeVisible();

  status = {
    mode: 'local',
    setup_required: false,
    authenticated: false,
    password_change_required: false,
  };

  await page.goto('/force-password');
  await page.waitForURL(/\/login\/?$/);
  await expect(page.getByRole('heading', { name: 'Sign in' })).toBeVisible();
});

test('forced password guard redirects /onboarding to /force-password', async ({ page }) => {
  await page.route('**/api/**', async (route) => {
    const url = new URL(route.request().url());

    if (url.pathname === '/api/auth/status') {
      await route.fulfill({
        status: 200,
        contentType: 'application/json',
        body: JSON.stringify({
          mode: 'local',
          setup_required: false,
          authenticated: true,
          password_change_required: true,
        }),
      });
      return;
    }

    if (url.pathname === '/api/websites') {
      await route.fulfill({
        status: 200,
        contentType: 'application/json',
        body: JSON.stringify({ data: [] }),
      });
      return;
    }

    await route.fulfill({
      status: 404,
      contentType: 'application/json',
      body: JSON.stringify({ error: { code: 'not_found', message: 'Not found', field: null } }),
    });
  });

  await page.goto('/onboarding');
  await page.waitForURL(/\/force-password\/?$/);
  await expect(page.getByRole('heading', { name: /change password before continuing/i })).toBeVisible();
});

test('security settings change password keeps errors local and redirects to login on success', async ({ page }) => {
  let passwordAttempts = 0;
  let authStatus = {
    mode: 'local',
    setup_required: false,
    authenticated: true,
    password_change_required: false,
  };

  await page.route('**/api/**', async (route) => {
    const url = new URL(route.request().url());
    const path = url.pathname;
    const method = route.request().method();

    const json = (status: number, payload: unknown) =>
      route.fulfill({
        status,
        contentType: 'application/json',
        body: JSON.stringify(payload),
      });

    if (path === '/api/auth/status') {
      return json(200, authStatus);
    }

    if (path === '/api/websites' && method === 'GET') {
      return json(200, {
        data: [{ id: 'example-site', name: 'Example Site', domain: 'example.com', timezone: 'UTC' }],
      });
    }

    if (path === '/api/websites/example-site' && method === 'GET') {
      return json(200, {
        data: {
          id: 'example-site',
          name: 'Example Site',
          domain: 'example.com',
          timezone: 'UTC',
          share_id: null,
        },
      });
    }

    if (path === '/api/websites/example-site/ingest-limits' && method === 'GET') {
      return json(200, {
        data: {
          website_id: 'example-site',
          peak_events_per_sec: null,
          queue_max_events: null,
          source: {
            peak_events_per_sec: 'default',
            queue_max_events: 'default',
          },
        },
      });
    }

    if (path === '/api/auth/password' && method === 'PUT') {
      passwordAttempts += 1;
      if (passwordAttempts === 1) {
        return json(400, {
          error: {
            code: 'bad_request',
            message: 'Current password is incorrect',
            field: 'current_password',
          },
        });
      }
      authStatus = {
        ...authStatus,
        authenticated: false,
        password_change_required: false,
      };
      return json(200, { data: { ok: true } });
    }

    return json(404, {
      error: { code: 'not_found', message: 'Not found', field: null },
    });
  });

  await page.goto('/dashboard');
  await expect(page.getByRole('button', { name: 'Security' })).toBeVisible();
  await page.evaluate(() => {
    window.history.pushState({}, '', '/dashboard/example-site/settings/security');
    window.dispatchEvent(new PopStateEvent('popstate'));
  });

  await expect(page.getByRole('heading', { name: 'Change password' })).toBeVisible();
  await page.getByLabel('Current password').fill('wrong-password');
  await page.getByLabel(/^New password$/).fill('correct horse battery staple');
  await page.getByLabel('Confirm new password').fill('correct horse battery staple');
  await page.getByRole('button', { name: 'Change password' }).click();

  await expect(page).toHaveURL(/\/dashboard\/example-site\/settings\/security\/?$/);
  await expect(page.getByText('Failed to change password', { exact: true }).first()).toBeVisible();
  await expect(page.getByText('Current password is incorrect', { exact: true }).first()).toBeVisible();

  await page.getByLabel('Current password').fill('current-password');
  await page.getByRole('button', { name: 'Change password' }).click();

  await page.waitForURL(/\/login\/?$/);
  await expect(page.getByRole('heading', { name: 'Sign in' })).toBeVisible();
});
