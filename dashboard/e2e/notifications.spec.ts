import { expect, test } from '@playwright/test';

type Subscription = {
  id: string;
  website_id: string;
  report_id: string;
  schedule: 'daily' | 'weekly' | 'monthly';
  timezone: string;
  channel: 'email' | 'webhook';
  target: string;
  is_active: boolean;
  last_run_at: string | null;
  next_run_at: string;
  created_at: string;
};

type AlertRule = {
  id: string;
  website_id: string;
  name: string;
  metric: 'pageviews' | 'visitors' | 'conversions' | 'conversion_rate';
  condition_type: 'spike' | 'drop' | 'threshold_above' | 'threshold_below';
  threshold_value: number;
  lookback_days: number;
  channel: 'email' | 'webhook';
  target: string;
  is_active: boolean;
  created_at: string;
};

type Delivery = {
  id: string;
  source_type: 'subscription' | 'alert';
  source_id: string;
  idempotency_key: string;
  status: 'sent' | 'failed';
  error_message: string | null;
  delivered_at: string;
};

test('notifications settings supports subscription and alert test-send flows', async ({ page }) => {
  const websiteId = 'site_test';
  const subscriptions: Subscription[] = [];
  const alerts: AlertRule[] = [];
  const deliveries: Delivery[] = [];
  let subSeq = 1;
  let alertSeq = 1;
  let deliverySeq = 1;

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
      return json(404, { error: { code: 'not_found', message: 'Not found', field: null } });
    }
    if (path === '/api/usage') {
      return route.fulfill({ status: 404 });
    }
    if (path === '/api/websites') {
      return json(200, {
        data: [
          {
            id: websiteId,
            name: 'Test Site',
            domain: 'example.com',
            timezone: 'UTC',
            created_at: '2026-02-25T00:00:00Z',
            share_id: null,
          },
        ],
      });
    }
    if (path === `/api/websites/${websiteId}`) {
      return json(200, {
        data: {
          id: websiteId,
          name: 'Test Site',
          domain: 'example.com',
          timezone: 'UTC',
          created_at: '2026-02-25T00:00:00Z',
          share_id: null,
        },
      });
    }
    if (path === `/api/websites/${websiteId}/reports`) {
      return json(200, {
        data: [
          {
            id: 'report_1',
            name: 'Daily KPIs',
            description: null,
            report_type: 'stats',
            last_run_at: null,
            created_at: '2026-02-25T00:00:00Z',
            updated_at: '2026-02-25T00:00:00Z',
          },
        ],
      });
    }
    if (path === `/api/websites/${websiteId}/subscriptions` && method === 'GET') {
      return json(200, { data: subscriptions });
    }
    if (path === `/api/websites/${websiteId}/subscriptions` && method === 'POST') {
      const body = route.request().postDataJSON() as {
        report_id: string;
        schedule: 'daily' | 'weekly' | 'monthly';
        timezone: string;
        channel: 'email' | 'webhook';
        target: string;
      };
      const row: Subscription = {
        id: `sub_${subSeq++}`,
        website_id: websiteId,
        report_id: body.report_id,
        schedule: body.schedule,
        timezone: body.timezone,
        channel: body.channel,
        target: body.target,
        is_active: true,
        last_run_at: null,
        next_run_at: '2026-02-26T00:00:00Z',
        created_at: '2026-02-25T00:00:00Z',
      };
      subscriptions.unshift(row);
      return json(201, { data: row });
    }
    const subTestMatch = path.match(new RegExp(`/api/websites/${websiteId}/subscriptions/([^/]+)/test$`));
    if (subTestMatch && method === 'POST') {
      const row: Delivery = {
        id: `delivery_${deliverySeq++}`,
        source_type: 'subscription',
        source_id: subTestMatch[1],
        idempotency_key: `sub-test-${Date.now()}`,
        status: 'sent',
        error_message: null,
        delivered_at: '2026-02-25T00:00:00Z',
      };
      deliveries.unshift(row);
      return json(200, { data: row });
    }
    if (path === `/api/websites/${websiteId}/alerts` && method === 'GET') {
      return json(200, { data: alerts });
    }
    if (path === `/api/websites/${websiteId}/alerts` && method === 'POST') {
      const body = route.request().postDataJSON() as {
        name: string;
        metric: AlertRule['metric'];
        condition_type: AlertRule['condition_type'];
        threshold_value: number;
        lookback_days: number;
        channel: AlertRule['channel'];
        target: string;
      };
      const row: AlertRule = {
        id: `alert_${alertSeq++}`,
        website_id: websiteId,
        name: body.name,
        metric: body.metric,
        condition_type: body.condition_type,
        threshold_value: body.threshold_value,
        lookback_days: body.lookback_days,
        channel: body.channel,
        target: body.target,
        is_active: true,
        created_at: '2026-02-25T00:00:00Z',
      };
      alerts.unshift(row);
      return json(201, { data: row });
    }
    const alertTestMatch = path.match(new RegExp(`/api/websites/${websiteId}/alerts/([^/]+)/test$`));
    if (alertTestMatch && method === 'POST') {
      const row: Delivery = {
        id: `delivery_${deliverySeq++}`,
        source_type: 'alert',
        source_id: alertTestMatch[1],
        idempotency_key: `alert-test-${Date.now()}`,
        status: 'failed',
        error_message: 'timeout',
        delivered_at: '2026-02-25T00:00:00Z',
      };
      deliveries.unshift(row);
      return json(200, { data: row });
    }
    if (path === `/api/websites/${websiteId}/notifications/history`) {
      return json(200, { data: deliveries });
    }
    if (path.match(new RegExp(`/api/websites/${websiteId}/(subscriptions|alerts)/[^/]+$`)) && method === 'PUT') {
      const body = route.request().postDataJSON() as Record<string, unknown>;
      const id = path.split('/').pop() as string;
      const bucket = path.includes('/subscriptions/') ? subscriptions : alerts;
      const row = bucket.find((item) => item.id === id);
      if (!row) {
        return json(404, { error: { code: 'not_found', message: 'Not found', field: null } });
      }
      Object.assign(row, body);
      return json(200, { data: row });
    }
    if (path.match(new RegExp(`/api/websites/${websiteId}/(subscriptions|alerts)/[^/]+$`)) && method === 'DELETE') {
      return route.fulfill({ status: 204 });
    }

    return json(404, { error: { code: 'not_found', message: 'Not found', field: null } });
  });

  await page.goto('/dashboard');
  await page.getByRole('button', { name: 'General' }).click();
  await page.getByRole('button', { name: 'Notifications' }).click();
  await expect(page.getByRole('heading', { name: 'Notifications' })).toBeVisible();

  await page.getByRole('button', { name: 'New subscription' }).click();
  await page.getByLabel('Report').selectOption('report_1');
  await page.getByLabel('Schedule').selectOption('daily');
  await page.getByLabel('Timezone').fill('UTC');
  await page.getByLabel('Channel').selectOption('email');
  await page.getByLabel('Email target').fill('ops@example.com');
  await page.getByRole('button', { name: 'Create subscription' }).click();
  await expect(page.getByText('ops@example.com')).toBeVisible();

  await page.locator('table').first().getByRole('button', { name: 'Test' }).first().click();
  await expect(page.getByText('subscription:')).toBeVisible();

  await page.getByRole('button', { name: 'New alert' }).click();
  await page.getByLabel('Name').fill('Traffic spike');
  await page.getByLabel('Metric').selectOption('pageviews');
  await page.getByLabel('Condition').selectOption('spike');
  await page.getByLabel('Threshold').fill('2');
  await page.getByLabel('Lookback days').fill('7');
  await page.getByLabel('Channel').selectOption('email');
  await page.getByLabel('Email target').fill('ops@example.com');
  await page.getByRole('button', { name: 'Create alert' }).click();
  await expect(page.getByText('Traffic spike')).toBeVisible();

  await page.locator('table').nth(1).getByRole('button', { name: 'Test' }).first().click();
  await expect(page.getByText('alert:')).toBeVisible();
  await expect(page.getByText('failed')).toBeVisible();
});
