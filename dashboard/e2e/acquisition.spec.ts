import { expect, test } from '@playwright/test';

type StoredLink = {
  id: string;
  website_id: string;
  name: string;
  slug: string;
  destination_url: string;
  utm_source: string | null;
  utm_medium: string | null;
  utm_campaign: string | null;
  utm_term: string | null;
  utm_content: string | null;
  is_active: boolean;
  created_at: string;
  clicks: number;
  unique_visitors: number;
  conversions: number;
  revenue: number;
  tracking_url: string;
};

type StoredPixel = {
  id: string;
  website_id: string;
  name: string;
  pixel_key: string;
  default_url: string | null;
  is_active: boolean;
  created_at: string;
  views: number;
  unique_visitors: number;
  pixel_url: string;
  snippet: string;
};

test('acquisition links and pixels support create/edit/delete and copy flows', async ({ page }) => {
  const websiteId = 'site_test';
  let linkSeq = 1;
  let pixelSeq = 1;
  const links: StoredLink[] = [];
  const pixels: StoredPixel[] = [];

  await page.addInitScript(() => {
    const writes: string[] = [];
    // @ts-expect-error test-only global
    window.__clipboardWrites = writes;
    Object.defineProperty(navigator, 'clipboard', {
      configurable: true,
      value: {
        writeText: async (text: string) => {
          writes.push(text);
        },
      },
    });
  });

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
          prev_pageviews: 0,
          prev_visitors: 0,
          prev_sessions: 0,
          prev_bounce_rate: 0,
          prev_avg_duration_seconds: 0,
          timezone: 'UTC',
        },
      });
    }
    if (path === `/api/websites/${websiteId}/pageviews`) {
      return json(200, {
        data: {
          series: [{ date: '2026-02-20', pageviews: 10, visitors: 8 }],
          granularity: 'day',
          compare_series: [],
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

    if (path === `/api/websites/${websiteId}/links` && method === 'GET') {
      return json(200, { data: links });
    }
    if (path === `/api/websites/${websiteId}/links` && method === 'POST') {
      const body = route.request().postDataJSON() as {
        name: string;
        destination_url: string;
        utm_source?: string | null;
        utm_medium?: string | null;
        utm_campaign?: string | null;
      };
      const slug = `slug${linkSeq}`;
      const created: StoredLink = {
        id: `lnk_${linkSeq++}`,
        website_id: websiteId,
        name: body.name,
        slug,
        destination_url: body.destination_url,
        utm_source: body.utm_source ?? null,
        utm_medium: body.utm_medium ?? null,
        utm_campaign: body.utm_campaign ?? null,
        utm_term: null,
        utm_content: null,
        is_active: true,
        created_at: '2026-02-25T12:00:00Z',
        clicks: 0,
        unique_visitors: 0,
        conversions: 0,
        revenue: 0,
        tracking_url: `http://localhost:3000/l/${slug}`,
      };
      links.unshift(created);
      return json(201, { data: created });
    }

    const linkMatch = path.match(new RegExp(`/api/websites/${websiteId}/links/([^/]+)$`));
    if (linkMatch && method === 'PUT') {
      const link = links.find((item) => item.id === linkMatch[1]);
      if (!link) {
        return json(404, { error: { code: 'not_found', message: 'Link not found', field: null } });
      }
      const body = route.request().postDataJSON() as Partial<StoredLink>;
      Object.assign(link, body);
      return json(200, { data: link });
    }
    if (linkMatch && method === 'DELETE') {
      const idx = links.findIndex((item) => item.id === linkMatch[1]);
      if (idx >= 0) links.splice(idx, 1);
      return route.fulfill({ status: 204 });
    }
    if (path.match(new RegExp(`/api/websites/${websiteId}/links/[^/]+/stats$`))) {
      return json(200, {
        data: { link_id: 'unused', clicks: 0, unique_visitors: 0, conversions: 0, revenue: 0 },
      });
    }

    if (path === `/api/websites/${websiteId}/pixels` && method === 'GET') {
      return json(200, { data: pixels });
    }
    if (path === `/api/websites/${websiteId}/pixels` && method === 'POST') {
      const body = route.request().postDataJSON() as { name: string; default_url?: string | null };
      const pixelKey = `px_${pixelSeq}`;
      const created: StoredPixel = {
        id: `pxl_${pixelSeq++}`,
        website_id: websiteId,
        name: body.name,
        pixel_key: pixelKey,
        default_url: body.default_url ?? null,
        is_active: true,
        created_at: '2026-02-25T12:00:00Z',
        views: 0,
        unique_visitors: 0,
        pixel_url: `http://localhost:3000/p/${pixelKey}.gif`,
        snippet: `<img src="http://localhost:3000/p/${pixelKey}.gif" width="1" height="1" style="display:none" alt="" />`,
      };
      pixels.unshift(created);
      return json(201, { data: created });
    }

    const pixelMatch = path.match(new RegExp(`/api/websites/${websiteId}/pixels/([^/]+)$`));
    if (pixelMatch && method === 'PUT') {
      const pixel = pixels.find((item) => item.id === pixelMatch[1]);
      if (!pixel) {
        return json(404, { error: { code: 'not_found', message: 'Pixel not found', field: null } });
      }
      const body = route.request().postDataJSON() as Partial<StoredPixel>;
      Object.assign(pixel, body);
      return json(200, { data: pixel });
    }
    if (pixelMatch && method === 'DELETE') {
      const idx = pixels.findIndex((item) => item.id === pixelMatch[1]);
      if (idx >= 0) pixels.splice(idx, 1);
      return route.fulfill({ status: 204 });
    }
    if (path.match(new RegExp(`/api/websites/${websiteId}/pixels/[^/]+/stats$`))) {
      return json(200, {
        data: { pixel_id: 'unused', views: 0, unique_visitors: 0 },
      });
    }

    return json(404, { error: { code: 'not_found', message: 'Not found', field: null } });
  });

  await page.goto('/dashboard');
  await page.getByRole('button', { name: 'Campaign Links' }).click();
  await expect(page.getByRole('heading', { name: 'Campaign Links' })).toBeVisible();

  await page.getByPlaceholder('Name').first().fill('Launch Link');
  await page.getByPlaceholder('Destination URL').fill('https://example.com/pricing');
  await page.getByRole('button', { name: 'Create Link' }).click();
  await expect(page.locator('main').getByText('Launch Link').first()).toBeVisible();
  await expect(page.getByText('0 clicks')).toBeVisible();

  await page.getByRole('button', { name: 'Copy' }).first().click();
  const copiedLink = await page.evaluate(() => {
    // @ts-expect-error test-only global
    return window.__clipboardWrites[0] as string;
  });
  expect(copiedLink).toContain('/l/');

  const linkPrompts = [
    'Launch Link Updated',
    'https://example.com/pricing-updated',
    'newsletter',
    'email',
    'spring_launch',
  ];
  const linkDialogHandler = async (dialog: any) => {
    await dialog.accept(linkPrompts.shift() ?? '');
    if (linkPrompts.length === 0) {
      page.off('dialog', linkDialogHandler);
    }
  };
  page.on('dialog', linkDialogHandler);
  await page.getByRole('button', { name: 'Edit' }).first().click();
  await expect(page.locator('main').getByText('Launch Link Updated').first()).toBeVisible();

  await page.locator('tbody tr').first().getByRole('button').last().click();
  await expect(page.locator('main').getByText('Launch Link Updated')).toHaveCount(0);

  await page.getByRole('button', { name: 'Tracking Pixels' }).click();
  await expect(page.getByRole('heading', { name: 'Tracking Pixels' })).toBeVisible();

  await page.getByPlaceholder('Name').first().fill('Email Pixel');
  await page.getByPlaceholder('Default URL (optional)').fill('https://example.com/docs');
  await page.getByRole('button', { name: 'Create Pixel' }).click();
  await expect(page.locator('main').getByText('Email Pixel').first()).toBeVisible();
  await expect(page.getByText('0 views')).toBeVisible();

  await page.getByRole('button', { name: 'Copy Snippet' }).first().click();
  const copiedSnippet = await page.evaluate(() => {
    // @ts-expect-error test-only global
    const writes = window.__clipboardWrites as string[];
    return writes[writes.length - 1];
  });
  expect(copiedSnippet).toContain('<img src=');

  const pixelPrompts = ['Email Pixel Updated', 'https://example.com/docs-updated'];
  const pixelDialogHandler = async (dialog: any) => {
    await dialog.accept(pixelPrompts.shift() ?? '');
    if (pixelPrompts.length === 0) {
      page.off('dialog', pixelDialogHandler);
    }
  };
  page.on('dialog', pixelDialogHandler);
  await page.getByRole('button', { name: 'Edit' }).first().click();
  await expect(page.locator('main').getByText('Email Pixel Updated').first()).toBeVisible();

  await page.locator('tbody tr').first().getByRole('button').last().click();
  await expect(page.locator('main').getByText('Email Pixel Updated')).toHaveCount(0);
});
