import { expect, test } from '@playwright/test';

test('local auth first launch guides the user through setup, login, and onboarding handoff', async ({ page }) => {
  let setupComplete = false;
  let loggedIn = false;
  let createdWebsiteId: string | null = null;

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
      return json(200, {
        data: createdWebsiteId
          ? [
              {
                id: createdWebsiteId,
                name: 'Example Site',
                domain: 'example.com',
                timezone: 'UTC',
                created_at: '2026-03-08T00:00:00Z',
                share_id: null,
              },
            ]
          : [],
      });
    }

    if (path === '/api/websites' && method === 'POST') {
      createdWebsiteId = 'example-site';
      return json(201, {
        data: {
          id: createdWebsiteId,
          name: 'Example Site',
          domain: 'example.com',
          timezone: 'UTC',
          created_at: '2026-03-08T00:00:00Z',
          share_id: null,
        },
      });
    }

    if (path === `/api/websites/${createdWebsiteId}/stats`) {
      return json(200, {
        data: {
          pageviews: 0,
          visitors: 0,
          sessions: 0,
          bounce_rate: 0,
          avg_duration_seconds: 0,
          prev_pageviews: 0,
          prev_visitors: 0,
          prev_sessions: 0,
          prev_bounce_rate: 0,
          prev_avg_duration_seconds: 0,
          timezone: 'UTC',
        },
      });
    }

    return json(404, {
      error: { code: 'not_found', message: 'Not found', field: null },
    });
  });

  await page.goto('/setup');

  await expect(page.getByRole('heading', { name: 'Set up your instance' })).toBeVisible();
  await expect(page.getByText(/install the tracking snippet/i)).toBeVisible();

  await page.getByLabel(/^Password$/).fill('correct horse battery staple');
  await page.getByLabel(/^Confirm password$/).fill('correct horse battery staple');
  await page.getByRole('button', { name: /create admin account/i }).click();

  await expect(page.getByRole('heading', { name: 'Sign in' })).toBeVisible();
  await page.getByLabel(/^Password$/).fill('correct horse battery staple');
  await page.getByRole('button', { name: 'Sign in' }).click();

  await expect(page.getByRole('heading', { name: /welcome to sparklytics/i })).toBeVisible({
    timeout: 10000,
  });
  await expect(page.getByText(/add your first website/i)).toBeVisible();
  await expect(page.getByText(/start with the domain you want to verify today/i)).toBeVisible();

  await page.getByLabel('Website name').fill('Example Site');
  await page.getByLabel('Domain').fill('example.com');
  await page.getByRole('button', { name: 'Create website' }).click();

  await expect(page.getByRole('heading', { name: /install the tracking snippet/i })).toBeVisible();
  await expect(page.getByText(/paste it inside the/i)).toBeVisible();

  await page.getByRole('button', { name: /done, verify installation/i }).click();
  await expect(page.getByRole('heading', { name: /verify installation/i })).toBeVisible();
  await page.getByRole('button', { name: /check for pageviews/i }).click();
  await expect(page.getByText(/no events received yet/i)).toBeVisible();
});
