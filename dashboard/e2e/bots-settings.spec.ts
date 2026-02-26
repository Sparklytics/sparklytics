import { expect, test } from '@playwright/test';

type BotPolicyMode = 'strict' | 'balanced' | 'off';
type MatchType = 'ua_contains' | 'ip_exact' | 'ip_cidr';

type BotPolicy = {
  website_id: string;
  mode: BotPolicyMode;
  threshold_score: number;
  updated_at: string;
};

type ListEntry = {
  id: string;
  match_type: MatchType;
  match_value: string;
  note: string | null;
  created_at: string;
};

type AuditRecord = {
  id: string;
  actor: string;
  action: string;
  payload: Record<string, unknown>;
  created_at: string;
};

test('bots settings supports policy, list overrides, recompute, and audit visibility', async ({ page }) => {
  const websiteId = 'site_test';
  let policy: BotPolicy = {
    website_id: websiteId,
    mode: 'balanced',
    threshold_score: 70,
    updated_at: '2026-02-26T00:00:00Z',
  };
  const allowlist: ListEntry[] = [];
  const blocklist: ListEntry[] = [];
  const audit: AuditRecord[] = [];
  let listSeq = 1;
  let auditSeq = 1;
  const recomputeJobId = 'bot_recompute_1';
  let recomputePolls = 0;

  const pushAudit = (action: string, payload: Record<string, unknown>) => {
    audit.unshift({
      id: `audit_${auditSeq++}`,
      actor: 'selfhosted',
      action,
      payload,
      created_at: '2026-02-26T00:00:00Z',
    });
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
            created_at: '2026-02-26T00:00:00Z',
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
          created_at: '2026-02-26T00:00:00Z',
          share_id: null,
        },
      });
    }

    if (path === `/api/websites/${websiteId}/bot/policy` && method === 'GET') {
      return json(200, { data: policy });
    }
    if (path === `/api/websites/${websiteId}/bot/policy` && method === 'PUT') {
      const body = route.request().postDataJSON() as { mode: BotPolicyMode; threshold_score: number };
      policy = {
        ...policy,
        mode: body.mode,
        threshold_score: body.threshold_score,
        updated_at: '2026-02-26T00:10:00Z',
      };
      pushAudit('policy_update', body);
      return json(200, { data: policy });
    }

    if (path === `/api/websites/${websiteId}/bot-summary`) {
      return json(200, {
        data: {
          website_id: websiteId,
          start_date: '2026-02-20T00:00:00Z',
          end_date: '2026-02-26T00:00:00Z',
          bot_events: 12,
          human_events: 120,
          bot_rate: 0.09,
          top_reasons: [{ code: 'ua_signature', count: 9 }],
        },
      });
    }
    if (path === `/api/websites/${websiteId}/bot/report`) {
      return json(200, {
        data: {
          split: { bot_events: 12, human_events: 120, bot_rate: 0.09 },
          timeseries: [
            { period_start: '2026-02-25T00:00:00Z', bot_events: 4, human_events: 60 },
            { period_start: '2026-02-26T00:00:00Z', bot_events: 8, human_events: 60 },
          ],
          top_reasons: [{ code: 'ua_signature', count: 9 }],
          top_user_agents: [{ value: 'Googlebot/2.1', count: 6 }],
        },
      });
    }

    if (path === `/api/websites/${websiteId}/bot/allowlist` && method === 'GET') {
      return json(200, { data: allowlist, next_cursor: null });
    }
    if (path === `/api/websites/${websiteId}/bot/allowlist` && method === 'POST') {
      const body = route.request().postDataJSON() as {
        match_type: MatchType;
        match_value: string;
        note?: string | null;
      };
      const entry: ListEntry = {
        id: `allow_${listSeq++}`,
        match_type: body.match_type,
        match_value: body.match_value,
        note: body.note ?? null,
        created_at: '2026-02-26T00:00:00Z',
      };
      allowlist.unshift(entry);
      pushAudit('allow_add', { id: entry.id, match_type: entry.match_type, match_value: entry.match_value });
      return json(201, { data: entry });
    }
    const allowDeleteMatch = path.match(new RegExp(`/api/websites/${websiteId}/bot/allowlist/([^/]+)$`));
    if (allowDeleteMatch && method === 'DELETE') {
      const index = allowlist.findIndex((row) => row.id === allowDeleteMatch[1]);
      if (index >= 0) {
        const [entry] = allowlist.splice(index, 1);
        pushAudit('allow_remove', { id: entry.id });
        return route.fulfill({ status: 204 });
      }
      return json(404, { error: { code: 'not_found', message: 'not found', field: null } });
    }

    if (path === `/api/websites/${websiteId}/bot/blocklist` && method === 'GET') {
      return json(200, { data: blocklist, next_cursor: null });
    }
    if (path === `/api/websites/${websiteId}/bot/blocklist` && method === 'POST') {
      const body = route.request().postDataJSON() as {
        match_type: MatchType;
        match_value: string;
        note?: string | null;
      };
      const entry: ListEntry = {
        id: `block_${listSeq++}`,
        match_type: body.match_type,
        match_value: body.match_value,
        note: body.note ?? null,
        created_at: '2026-02-26T00:00:00Z',
      };
      blocklist.unshift(entry);
      pushAudit('block_add', { id: entry.id, match_type: entry.match_type, match_value: entry.match_value });
      return json(201, { data: entry });
    }
    const blockDeleteMatch = path.match(new RegExp(`/api/websites/${websiteId}/bot/blocklist/([^/]+)$`));
    if (blockDeleteMatch && method === 'DELETE') {
      const index = blocklist.findIndex((row) => row.id === blockDeleteMatch[1]);
      if (index >= 0) {
        const [entry] = blocklist.splice(index, 1);
        pushAudit('block_remove', { id: entry.id });
        return route.fulfill({ status: 204 });
      }
      return json(404, { error: { code: 'not_found', message: 'not found', field: null } });
    }

    if (path === `/api/websites/${websiteId}/bot/recompute` && method === 'POST') {
      pushAudit('recompute_start', { job_id: recomputeJobId });
      recomputePolls = 0;
      return json(202, { job_id: recomputeJobId, status: 'queued' });
    }
    if (path === `/api/websites/${websiteId}/bot/recompute/${recomputeJobId}` && method === 'GET') {
      recomputePolls += 1;
      return json(200, {
        data: {
          job_id: recomputeJobId,
          website_id: websiteId,
          status: recomputePolls > 1 ? 'success' : 'running',
          start_date: '2026-02-20T00:00:00Z',
          end_date: '2026-02-26T00:00:00Z',
          created_at: '2026-02-26T00:00:00Z',
          started_at: '2026-02-26T00:00:01Z',
          completed_at: recomputePolls > 1 ? '2026-02-26T00:00:03Z' : null,
          error_message: null,
        },
      });
    }

    if (path === `/api/websites/${websiteId}/bot/audit`) {
      return json(200, { data: audit, next_cursor: null });
    }

    return json(404, { error: { code: 'not_found', message: 'Not found', field: null } });
  });

  await page.goto('/dashboard');
  await page.getByRole('button', { name: 'General' }).click();
  await page.getByRole('button', { name: 'Bots' }).click();

  await expect(page.getByRole('heading', { name: 'Bots' })).toBeVisible();
  await expect(page.getByText('9.00%')).toBeVisible();
  await expect(page.getByText('ua_signature')).toBeVisible();
  await expect(page.getByText('Googlebot/2.1')).toBeVisible();

  await page.getByRole('button', { name: /strict/i }).click();
  await page.getByRole('button', { name: 'Save policy' }).click();

  await page.getByPlaceholder('match value').first().fill('my-monitor');
  await page.getByPlaceholder('note (optional)').first().fill('synthetic');
  await page.getByRole('button', { name: 'Add to allowlist' }).click();
  await expect(page.getByRole('cell', { name: 'my-monitor' })).toBeVisible();

  await page.getByPlaceholder('match value').nth(1).fill('203.0.113.10');
  await page.getByPlaceholder('note (optional)').nth(1).fill('abusive');
  await page.getByRole('button', { name: 'Add to blocklist' }).click();
  await expect(page.getByRole('cell', { name: '203.0.113.10' })).toBeVisible();

  await page.getByRole('button', { name: 'Recompute last N days' }).click();
  await expect(page.getByText('Job:')).toBeVisible();
  await expect(page.getByText('success')).toBeVisible();

  await expect(page.getByText('policy_update').first()).toBeVisible();
  await expect(page.getByText('recompute_start').first()).toBeVisible();
});
