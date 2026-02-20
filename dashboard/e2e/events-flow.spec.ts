import { expect, test } from '@playwright/test';

test('events page renders list and detail panel with properties/timeseries', async ({
  page,
}) => {
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
      const type = url.searchParams.get('type');
      if (type === 'page') {
        return json(200, {
          data: {
            type: 'page',
            rows: [{ value: '/pricing', pageviews: 6, visitors: 5 }],
          },
          pagination: { total: 1, limit: 10, offset: 0, has_more: false },
        });
      }
      return json(200, {
        data: { type, rows: [{ value: 'stub', visitors: 3 }] },
        pagination: { total: 1, limit: 10, offset: 0, has_more: false },
      });
    }

    if (path === '/api/websites/site_test/realtime') {
      return json(200, {
        data: {
          active_visitors: 1,
          recent_events: [],
          pagination: { limit: 100, total_in_window: 0 },
        },
      });
    }

    if (path === '/api/websites/site_test/events') {
      return json(200, {
        data: {
          rows: [
            { event_name: 'purchase', count: 5, visitors: 4, prev_count: 2 },
            { event_name: 'signup', count: 2, visitors: 2, prev_count: 1 },
          ],
          total: 2,
        },
      });
    }

    if (path === '/api/websites/site_test/events/properties') {
      return json(200, {
        data: {
          event_name: url.searchParams.get('event_name'),
          total_occurrences: 5,
          sample_size: 2,
          properties: [
            { property_key: 'plan', property_value: 'pro', count: 1 },
            { property_key: 'plan', property_value: 'starter', count: 1 },
          ],
        },
      });
    }

    if (path === '/api/websites/site_test/events/timeseries') {
      return json(200, {
        data: {
          series: [
            { date: '2026-02-19', pageviews: 2, visitors: 2 },
            { date: '2026-02-20', pageviews: 3, visitors: 2 },
          ],
          granularity: 'day',
        },
      });
    }

    return json(404, {
      error: { code: 'not_found', message: 'Not found', field: null },
    });
  });

  await page.goto('/dashboard');
  await expect(page.getByRole('heading', { name: /Test Site/i })).toBeVisible();
  await page.getByRole('button', { name: 'Events' }).click();

  await expect(page.getByText('Custom Events')).toBeVisible();
  await page.getByRole('button', { name: /purchase/i }).first().click();

  await expect(page.getByText('purchase').first()).toBeVisible();
  await expect(page.getByText('Sampled 2 of 5')).toBeVisible();
  await expect(page.getByText('plan', { exact: false })).toBeVisible();
});
