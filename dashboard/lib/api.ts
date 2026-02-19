const BASE = typeof window !== 'undefined' ? '' : 'http://localhost:3000';

type RequestOptions = {
  method?: string;
  body?: unknown;
};

async function request<T>(path: string, opts: RequestOptions = {}): Promise<T> {
  const res = await fetch(`${BASE}${path}`, {
    method: opts.method ?? 'GET',
    credentials: 'include',
    headers: {
      'Content-Type': 'application/json',
    },
    body: opts.body !== undefined ? JSON.stringify(opts.body) : undefined,
  });

  if (res.status === 401) {
    if (typeof window !== 'undefined') window.location.href = '/login';
    throw new Error('Unauthorized');
  }

  if (!res.ok) {
    const err = await res.json().catch(() => ({ error: { message: 'Request failed' } }));
    throw new Error(err?.error?.message ?? 'Request failed');
  }

  return res.json() as Promise<T>;
}

export type DateRange = { start_date: string; end_date: string };
export type Filters = Record<string, string>;

export const api = {
  // Auth — getAuthStatus returns null when auth mode is "none" (endpoint returns 404)
  getAuthStatus: async (): Promise<{ mode: string; setup_required: boolean; authenticated: boolean } | null> => {
    const res = await fetch(`${BASE}/api/auth/status`, { credentials: 'include' });
    if (res.status === 404) return null;
    if (res.status === 401) {
      if (typeof window !== 'undefined') window.location.href = '/login';
      throw new Error('Unauthorized');
    }
    if (!res.ok) {
      const err = await res.json().catch(() => ({ error: { message: 'Request failed' } }));
      throw new Error(err?.error?.message ?? 'Auth status failed');
    }
    return res.json();
  },
  login: (password: string) => request('/api/auth/login', { method: 'POST', body: { password } }),
  logout: () => request('/api/auth/logout', { method: 'POST' }),
  setup: (password: string) => request('/api/auth/setup', { method: 'POST', body: { password } }),

  // Websites
  getWebsites: () => request<{ data: Website[] }>('/api/websites'),
  createWebsite: (payload: { name: string; domain: string }) =>
    request<{ data: Website }>('/api/websites', { method: 'POST', body: payload }),

  // Analytics
  getStats: (websiteId: string, params: DateRange & Filters) =>
    request<{ data: StatsResponse }>(`/api/websites/${websiteId}/stats?${toQuery(params)}`),
  getPageviews: (websiteId: string, params: DateRange & Filters) =>
    request<{ data: PageviewsResponse }>(`/api/websites/${websiteId}/pageviews?${toQuery(params)}`),
  getMetrics: (websiteId: string, type: string, params: DateRange & Filters) =>
    request<{ data: MetricsResult; pagination: MetricsPagination }>(
      `/api/websites/${websiteId}/metrics?type=${type}&${toQuery(params)}`
    ),
  getRealtime: (websiteId: string) =>
    request<{ data: RealtimeResponse }>(`/api/websites/${websiteId}/realtime`),
};

function toQuery(params: Record<string, string>): string {
  return new URLSearchParams(params).toString();
}

// ─── Response types ───────────────────────────────────────────────────────

export interface Website {
  id: string;
  name: string;
  domain: string;
  timezone: string;
  created_at: string;
}

export interface StatsResponse {
  pageviews: number;
  visitors: number;
  sessions: number;
  bounce_rate: number;
  avg_duration_seconds: number;
  timezone: string;
  prev_pageviews: number;
  prev_visitors: number;
  prev_sessions: number;
  prev_bounce_rate: number;
  prev_avg_duration_seconds: number;
}

export interface PageviewsPoint {
  date: string;
  pageviews: number;
  visitors: number;
}

export interface PageviewsResponse {
  series: PageviewsPoint[];
  granularity: string;
}

export interface MetricRow {
  value: string;
  visitors: number;
  pageviews?: number;
}

export interface MetricsResult {
  type: string;
  rows: MetricRow[];
}

export interface MetricsPagination {
  total: number;
  limit: number;
  offset: number;
  has_more: boolean;
}

export interface RealtimeEvent {
  event_type: string;
  url: string;
  referrer_domain?: string;
  country?: string;
  browser?: string;
  device_type?: string;
  ts: string;
}

export interface RealtimeResponse {
  active_visitors: number;
  recent_events: RealtimeEvent[];
  pagination: {
    limit: number;
    total_in_window: number;
  };
}
