import { expect, test } from '@playwright/test';

test('compare mode picker syncs URL and sends compare params', async ({ page }) => {
  const seenModes: string[] = [];

  await page.route('**/api/**', async (route) => {
    const url = new URL(route.request().url());
    const path = url.pathname;
    const query = url.searchParams;

    const json = (status: number, payload: unknown) =>
      route.fulfill({
        status,
        contentType: 'application/json',
        body: JSON.stringify(payload),
      });

    if (path === '/api/auth/status') {
      return json(404, { error: { code: 'not_found', message: 'Not found', field: null } });
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
    if (path.startsWith('/api/websites/site_test/')) {
      const mode = query.get('compare_mode');
      if (mode) seenModes.push(mode);
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
          prev_sessions: 9,
          prev_bounce_rate: 0.45,
          prev_avg_duration_seconds: 110,
          timezone: 'UTC',
        },
      });
    }
    if (path === '/api/websites/site_test/pageviews') {
      return json(200, {
        data: {
          series: [
            { date: '2026-02-19', pageviews: 5, visitors: 3 },
            { date: '2026-02-20', pageviews: 6, visitors: 4 },
          ],
          compare_series: [
            { date: '2026-02-12', pageviews: 4, visitors: 2 },
            { date: '2026-02-13', pageviews: 5, visitors: 3 },
          ],
          granularity: 'day',
        },
      });
    }
    if (path === '/api/websites/site_test/metrics') {
      return json(200, {
        data: {
          type: query.get('type') ?? 'page',
          rows: [
            {
              value: '/pricing',
              visitors: 5,
              pageviews: 6,
              prev_visitors: 4,
              prev_pageviews: 4,
              delta_visitors_abs: 1,
              delta_visitors_pct: 0.25,
              bounce_rate: 0,
              avg_duration_seconds: 100,
            },
          ],
        },
        pagination: { total: 1, limit: 10, offset: 0, has_more: false },
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

    return json(404, { error: { code: 'not_found', message: 'Not found', field: null } });
  });

  await page.goto('/dashboard');
  await expect(page.getByRole('heading', { name: /Test Site/i })).toBeVisible();

  await page.locator('select').filter({ hasText: 'No compare' }).first().selectOption('previous_period');
  await page.waitForTimeout(300);
  await expect(page).toHaveURL(/compare_mode=previous_period/);
  expect(seenModes.includes('previous_period')).toBeTruthy();
  await expect(page.getByText('Curr / Prev / Δ').first()).toBeVisible();
  await expect(page.getByText('+25.0%').first()).toBeVisible();
  await expect(page.locator('path[stroke-dasharray=\"4 4\"]').first()).toBeVisible();

  await page.locator('select').filter({ hasText: 'Previous period' }).first().selectOption('custom');
  await page.getByLabel('Compare start').fill('2026-01-01');
  await page.getByLabel('Compare end').fill('2026-01-31');
  await page.waitForTimeout(300);
  await expect(page).toHaveURL(/compare_mode=custom/);
  await expect(page).toHaveURL(/compare_start=2026-01-01/);
  await expect(page).toHaveURL(/compare_end=2026-01-31/);
});
