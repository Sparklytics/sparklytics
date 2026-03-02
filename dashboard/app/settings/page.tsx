'use client';

import { useEffect, useState } from 'react';
import { AppShell } from '@/components/layout/AppShell';
import { Button } from '@/components/ui/button';
import { AdminLimitsPanel } from '@/components/settings/AdminLimitsPanel';
import { CreateWebsiteDialog } from '@/components/settings/CreateWebsiteDialog';
import { useWebsites } from '@/hooks/useWebsites';
import { useAuth } from '@/hooks/useAuth';
import { IS_CLOUD } from '@/lib/config';



function navigate(path: string) {
  window.history.pushState({}, '', path);
  window.dispatchEvent(new PopStateEvent('popstate'));
}

export default function SettingsPage() {
  const { data: authStatus, isSuccess: authLoaded } = useAuth();
  const { data, isLoading } = useWebsites();
  const [showCreate, setShowCreate] = useState(false);

  useEffect(() => {
    if (!authLoaded) return;
    if (authStatus === null) return;
    if (authStatus.setup_required) { window.location.href = '/setup'; return; }
    if (!authStatus.authenticated) { window.location.href = '/login'; }
  }, [authStatus, authLoaded]);



  const websites = data?.data ?? [];

  return (
    <AppShell websiteId=''>
      <div className="max-w-3xl space-y-8">
        <section className="space-y-6">
          <div className="flex items-center justify-between">
            <h1 className="text-lg font-semibold text-ink">Websites</h1>
            <Button size="sm" onClick={() => setShowCreate(true)} className="gap-2 text-xs">
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
              <Button size="sm" onClick={() => setShowCreate(true)}>Add your first website</Button>
            </div>
          ) : (
            <div className="space-y-2">
              {websites.map((site) => (
                <button
                  key={site.id}
                  onClick={() => navigate(`/dashboard/${site.id}/settings/general`)}
                  className="flex items-center justify-between w-full bg-surface-1 border border-line rounded-lg px-4 py-3 hover:border-line-3 transition-colors text-left"
                >
                  <div>
                    <p className="text-sm font-medium text-ink">{site.name}</p>
                    <p className="text-xs text-ink-3">{site.domain}</p>
                  </div>
                  <span className="text-xs text-ink-4">{site.timezone}</span>
                </button>
              ))}
            </div>
          )}
        </section>

        {IS_CLOUD && <AdminLimitsPanel />}
      </div>

      <CreateWebsiteDialog open={showCreate} onClose={() => setShowCreate(false)} />
    </AppShell>
  );
}
