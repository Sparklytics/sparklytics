'use client';

import { useEffect, useState } from 'react';
import { Loader2, Trash2 } from 'lucide-react';
import { Button } from '@/components/ui/button';
import { TrackingSnippet } from '@/components/settings/TrackingSnippet';
import { SharingToggle } from '@/components/settings/SharingToggle';
import { ApiKeysSection } from '@/components/settings/ApiKeysSection';
import { ChangePasswordSection } from '@/components/settings/ChangePasswordSection';
import { BotsSettingsPage } from '@/components/settings/BotsSettingsPage';
import { NotificationsSettingsPage } from '@/components/notifications/NotificationsSettingsPage';
import { ConfirmDialog } from '@/components/ui/confirm-dialog';
import {
  useWebsite,
  useUpdateWebsite,
  useDeleteWebsite,
  useWebsiteIngestLimits,
  useUpdateWebsiteIngestLimits,
} from '@/hooks/useWebsites';
import { useAuth } from '@/hooks/useAuth';
import { TIMEZONE_GROUPS } from '@/lib/timezones';

function navigate(path: string) {
  window.history.pushState({}, '', path);
  window.dispatchEvent(new PopStateEvent('popstate'));
}

export function WebsiteDetail({ websiteId, subSubPage = 'general' }: { websiteId: string, subSubPage?: string }) {
  const { data: websiteData, isLoading } = useWebsite(websiteId);
  const updateWebsite = useUpdateWebsite(websiteId);
  const deleteWebsite = useDeleteWebsite();
  const ingestLimitsQuery = useWebsiteIngestLimits(websiteId);
  const updateIngestLimits = useUpdateWebsiteIngestLimits(websiteId);
  const { data: authStatus } = useAuth();
  const [name, setName] = useState('');
  const [domain, setDomain] = useState('');
  const [timezone, setTimezone] = useState('UTC');
  const [peakEventsPerSec, setPeakEventsPerSec] = useState('');
  const [queueMaxEvents, setQueueMaxEvents] = useState('');
  const [customPeak, setCustomPeak] = useState(false);
  const [customQueue, setCustomQueue] = useState(false);
  const [showDeleteConfirm, setShowDeleteConfirm] = useState(false);

  const website = websiteData?.data;
  const showPasswordSection = authStatus?.mode === 'local';
  const showApiKeysSection =
    authStatus?.mode === 'password' || authStatus?.mode === 'local';

  useEffect(() => {
    if (website) {
      setName(website.name);
      setDomain(website.domain);
      setTimezone(website.timezone);
    }
  }, [website]);

  useEffect(() => {
    const limits = ingestLimitsQuery.data?.data;
    if (!limits) return;
    setPeakEventsPerSec(String(limits.peak_events_per_sec));
    setQueueMaxEvents(String(limits.queue_max_events));
    setCustomPeak(limits.source.peak_events_per_sec === 'custom');
    setCustomQueue(limits.source.queue_max_events === 'custom');
  }, [ingestLimitsQuery.data]);

  async function handleSave() {
    await updateWebsite.mutateAsync({ name, domain, timezone });
  }

  async function handleDelete() {
    await deleteWebsite.mutateAsync(websiteId);
    setShowDeleteConfirm(false);
    navigate('/settings');
  }

  async function handleSaveIngestion() {
    const parsedPeak = Number(peakEventsPerSec);
    const parsedQueue = Number(queueMaxEvents);
    if (customPeak && (!Number.isInteger(parsedPeak) || parsedPeak <= 0)) {
      return;
    }
    if (customQueue && (!Number.isInteger(parsedQueue) || parsedQueue <= 0)) {
      return;
    }
    await updateIngestLimits.mutateAsync({
      peak_events_per_sec: customPeak ? parsedPeak : null,
      queue_max_events: customQueue ? parsedQueue : null,
    });
  }

  if (isLoading) {
    return (
      <div className="max-w-2xl flex items-center justify-center py-16">
        <Loader2 className="w-5 h-5 animate-spin text-ink-3" />
      </div>
    );
  }

  if (!website) {
    return (
      <div className="max-w-2xl">
        <p className="text-sm text-ink-3">Website not found.</p>
      </div>
    );
  }

  return (
    <div className="max-w-2xl space-y-8">
      <div>
        <button
          onClick={() => navigate('/settings')}
          className="text-xs text-ink-3 hover:text-ink mb-4 transition-colors"
        >
          &larr; All websites
        </button>
        <h1 className="text-lg font-semibold text-ink">{website.name}</h1>
        <p className="text-xs text-ink-3">{website.id}</p>
      </div>

      {/* General */}
      {subSubPage === 'general' && (
        <section className="bg-surface-1 border border-line rounded-lg p-6 space-y-4">
          <h2 className="text-sm font-semibold text-ink">General</h2>
          <div className="space-y-4">
            <label className="block">
              <span className="text-xs text-ink-2 mb-1 block">Name</span>
              <input
                value={name}
                onChange={(e) => setName(e.target.value)}
                className="w-full bg-canvas border border-line rounded-md px-3 py-2 text-sm text-ink focus:outline-none focus:ring-2 focus:ring-spark focus:border-spark"
              />
            </label>
            <label className="block">
              <span className="text-xs text-ink-2 mb-1 block">Domain</span>
              <input
                value={domain}
                onChange={(e) => setDomain(e.target.value)}
                className="w-full bg-canvas border border-line rounded-md px-3 py-2 text-sm text-ink focus:outline-none focus:ring-2 focus:ring-spark focus:border-spark"
              />
            </label>
            <label className="block">
              <span className="text-xs text-ink-2 mb-1 block">Timezone</span>
              <select
                value={timezone}
                onChange={(e) => setTimezone(e.target.value)}
                className="w-full bg-canvas border border-line rounded-md px-3 py-2 text-sm text-ink focus:outline-none focus:ring-2 focus:ring-spark focus:border-spark"
              >
                {Object.entries(TIMEZONE_GROUPS).map(([group, zones]) => (
                  <optgroup key={group} label={group}>
                    {zones.map((tz) => (
                      <option key={tz} value={tz}>{tz}</option>
                    ))}
                  </optgroup>
                ))}
              </select>
            </label>
          </div>
          <Button size="sm" onClick={handleSave} disabled={updateWebsite.isPending} className="text-xs">
            {updateWebsite.isPending && <Loader2 className="w-3 h-3 mr-1 animate-spin" />}
            Save changes
          </Button>
        </section>
      )}

      {/* Tracking snippet */}
      {subSubPage === 'snippet' && (
        <section className="bg-surface-1 border border-line rounded-lg p-6">
          <h2 className="text-sm font-semibold text-ink mb-4">Tracking snippet</h2>
          <TrackingSnippet websiteId={websiteId} domain={website.domain} />
        </section>
      )}

      {subSubPage === 'ingestion' && (
        <section className="bg-surface-1 border border-line rounded-lg p-6 space-y-4">
          <h2 className="text-sm font-semibold text-ink">Ingestion limits</h2>
          <p className="text-xs text-ink-3">
            Configure per-website peak ingest and queue cap. Peak is enforced as a 60-second rolling bucket derived from events/second.
          </p>
          {ingestLimitsQuery.isError && (
            <p className="text-xs text-down">
              Failed to load current ingestion limits. Refresh and retry before saving.
            </p>
          )}
          <label className="flex items-center gap-2 text-xs text-ink-2">
            <input
              type="checkbox"
              checked={customPeak}
              onChange={(e) => setCustomPeak(e.target.checked)}
            />
            Use custom peak limit
          </label>
          <label className="block">
            <span className="text-xs text-ink-2 mb-1 block">Peak events per second</span>
            <input
              type="number"
              min={1}
              disabled={!customPeak}
              value={peakEventsPerSec}
              onChange={(e) => setPeakEventsPerSec(e.target.value)}
              className="w-full bg-canvas border border-line rounded-md px-3 py-2 text-sm text-ink focus:outline-none focus:ring-2 focus:ring-spark focus:border-spark disabled:opacity-60"
            />
          </label>

          <label className="flex items-center gap-2 text-xs text-ink-2">
            <input
              type="checkbox"
              checked={customQueue}
              onChange={(e) => setCustomQueue(e.target.checked)}
            />
            Use custom queue cap
          </label>
          <label className="block">
            <span className="text-xs text-ink-2 mb-1 block">Queue max events</span>
            <input
              type="number"
              min={1}
              disabled={!customQueue}
              value={queueMaxEvents}
              onChange={(e) => setQueueMaxEvents(e.target.value)}
              className="w-full bg-canvas border border-line rounded-md px-3 py-2 text-sm text-ink focus:outline-none focus:ring-2 focus:ring-spark focus:border-spark disabled:opacity-60"
            />
          </label>

          <Button
            size="sm"
            onClick={handleSaveIngestion}
            disabled={
              updateIngestLimits.isPending ||
              ingestLimitsQuery.isLoading ||
              ingestLimitsQuery.isError
            }
            className="text-xs"
          >
            {updateIngestLimits.isPending && <Loader2 className="w-3 h-3 mr-1 animate-spin" />}
            Save ingestion limits
          </Button>
        </section>
      )}

      {/* Sharing */}
      {subSubPage === 'sharing' && (
        <section className="bg-surface-1 border border-line rounded-lg p-6">
          <h2 className="text-sm font-semibold text-ink mb-4">Sharing</h2>
          <SharingToggle websiteId={websiteId} shareId={website.share_id ?? null} />
        </section>
      )}

      {/* API Keys (self-hosted cookie auth modes only) */}
      {(showApiKeysSection && subSubPage === 'keys') && (
        <section className="bg-surface-1 border border-line rounded-lg p-6">
          <h2 className="text-sm font-semibold text-ink mb-4">API keys</h2>
          <ApiKeysSection />
        </section>
      )}

      {/* Change password (local auth only) */}
      {(showPasswordSection && subSubPage === 'security') && (
        <section className="bg-surface-1 border border-line rounded-lg p-6">
          <h2 className="text-sm font-semibold text-ink mb-4">Change password</h2>
          <ChangePasswordSection />
        </section>
      )}

      {subSubPage === 'notifications' && (
        <section className="bg-surface-1 border border-line rounded-lg p-6">
          <NotificationsSettingsPage websiteId={websiteId} />
        </section>
      )}

      {subSubPage === 'bots' && (
        <section className="bg-surface-1 border border-line rounded-lg p-6">
          <BotsSettingsPage websiteId={websiteId} />
        </section>
      )}

      {/* Danger zone */}
      {subSubPage === 'danger' && (
        <section className="bg-surface-1 border border-down/20 rounded-lg p-6">
          <h2 className="text-sm font-semibold text-down mb-2">Danger zone</h2>
          <p className="text-xs text-ink-3 mb-4">
            Deleting a website permanently removes all analytics data. This cannot be undone.
          </p>
          <Button
            variant="outline"
            size="sm"
            onClick={() => setShowDeleteConfirm(true)}
            className="text-xs border-down/30 text-down hover:bg-down/10"
          >
            <Trash2 className="w-3 h-3 mr-1" />
            Delete website
          </Button>
        </section>
      )}

      <ConfirmDialog
        open={showDeleteConfirm}
        onOpenChange={setShowDeleteConfirm}
        title={`Delete "${website.name}"?`}
        description="This permanently removes all analytics data for this website. This cannot be undone."
        confirmLabel="Delete website"
        onConfirm={handleDelete}
        destructive
        loading={deleteWebsite.isPending}
      />
    </div>
  );
}
