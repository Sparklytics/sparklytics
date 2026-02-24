import { expect, test } from '@playwright/test';

test('runtime auth mode none suppresses auth status requests', async ({ page }) => {
  let authStatusCalls = 0;

  await page.addInitScript(() => {
    (window as Window & { __SPARKLYTICS_AUTH_MODE__?: string }).__SPARKLYTICS_AUTH_MODE__ =
      'none';
  });

  await page.route('**/api/**', async (route) => {
    const url = new URL(route.request().url());
    const path = url.pathname;

    const json = (status: number, payload: unknown) =>
      route.fulfill({
        status,
        contentType: 'application/json',
        body: JSON.stringify(payload),
      });

    if (path === '/api/auth/status') {
      authStatusCalls += 1;
      return json(404, {
        error: { code: 'not_found', message: 'Not found', field: null },
      });
    }

    if (path === '/api/websites') {
      return json(200, {
        data: [
          {
            id: 'site_test',
            name: 'Test Site',
            domain: 'example.com',
            timezone: 'UTC',
            created_at: '2026-02-20T00:00:00Z',
            share_id: null,
          },
        ],
      });
    }

    if (path === '/api/websites/site_test/stats') {
      return json(200, {
        data: {
          pageviews: 12,
          visitors: 9,
          sessions: 10,
          bounce_rate: 0.4,
          avg_duration_seconds: 120,
          prev_pageviews: 10,
          prev_visitors: 8,
          prev_sessions: 8,
          prev_bounce_rate: 0.5,
          prev_avg_duration_seconds: 100,
          timezone: 'UTC',
        },
      });
    }

    if (path === '/api/websites/site_test/pageviews') {
      return json(200, {
        data: {
          series: [
            { date: '2026-02-19', pageviews: 5, visitors: 4 },
            { date: '2026-02-20', pageviews: 7, visitors: 5 },
          ],
          granularity: 'day',
        },
      });
    }

    if (path === '/api/websites/site_test/metrics') {
      return json(200, {
        data: {
          type: url.searchParams.get('type') ?? 'page',
          rows: [],
        },
        pagination: { total: 0, limit: 10, offset: 0, has_more: false },
      });
    }

    if (path === '/api/websites/site_test/realtime') {
      return json(200, {
        data: {
          active_visitors: 0,
          recent_events: [],
          pagination: { limit: 100, total_in_window: 0 },
        },
      });
    }

    return json(404, {
      error: { code: 'not_found', message: 'Not found', field: null },
    });
  });

  await page.goto('/dashboard');
  await expect(page.getByRole('heading', { name: /Test Site/i })).toBeVisible();
  await page.waitForTimeout(300);

  expect(authStatusCalls).toBe(0);
});
