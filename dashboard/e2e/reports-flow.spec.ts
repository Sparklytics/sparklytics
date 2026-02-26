import { expect, test } from '@playwright/test';

type StoredReport = {
  id: string;
  website_id: string;
  name: string;
  description: string | null;
  config: Record<string, unknown>;
  last_run_at: string | null;
  created_at: string;
  updated_at: string;
};

test('reports page supports create, run, edit, and delete flow', async ({ page }) => {
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
      return json(200, {
        data: {
          pageviews: 10,
          visitors: 8,
          sessions: 9,
          bounce_rate: 0.4,
          avg_duration_seconds: 120,
          prev_pageviews: 9,
          prev_visitors: 7,
          prev_sessions: 8,
          prev_bounce_rate: 0.45,
          prev_avg_duration_seconds: 100,
          timezone: 'UTC',
        },
      });
    }

    if (path === `/api/websites/${websiteId}/pageviews`) {
      return json(200, {
        data: {
          series: [
            { date: '2026-02-20', pageviews: 5, visitors: 4 },
            { date: '2026-02-21', pageviews: 5, visitors: 4 },
          ],
          granularity: 'day',
        },
      });
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
        data: reports.map((report) => ({
          id: report.id,
          name: report.name,
          description: report.description,
          report_type: report.config.report_type,
          last_run_at: report.last_run_at,
          created_at: report.created_at,
          updated_at: report.updated_at,
        })),
      });
    }

    if (path === `/api/websites/${websiteId}/reports` && method === 'POST') {
      const body = route.request().postDataJSON() as {
        name: string;
        description?: string | null;
        config: Record<string, unknown>;
      };
      const now = '2026-02-25T12:00:00Z';
      const report: StoredReport = {
        id: `report_${nextId++}`,
        website_id: websiteId,
        name: body.name,
        description: body.description ?? null,
        config: body.config,
        last_run_at: null,
        created_at: now,
        updated_at: now,
      };
      reports.push(report);
      return json(201, { data: report });
    }

    if (path === `/api/websites/${websiteId}/reports/preview` && method === 'POST') {
      const config = route.request().postDataJSON() as Record<string, unknown>;
      return json(200, {
        data: {
          report_id: null,
          config,
          ran_at: '2026-02-25T12:01:00Z',
          data: { pageviews: 10, visitors: 8 },
        },
      });
    }

    const reportMatch = path.match(new RegExp(`/api/websites/${websiteId}/reports/([^/]+)$`));
    if (reportMatch && method === 'GET') {
      const report = reports.find((item) => item.id === reportMatch[1]);
      if (!report) {
        return json(404, { error: { code: 'not_found', message: 'Report not found', field: null } });
      }
      return json(200, { data: report });
    }

    if (reportMatch && method === 'PUT') {
      const report = reports.find((item) => item.id === reportMatch[1]);
      if (!report) {
        return json(404, { error: { code: 'not_found', message: 'Report not found', field: null } });
      }
      const body = route.request().postDataJSON() as {
        name?: string;
        description?: string | null;
        config?: Record<string, unknown>;
      };
      report.name = body.name ?? report.name;
      if ('description' in body) report.description = body.description ?? null;
      report.config = body.config ?? report.config;
      report.updated_at = '2026-02-25T12:02:00Z';
      return json(200, { data: report });
    }

    if (reportMatch && method === 'DELETE') {
      const index = reports.findIndex((item) => item.id === reportMatch[1]);
      if (index >= 0) reports.splice(index, 1);
      return route.fulfill({ status: 204 });
    }

    const runMatch = path.match(new RegExp(`/api/websites/${websiteId}/reports/([^/]+)/run$`));
    if (runMatch && method === 'POST') {
      const report = reports.find((item) => item.id === runMatch[1]);
      if (!report) {
        return json(404, { error: { code: 'not_found', message: 'Report not found', field: null } });
      }
      report.last_run_at = '2026-02-25T12:03:00Z';
      return json(200, {
        data: {
          report_id: report.id,
          config: report.config,
          ran_at: '2026-02-25T12:03:00Z',
          data: { pageviews: 10, visitors: 8 },
        },
      });
    }

    return json(404, { error: { code: 'not_found', message: 'Not found', field: null } });
  });

  await page.goto('/dashboard');
  await page.getByRole('button', { name: 'Reports' }).click();
  await expect(page.getByRole('heading', { name: 'Reports' })).toBeVisible();

  await page.getByRole('button', { name: 'New Report' }).click();
  await page.getByPlaceholder('e.g. Weekly KPI').fill('Signup Report');
  await page.getByRole('button', { name: 'Create report' }).click();
  await expect(page.locator('main').getByText('Signup Report').first()).toBeVisible();

  await page.getByTitle('Run report').first().click();
  await expect(page.getByText('Run: Signup Report')).toBeVisible();
  await expect(page.getByText('"pageviews": 10')).toBeVisible();

  await page.getByTitle('Edit report').first().click();
  await page.getByPlaceholder('e.g. Weekly KPI').fill('Signup Report v2');
  await page.getByRole('button', { name: 'Save changes' }).click();
  await expect(page.locator('main').getByText('Signup Report v2').first()).toBeVisible();

  await page.getByTitle('Delete report').first().click();
  await page.getByRole('button', { name: 'Delete' }).click();
  await expect(page.getByText('No reports yet')).toBeVisible();
});
