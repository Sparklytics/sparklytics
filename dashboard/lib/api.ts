const BASE = typeof window !== 'undefined' ? '' : 'http://localhost:3000';

// In cloud mode, ClerkTokenSync registers this so request() can attach Bearer tokens.
let _tokenGetter: (() => Promise<string | null>) | null = null;

export function setTokenGetter(fn: () => Promise<string | null>): void {
  _tokenGetter = fn;
}

type RequestOptions = {
  method?: string;
  body?: unknown;
};

async function request<T>(path: string, opts: RequestOptions = {}): Promise<T> {
  const headers: Record<string, string> = { 'Content-Type': 'application/json' };
  if (_tokenGetter) {
    const token = await _tokenGetter();
    if (token) headers['Authorization'] = `Bearer ${token}`;
  }

  const res = await fetch(`${BASE}${path}`, {
    method: opts.method ?? 'GET',
    credentials: 'include',
    headers,
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

  if (res.status === 204) {
    return undefined as T;
  }

  const contentType = res.headers.get('content-type') ?? '';
  if (!contentType.includes('application/json')) {
    return undefined as T;
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
  getWebsite: (id: string) => request<{ data: Website }>(`/api/websites/${id}`),
  createWebsite: (payload: { name: string; domain: string; timezone?: string }) =>
    request<{ data: Website }>('/api/websites', { method: 'POST', body: payload }),
  updateWebsite: (id: string, payload: { name?: string; domain?: string; timezone?: string }) =>
    request<{ data: Website }>(`/api/websites/${id}`, { method: 'PUT', body: payload }),
  deleteWebsite: (id: string) =>
    request<void>(`/api/websites/${id}`, { method: 'DELETE' }),

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

  // Sharing
  enableSharing: (websiteId: string) =>
    request<{ data: { share_id: string; share_url: string } }>(
      `/api/websites/${websiteId}/share`,
      { method: 'POST' }
    ),
  disableSharing: (websiteId: string) =>
    request(`/api/websites/${websiteId}/share`, { method: 'DELETE' }),

  // Usage (cloud only — returns null if 404 in self-hosted mode)
  getUsage: async (): Promise<UsageResponse | null> => {
    const res = await fetch(`${BASE}/api/usage`, { credentials: 'include' });
    if (res.status === 404) return null;
    if (!res.ok) throw new Error('Failed to fetch usage');
    const json = await res.json();
    return json.data;
  },

  // Export — triggers a file download
  getExportUrl: (websiteId: string, startDate: string, endDate: string): string =>
    `${BASE}/api/websites/${websiteId}/export?start_date=${startDate}&end_date=${endDate}&format=csv`,

  // API Keys (self-hosted auth)
  listApiKeys: () => request<{ data: ApiKey[] }>('/api/auth/keys'),
  createApiKey: (name: string) =>
    request<{ data: { id: string; name: string; key: string; prefix: string; created_at: string } }>(
      '/api/auth/keys', { method: 'POST', body: { name } }
    ),
  deleteApiKey: (id: string) =>
    request<void>(`/api/auth/keys/${id}`, { method: 'DELETE' }),

  // Password (self-hosted auth)
  changePassword: (currentPassword: string, newPassword: string) =>
    request<{ data: { ok: boolean } }>('/api/auth/password', {
      method: 'PUT',
      body: { current_password: currentPassword, new_password: newPassword },
    }),

  // Custom Events
  getEventNames: (websiteId: string, params: DateRange & Filters) =>
    request<{ data: EventNamesResult }>(
      `/api/websites/${websiteId}/events?${toQuery(params)}`
    ),
  getEventProperties: (websiteId: string, eventName: string, params: DateRange & Filters) =>
    request<{ data: EventPropertiesResult }>(
      `/api/websites/${websiteId}/events/properties?event_name=${encodeURIComponent(eventName)}&${toQuery(params)}`
    ),
  getEventTimeseries: (websiteId: string, eventName: string, params: DateRange & Filters) =>
    request<{ data: PageviewsResponse }>(
      `/api/websites/${websiteId}/events/timeseries?event_name=${encodeURIComponent(eventName)}&${toQuery(params)}`
    ),

  // Sessions Explorer (Sprint 11)
  getSessions: (websiteId: string, params: SessionsParams) =>
    request<SessionsResponse>(`/api/websites/${websiteId}/sessions?${toQuery(params as Record<string, string>)}`),
  getSessionDetail: (websiteId: string, sessionId: string) =>
    request<SessionDetailResponse>(`/api/websites/${websiteId}/sessions/${sessionId}`),

  // Goals & Conversion (Sprint 12)
  listGoals: (websiteId: string) =>
    request<{ data: Goal[] }>(`/api/websites/${websiteId}/goals`),
  createGoal: (websiteId: string, body: CreateGoalPayload) =>
    request<{ data: Goal }>(`/api/websites/${websiteId}/goals`, { method: 'POST', body }),
  updateGoal: (websiteId: string, goalId: string, body: UpdateGoalPayload) =>
    request<{ data: Goal }>(`/api/websites/${websiteId}/goals/${goalId}`, { method: 'PUT', body }),
  deleteGoal: (websiteId: string, goalId: string) =>
    request<void>(`/api/websites/${websiteId}/goals/${goalId}`, { method: 'DELETE' }),
  getGoalStats: (websiteId: string, goalId: string, params: Record<string, string>) =>
    request<{ data: GoalStats }>(`/api/websites/${websiteId}/goals/${goalId}/stats?${toQuery(params as Record<string, string>)}`),

  // Funnel Analysis (Sprint 13)
  listFunnels: (websiteId: string) =>
    request<{ data: FunnelSummary[] }>(`/api/websites/${websiteId}/funnels`),
  getFunnel: (websiteId: string, funnelId: string) =>
    request<{ data: Funnel }>(`/api/websites/${websiteId}/funnels/${funnelId}`),
  createFunnel: (websiteId: string, body: CreateFunnelPayload) =>
    request<{ data: Funnel }>(`/api/websites/${websiteId}/funnels`, { method: 'POST', body }),
  updateFunnel: (websiteId: string, funnelId: string, body: UpdateFunnelPayload) =>
    request<{ data: Funnel }>(`/api/websites/${websiteId}/funnels/${funnelId}`, { method: 'PUT', body }),
  deleteFunnel: (websiteId: string, funnelId: string) =>
    request<void>(`/api/websites/${websiteId}/funnels/${funnelId}`, { method: 'DELETE' }),
  getFunnelResults: (websiteId: string, funnelId: string, params: Record<string, string>) =>
    request<{ data: FunnelResults }>(`/api/websites/${websiteId}/funnels/${funnelId}/results?${toQuery(params)}`),

  // Journey Analysis (Sprint 14)
  getJourney: (websiteId: string, params: JourneyParams) =>
    request<{ data: JourneyResponse }>(
      `/api/websites/${websiteId}/journey?${toQuery(params)}`
    ),

  // Retention Cohorts (Sprint 15)
  getRetention: (websiteId: string, params: RetentionParams) =>
    request<{ data: RetentionResponse }>(
      `/api/websites/${websiteId}/retention?${toQuery(params)}`
    ),
};

function toQuery<T extends object>(params: T): string {
  const entries = Object.entries(params as Record<string, unknown>)
    .filter(([, value]) => value !== undefined && value !== null && value !== '')
    .map(([key, value]) => [key, String(value)]);
  return new URLSearchParams(entries).toString();
}

// ─── Response types ───────────────────────────────────────────────────────

export interface Website {
  id: string;
  name: string;
  domain: string;
  timezone: string;
  created_at: string;
  share_id?: string | null;
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
  /** Always present as of the CTE-based query; was optional for non-page types before. */
  pageviews?: number;
  /** Percentage of sessions with ≤ 1 pageview (0–100). */
  bounce_rate: number;
  /** Mean session duration in seconds. */
  avg_duration_seconds: number;
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

export interface ApiKey {
  id: string;
  name: string;
  prefix: string;
  created_at: string;
  last_used_at: string | null;
  revoked_at: string | null;
}

export interface UsageResponse {
  month: string;
  event_count: number;
  event_limit: number;
  percent_used: number;
  plan: string;
}

// --- Custom Events response types ---

export interface EventNameRow {
  event_name: string;
  count: number;
  visitors: number;
  prev_count?: number;
}

export interface EventNamesResult {
  rows: EventNameRow[];
  total: number;
}

export interface EventPropertyRow {
  property_key: string;
  property_value: string;
  count: number;
}

export interface EventPropertiesResult {
  event_name: string;
  total_occurrences: number;
  sample_size: number;
  properties: EventPropertyRow[];
}

// --- Sessions Explorer types (Sprint 11) ---

export interface SessionListItem {
  session_id: string;
  visitor_id: string;
  first_seen: string;
  last_seen: string;
  duration_seconds: number;
  pageview_count: number;
  event_count: number;
  entry_page: string | null;
  exit_page: string | null;
  country: string | null;
  browser: string | null;
  os: string | null;
  device_type: string | null;
}

export interface SessionsPagination {
  limit: number;
  next_cursor: string | null;
  has_more: boolean;
}

export interface SessionsResponse {
  data: SessionListItem[];
  pagination: SessionsPagination;
}

export interface SessionEventItem {
  id: string;
  event_type: string;
  url: string;
  event_name: string | null;
  event_data: string | null;
  created_at: string;
}

export interface SessionDetailData {
  session: SessionListItem;
  events: SessionEventItem[];
  truncated: boolean;
}

export interface SessionDetailResponse {
  data: SessionDetailData;
}

export interface SessionsParams {
  start_date?: string;
  end_date?: string;
  timezone?: string;
  limit?: number;
  cursor?: string;
  filter_country?: string;
  filter_page?: string;
  filter_referrer?: string;
  filter_browser?: string;
  filter_os?: string;
  filter_device?: string;
  filter_region?: string;
  filter_city?: string;
  filter_hostname?: string;
}

// --- Goals & Conversion types (Sprint 12) ---

export type GoalType = 'page_view' | 'event';
export type MatchOperator = 'equals' | 'contains';

export interface Goal {
  id: string;
  website_id: string;
  name: string;
  goal_type: GoalType;
  match_value: string;
  match_operator: MatchOperator;
  created_at: string;
  updated_at: string;
}

export interface CreateGoalPayload {
  name: string;
  goal_type: GoalType;
  match_value: string;
  match_operator?: MatchOperator;
}

export interface UpdateGoalPayload {
  name?: string;
  match_value?: string;
  match_operator?: MatchOperator;
}

export interface GoalStats {
  goal_id: string;
  conversions: number;
  converting_sessions: number;
  total_sessions: number;
  conversion_rate: number;
  prev_conversions: number | null;
  prev_conversion_rate: number | null;
  trend_pct: number | null;
}

// --- Funnel Analysis types (Sprint 13) ---

export type StepType = 'page_view' | 'event';

export interface FunnelStep {
  id: string;
  funnel_id: string;
  step_order: number;
  step_type: StepType;
  match_value: string;
  match_operator: MatchOperator;
  label: string;
  created_at: string;
}

export interface Funnel {
  id: string;
  website_id: string;
  name: string;
  steps: FunnelStep[];
  created_at: string;
  updated_at: string;
}

export interface FunnelSummary {
  id: string;
  website_id: string;
  name: string;
  step_count: number;
  created_at: string;
  updated_at: string;
}

export interface CreateFunnelStepPayload {
  step_type: StepType;
  match_value: string;
  match_operator?: MatchOperator;
  label?: string;
}

export interface CreateFunnelPayload {
  name: string;
  steps: CreateFunnelStepPayload[];
}

export interface UpdateFunnelPayload {
  name?: string;
  steps?: CreateFunnelStepPayload[];
}

export interface FunnelStepResult {
  step_order: number;
  label: string;
  sessions_reached: number;
  drop_off_count: number;
  drop_off_rate: number;
  conversion_rate_from_start: number;
  conversion_rate_from_previous: number;
}

export interface FunnelResults {
  funnel_id: string;
  name: string;
  total_sessions_entered: number;
  final_conversion_rate: number;
  steps: FunnelStepResult[];
}

// --- Journey Analysis types (Sprint 14) ---

export type AnchorType = 'page' | 'event';
export type JourneyDirection = 'next' | 'previous';

export interface JourneyParams {
  anchor_type: AnchorType;
  anchor_value: string;
  direction: JourneyDirection;
  max_depth?: number;
  start_date: string;
  end_date: string;
  timezone?: string;
  filter_country?: string;
  filter_page?: string;
  filter_referrer?: string;
  filter_browser?: string;
  filter_os?: string;
  filter_device?: string;
  filter_language?: string;
  filter_utm_source?: string;
  filter_utm_medium?: string;
  filter_utm_campaign?: string;
  filter_region?: string;
  filter_city?: string;
  filter_hostname?: string;
}

export interface JourneyNode {
  type: AnchorType;
  value: string;
}

export interface JourneyBranch {
  nodes: string[];
  sessions: number;
  share: number;
}

export interface JourneyResponse {
  anchor: JourneyNode;
  direction: JourneyDirection;
  max_depth: number;
  total_anchor_sessions: number;
  branches: JourneyBranch[];
}

// --- Retention Cohorts types (Sprint 15) ---

export type RetentionGranularity = 'day' | 'week' | 'month';

export interface RetentionPeriod {
  offset: number;
  retained: number;
  rate: number;
}

export interface RetentionCohortRow {
  cohort_start: string;
  cohort_size: number;
  periods: RetentionPeriod[];
}

export interface RetentionSummary {
  avg_period1_rate: number;
  avg_period4_rate: number | null;
}

export interface RetentionResponse {
  granularity: RetentionGranularity;
  max_periods: number;
  rows: RetentionCohortRow[];
  summary: RetentionSummary;
}

export interface RetentionParams {
  start_date: string;
  end_date: string;
  timezone?: string;
  cohort_granularity?: RetentionGranularity;
  max_periods?: number;
  filter_country?: string;
  filter_page?: string;
  filter_referrer?: string;
  filter_browser?: string;
  filter_os?: string;
  filter_device?: string;
  filter_language?: string;
  filter_utm_source?: string;
  filter_utm_medium?: string;
  filter_utm_campaign?: string;
  filter_region?: string;
  filter_city?: string;
  filter_hostname?: string;
}
