import { expect, test } from '@playwright/test';

type StoredReport = {
  id: string;
  website_id: string;
  name: string;
  description: string | null;
  config: Record<string, any>;
  last_run_at: string | null;
  created_at: string;
  updated_at: string;
};

function payloadForType(reportType: string) {
  if (reportType === 'pageviews') {
    return {
      series: [{ date: '2026-02-20', pageviews: 5, visitors: 4 }],
      granularity: 'day',
    };
  }
  if (reportType === 'metrics') {
    return {
      type: 'browser',
      rows: [{ value: 'Chrome', visitors: 4, pageviews: 5, bounce_rate: 0, avg_duration_seconds: 120 }],
      total: 1,
    };
  }
  if (reportType === 'events') {
    return {
      rows: [{ event_name: 'signup', count: 4, visitors: 4 }],
      total: 1,
    };
  }
  return {
    pageviews: 10,
    visitors: 8,
    sessions: 9,
    bounce_rate: 0.4,
    avg_duration_seconds: 120,
    prev_pageviews: 8,
    prev_visitors: 7,
    prev_sessions: 8,
    prev_bounce_rate: 0.45,
    prev_avg_duration_seconds: 100,
    timezone: 'UTC',
  };
}

function withOptionalCompare(config: Record<string, any>, payload: unknown) {
  if (!config.compare_mode || config.compare_mode === 'none') {
    return payload;
  }
  return {
    data: payload,
    compare: {
      mode: config.compare_mode,
      primary_range: ['2026-02-14', '2026-02-20'],
      comparison_range:
        config.compare_mode === 'custom'
          ? [config.compare_start_date ?? '2026-02-01', config.compare_end_date ?? '2026-02-07']
          : ['2026-02-07', '2026-02-13'],
    },
  };
}

test('reports supports preview/run across report types and absolute ranges', async ({ page }) => {
  const websiteId = 'site_test';
  let nextId = 1;
  const reports: StoredReport[] = [];

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
      return json(200, { data: payloadForType('stats') });
    }
    if (path === `/api/websites/${websiteId}/pageviews`) {
      return json(200, { data: payloadForType('pageviews') });
    }
    if (path === `/api/websites/${websiteId}/metrics`) {
      return json(200, {
        data: { type: url.searchParams.get('type') ?? 'page', rows: [] },
        pagination: { total: 0, limit: 10, offset: 0, has_more: false },
      });
    }
    if (path === `/api/websites/${websiteId}/realtime`) {
      return json(200, {
        data: { active_visitors: 0, recent_events: [], pagination: { limit: 100, total_in_window: 0 } },
      });
    }
    if (path === `/api/websites/${websiteId}/reports` && method === 'GET') {
      return json(200, {
        data: reports.map((r) => ({
          id: r.id,
          name: r.name,
          description: r.description,
          report_type: r.config.report_type,
          last_run_at: r.last_run_at,
          created_at: r.created_at,
          updated_at: r.updated_at,
        })),
      });
    }
    if (path === `/api/websites/${websiteId}/reports` && method === 'POST') {
      const body = route.request().postDataJSON() as any;
      const report: StoredReport = {
        id: `report_${nextId++}`,
        website_id: websiteId,
        name: body.name,
        description: body.description ?? null,
        config: body.config,
        last_run_at: null,
        created_at: '2026-02-25T12:00:00Z',
        updated_at: '2026-02-25T12:00:00Z',
      };
      reports.push(report);
      return json(201, { data: report });
    }
    if (path === `/api/websites/${websiteId}/reports/preview` && method === 'POST') {
      const config = route.request().postDataJSON() as any;
      return json(200, {
        data: {
          report_id: null,
          config,
          ran_at: '2026-02-25T12:01:00Z',
          data: withOptionalCompare(config, payloadForType(config.report_type)),
        },
      });
    }

    const reportMatch = path.match(new RegExp(`/api/websites/${websiteId}/reports/([^/]+)$`));
    if (reportMatch && method === 'GET') {
      const report = reports.find((r) => r.id === reportMatch[1]);
      return report
        ? json(200, { data: report })
        : json(404, { error: { code: 'not_found', message: 'Report not found', field: null } });
    }

    const runMatch = path.match(new RegExp(`/api/websites/${websiteId}/reports/([^/]+)/run$`));
    if (runMatch && method === 'POST') {
      const report = reports.find((r) => r.id === runMatch[1]);
      if (!report) {
        return json(404, { error: { code: 'not_found', message: 'Report not found', field: null } });
      }
      report.last_run_at = '2026-02-25T12:02:00Z';
      return json(200, {
        data: {
          report_id: report.id,
          config: report.config,
          ran_at: '2026-02-25T12:02:00Z',
          data: withOptionalCompare(report.config, payloadForType(String(report.config.report_type))),
        },
      });
    }

    return json(404, { error: { code: 'not_found', message: 'Not found', field: null } });
  });

  await page.goto('/dashboard');
  await page.getByRole('button', { name: 'Reports' }).click();

  // Stats + compare metadata
  await page.getByRole('button', { name: 'New Report' }).click();
  let dialog = page.getByRole('dialog');
  await dialog.getByPlaceholder('e.g. Weekly KPI').fill('Compare Stats');
  await dialog.locator('select').nth(2).selectOption('previous_period');
  await dialog.getByRole('button', { name: 'Preview' }).click();
  await expect(page.getByText('Preview: Compare Stats')).toBeVisible();
  await expect(page.getByText(/previous_period:/)).toBeVisible();
  await dialog.getByRole('button', { name: 'Create report' }).click();

  // Metrics + preview
  await page.getByRole('button', { name: 'New Report' }).click();
  dialog = page.getByRole('dialog');
  await dialog.getByPlaceholder('e.g. Weekly KPI').fill('Metrics Report');
  await dialog.locator('select').first().selectOption('metrics');
  await dialog.getByRole('button', { name: 'Preview' }).click();
  await expect(page.getByText('Preview: Metrics Report')).toBeVisible();
  await expect(page.getByText('"rows"')).toBeVisible();
  await dialog.getByRole('button', { name: 'Create report' }).click();
  await page.getByTitle('Run report').nth(1).click();
  await expect(page.getByText('Run: Metrics Report')).toBeVisible();

  // Pageviews
  await page.getByRole('button', { name: 'New Report' }).click();
  dialog = page.getByRole('dialog');
  await dialog.getByPlaceholder('e.g. Weekly KPI').fill('Pageviews Report');
  await dialog.locator('select').first().selectOption('pageviews');
  await dialog.getByRole('button', { name: 'Create report' }).click();
  await page.getByTitle('Run report').nth(2).click();
  await expect(page.getByText('Run: Pageviews Report')).toBeVisible();
  await expect(page.getByText('"series"')).toBeVisible();

  // Events + absolute date range
  await page.getByRole('button', { name: 'New Report' }).click();
  dialog = page.getByRole('dialog');
  await dialog.getByPlaceholder('e.g. Weekly KPI').fill('Events Report');
  await dialog.locator('select').first().selectOption('events');
  await dialog.locator('select').nth(1).selectOption('absolute');
  await dialog.locator('input[type="date"]').first().fill('2026-01-01');
  await dialog.locator('input[type="date"]').nth(1).fill('2026-01-31');
  await dialog.getByRole('button', { name: 'Create report' }).click();
  await page.getByTitle('Run report').nth(3).click();
  await expect(page.getByText('Run: Events Report')).toBeVisible();
  await expect(page.getByText('"event_name": "signup"')).toBeVisible();
});
