'use client';

import { useEffect, useState } from 'react';
import { Plus, ExternalLink, Loader2, Trash2 } from 'lucide-react';
import { AppShell } from '@/components/layout/AppShell';
import { Button } from '@/components/ui/button';
import { TrackingSnippet } from '@/components/settings/TrackingSnippet';
import { SharingToggle } from '@/components/settings/SharingToggle';
import { useWebsites } from '@/hooks/useWebsites';
import { useAuth } from '@/hooks/useAuth';
import { api } from '@/lib/api';

function useUrlWebsiteId(): string {
  const [id, setId] = useState('');
  useEffect(() => {
    function read() {
      const segs = window.location.pathname.split('/').filter(Boolean);
      // /settings → list; /settings/<id> → detail
      setId(segs[0] === 'settings' ? (segs[1] ?? '') : '');
    }
    read();
    window.addEventListener('popstate', read);
    return () => window.removeEventListener('popstate', read);
  }, []);
  return id;
}

function navigate(path: string) {
  window.history.pushState({}, '', path);
  window.dispatchEvent(new PopStateEvent('popstate'));
}

// ---------- Website Detail ----------

function WebsiteDetail({ websiteId }: { websiteId: string }) {
  const { data, isLoading, refetch } = useWebsites();
  const [saving, setSaving] = useState(false);
  const [deleting, setDeleting] = useState(false);
  const [name, setName] = useState('');
  const [domain, setDomain] = useState('');
  const [timezone, setTimezone] = useState('UTC');

  const website = data?.data?.find((w) => w.id === websiteId);

  useEffect(() => {
    if (website) {
      setName(website.name);
      setDomain(website.domain);
      setTimezone(website.timezone);
    }
  }, [website]);

  async function handleSave() {
    setSaving(true);
    try {
      await fetch(`/api/websites/${websiteId}`, {
        method: 'PUT',
        credentials: 'include',
        headers: { 'Content-Type': 'application/json' },
        body: JSON.stringify({ name, domain, timezone }),
      });
      refetch();
    } finally {
      setSaving(false);
    }
  }

  async function handleDelete() {
    if (!confirm(`Delete "${website?.name}"? This cannot be undone.`)) return;
    setDeleting(true);
    try {
      await fetch(`/api/websites/${websiteId}`, {
        method: 'DELETE',
        credentials: 'include',
      });
      navigate('/settings');
    } finally {
      setDeleting(false);
    }
  }

  if (isLoading) {
    return (
      <AppShell websiteId={websiteId}>
        <div className="max-w-2xl flex items-center justify-center py-16">
          <Loader2 className="w-5 h-5 animate-spin text-ink-3" />
        </div>
      </AppShell>
    );
  }

  if (!website) {
    return (
      <AppShell websiteId={websiteId}>
        <div className="max-w-2xl">
          <p className="text-sm text-ink-3">Website not found.</p>
        </div>
      </AppShell>
    );
  }

  return (
    <AppShell websiteId={websiteId}>
      <div className="max-w-2xl space-y-8">
        <div>
          <button
            onClick={() => navigate('/settings')}
            className="text-xs text-ink-3 hover:text-ink mb-4 transition-colors"
          >
            ← All websites
          </button>
          <h1 className="text-lg font-semibold text-ink">{website.name}</h1>
          <p className="text-xs text-ink-3">{website.id}</p>
        </div>

        {/* General */}
        <section className="bg-surface-1 border border-line rounded-lg p-6 space-y-4">
          <h2 className="text-sm font-semibold text-ink">General</h2>
          <div className="space-y-3">
            <label className="block">
              <span className="text-xs text-ink-2 mb-1 block">Name</span>
              <input
                value={name}
                onChange={(e) => setName(e.target.value)}
                className="w-full bg-canvas border border-line rounded-md px-3 py-2 text-sm text-ink focus:outline-none focus:border-spark"
              />
            </label>
            <label className="block">
              <span className="text-xs text-ink-2 mb-1 block">Domain</span>
              <input
                value={domain}
                onChange={(e) => setDomain(e.target.value)}
                className="w-full bg-canvas border border-line rounded-md px-3 py-2 text-sm text-ink focus:outline-none focus:border-spark"
              />
            </label>
            <label className="block">
              <span className="text-xs text-ink-2 mb-1 block">Timezone</span>
              <input
                value={timezone}
                onChange={(e) => setTimezone(e.target.value)}
                placeholder="UTC"
                className="w-full bg-canvas border border-line rounded-md px-3 py-2 text-sm text-ink focus:outline-none focus:border-spark"
              />
            </label>
          </div>
          <Button size="sm" onClick={handleSave} disabled={saving} className="text-xs">
            {saving && <Loader2 className="w-3 h-3 mr-1 animate-spin" />}
            Save changes
          </Button>
        </section>

        {/* Tracking snippet */}
        <section className="bg-surface-1 border border-line rounded-lg p-6">
          <h2 className="text-sm font-semibold text-ink mb-4">Tracking snippet</h2>
          <TrackingSnippet websiteId={websiteId} />
        </section>

        {/* Sharing */}
        <section className="bg-surface-1 border border-line rounded-lg p-6">
          <h2 className="text-sm font-semibold text-ink mb-4">Sharing</h2>
          <SharingToggle websiteId={websiteId} shareId={website.share_id ?? null} />
        </section>

        {/* Danger zone */}
        <section className="bg-surface-1 border border-down/20 rounded-lg p-6">
          <h2 className="text-sm font-semibold text-down mb-2">Danger zone</h2>
          <p className="text-xs text-ink-3 mb-4">
            Deleting a website permanently removes all analytics data. This cannot be undone.
          </p>
          <Button
            variant="outline"
            size="sm"
            onClick={handleDelete}
            disabled={deleting}
            className="text-xs border-down/30 text-down hover:bg-down/10"
          >
            {deleting ? (
              <Loader2 className="w-3 h-3 mr-1 animate-spin" />
            ) : (
              <Trash2 className="w-3 h-3 mr-1" />
            )}
            Delete website
          </Button>
        </section>
      </div>
    </AppShell>
  );
}

// ---------- Website List ----------

export default function SettingsPage() {
  const websiteId = useUrlWebsiteId();
  const { data: authStatus, isSuccess: authLoaded } = useAuth();
  const { data, isLoading, refetch } = useWebsites();
  const [creating, setCreating] = useState(false);

  useEffect(() => {
    if (!authLoaded) return;
    if (authStatus === null) return;
    if (authStatus.setup_required) { window.location.href = '/setup'; return; }
    if (!authStatus.authenticated) { window.location.href = '/login'; }
  }, [authStatus, authLoaded]);

  // Show detail view when websiteId is present in URL
  if (websiteId) {
    return <WebsiteDetail websiteId={websiteId} />;
  }

  const websites = data?.data ?? [];

  async function handleCreate() {
    const name = prompt('Website name:');
    if (!name) return;
    const domain = prompt('Domain (e.g. example.com):');
    if (!domain) return;
    setCreating(true);
    try {
      await api.createWebsite({ name, domain });
      refetch();
    } finally {
      setCreating(false);
    }
  }

  return (
    <AppShell websiteId=''>
      <div className="max-w-2xl space-y-6">
        <div className="flex items-center justify-between">
          <h1 className="text-lg font-semibold text-ink">Websites</h1>
          <Button size="sm" onClick={handleCreate} disabled={creating} className="gap-2">
            <Plus className="w-4 h-4" />
            Add website
          </Button>
        </div>

        {isLoading ? (
          <div className="space-y-2">
            {[1, 2, 3].map((i) => (
              <div key={i} className="h-14 bg-surface-1 border border-line rounded-lg animate-pulse" />
            ))}
          </div>
        ) : websites.length === 0 ? (
          <div className="bg-surface-1 border border-line rounded-lg p-8 text-center">
            <p className="text-sm text-ink-3 mb-4">No websites yet.</p>
            <Button size="sm" onClick={handleCreate}>Add your first website</Button>
          </div>
        ) : (
          <div className="space-y-2">
            {websites.map((site) => (
              <div
                key={site.id}
                className="flex items-center justify-between bg-surface-1 border border-line rounded-lg px-4 py-3"
              >
                <div>
                  <p className="text-sm font-medium text-ink">{site.name}</p>
                  <p className="text-xs text-ink-3">{site.domain}</p>
                </div>
                <div className="flex items-center gap-2">
                  <Button
                    variant="ghost"
                    size="sm"
                    onClick={() => navigate(`/dashboard/${site.id}`)}
                    className="gap-1 text-xs"
                  >
                    <ExternalLink className="w-3 h-3" />
                    View
                  </Button>
                  <Button
                    variant="outline"
                    size="sm"
                    onClick={() => navigate(`/settings/${site.id}`)}
                    className="text-xs"
                  >
                    Configure
                  </Button>
                </div>
              </div>
            ))}
          </div>
        )}
      </div>
    </AppShell>
  );
}
