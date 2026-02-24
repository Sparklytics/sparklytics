'use client';

import { useState } from 'react';
import { Plus } from 'lucide-react';
import { Button } from '@/components/ui/button';
import { FunnelCard } from './FunnelCard';
import { FunnelBuilderDialog } from './FunnelBuilderDialog';
import { useFunnels } from '@/hooks/useFunnels';

interface FunnelsPageProps {
  websiteId: string;
}

export function FunnelsPage({ websiteId }: FunnelsPageProps) {
  const { data, isLoading } = useFunnels(websiteId);
  const [createOpen, setCreateOpen] = useState(false);

  const funnels = data?.data ?? [];

  return (
    <div className="space-y-4">
      {/* Page header */}
      <div className="flex items-center justify-between">
        <div>
          <h2 className="text-lg font-semibold text-ink">Funnels</h2>
          <p className="text-xs text-ink-3 mt-1">
            Track multi-step conversion flows through your site.
          </p>
        </div>
        <Button
          size="sm"
          onClick={() => setCreateOpen(true)}
          className="text-xs gap-1"
        >
          <Plus className="w-4 h-4" />
          New Funnel
        </Button>
      </div>

      {/* Content */}
      {isLoading ? (
        <div className="grid grid-cols-1 md:grid-cols-2 gap-4">
          {[1, 2].map((i) => (
            <div key={i} className="h-16 bg-surface-1 border border-line rounded-lg animate-pulse" />
          ))}
        </div>
      ) : funnels.length === 0 ? (
        <div className="border border-line rounded-lg bg-surface-1 px-6 py-16 text-center">
          <p className="text-sm font-medium text-ink">No funnels yet</p>
          <p className="text-xs text-ink-3 mt-1">
            Create a funnel to track conversions across multiple steps.
          </p>
        </div>
      ) : (
        <div className="grid grid-cols-1 md:grid-cols-2 gap-4">
          {funnels.map((funnel) => (
            <FunnelCard key={funnel.id} websiteId={websiteId} funnel={funnel} />
          ))}
        </div>
      )}

      <FunnelBuilderDialog
        websiteId={websiteId}
        open={createOpen}
        onClose={() => setCreateOpen(false)}
      />
    </div>
  );
}
