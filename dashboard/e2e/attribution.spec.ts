import { expect, test } from '@playwright/test';

test('attribution page loads and model toggle updates channel breakdown', async ({ page }) => {
  const websiteId = 'site_test';

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
            id: websiteId,
            name: 'Test Site',
            domain: 'example.com',
            timezone: 'UTC',
            created_at: '2026-02-20T00:00:00Z',
            share_id: null,
          },
        ],
      });
    }
    if (path === `/api/websites/${websiteId}/stats`) {
      return json(200, {
        data: {
          pageviews: 20,
          visitors: 10,
          sessions: 12,
          bounce_rate: 0.4,
          avg_duration_seconds: 120,
          prev_pageviews: 15,
          prev_visitors: 8,
          prev_sessions: 10,
          prev_bounce_rate: 0.5,
          prev_avg_duration_seconds: 100,
          timezone: 'UTC',
        },
      });
    }
    if (path === `/api/websites/${websiteId}/pageviews`) {
      return json(200, {
        data: {
          series: [{ date: '2026-02-20', pageviews: 20, visitors: 10 }],
          granularity: 'day',
          compare_series: [],
        },
      });
    }
    if (path === `/api/websites/${websiteId}/metrics`) {
      return json(200, {
        data: { type: query.get('type') ?? 'page', rows: [] },
        pagination: { total: 0, limit: 10, offset: 0, has_more: false },
      });
    }
    if (path === `/api/websites/${websiteId}/realtime`) {
      return json(200, {
        data: { active_visitors: 0, recent_events: [], pagination: { limit: 100, total_in_window: 0 } },
      });
    }
    if (path === `/api/websites/${websiteId}/goals`) {
      return json(200, {
        data: [
          {
            id: 'goal_signup',
            website_id: websiteId,
            name: 'Signup',
            goal_type: 'event',
            match_value: 'signup',
            match_operator: 'equals',
            value_mode: 'fixed',
            fixed_value: 50,
            value_property_key: null,
            currency: 'USD',
            created_at: '2026-02-20T00:00:00Z',
            updated_at: '2026-02-20T00:00:00Z',
          },
        ],
      });
    }
    if (path === `/api/websites/${websiteId}/attribution`) {
      const model = query.get('model') ?? 'last_touch';
      if (model === 'first_touch') {
        return json(200, {
          data: {
            goal_id: 'goal_signup',
            model: 'first_touch',
            rows: [{ channel: 'google / cpc', conversions: 2, revenue: 80, share: 1 }],
            totals: { conversions: 2, revenue: 80 },
          },
        });
      }
      return json(200, {
        data: {
          goal_id: 'goal_signup',
          model: 'last_touch',
          rows: [{ channel: 'newsletter / email', conversions: 2, revenue: 100, share: 1 }],
          totals: { conversions: 2, revenue: 100 },
        },
      });
    }
    if (path === `/api/websites/${websiteId}/revenue/summary`) {
      const model = query.get('model') ?? 'last_touch';
      if (model === 'first_touch') {
        return json(200, {
          data: {
            goal_id: 'goal_signup',
            model: 'first_touch',
            conversions: 2,
            revenue: 80,
          },
        });
      }
      return json(200, {
        data: {
          goal_id: 'goal_signup',
          model: 'last_touch',
          conversions: 2,
          revenue: 100,
        },
      });
    }

    return json(404, { error: { code: 'not_found', message: 'Not found', field: null } });
  });

  await page.goto('/dashboard');
  await page.getByRole('button', { name: 'Attribution' }).click();

  await expect(page.getByRole('heading', { name: 'Attribution', exact: true })).toBeVisible();
  await expect(page.getByText('newsletter / email')).toBeVisible();
  await expect(page.getByText('Revenue (Last touch)')).toBeVisible();
  await expect(page.getByTestId('attribution-revenue-value')).toHaveText('100.00');

  await page.getByRole('button', { name: 'First touch' }).click();
  await expect(page.getByText('Revenue (First touch)')).toBeVisible();
  await expect(page.getByText('google / cpc')).toBeVisible();
  await expect(page.getByTestId('attribution-revenue-value')).toHaveText('80.00');
});
