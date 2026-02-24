'use client';

import { useState } from 'react';
import { ChevronDown, ChevronUp, Loader2, Pencil, Trash2 } from 'lucide-react';
import { FunnelResultsPanel } from './FunnelResultsPanel';
import { FunnelBuilderDialog } from './FunnelBuilderDialog';
import { FunnelDeleteConfirm } from './FunnelDeleteConfirm';
import { api } from '@/lib/api';
import { useQuery } from '@tanstack/react-query';
import type { FunnelSummary } from '@/lib/api';

interface FunnelCardProps {
  websiteId: string;
  funnel: FunnelSummary;
}

export function FunnelCard({ websiteId, funnel }: FunnelCardProps) {
  const [expanded, setExpanded] = useState(false);
  const [editOpen, setEditOpen] = useState(false);
  const [deleteTarget, setDeleteTarget] = useState<FunnelSummary | null>(null);

  const panelId = `funnel-panel-${funnel.id}`;

  // Fetch the full funnel (with steps) only when the edit dialog is opened.
  // isLoading is true only on the initial fetch (no cached data yet).
  const { data: fullFunnelData, isLoading: isLoadingFunnel } = useQuery({
    queryKey: ['funnel', websiteId, funnel.id],
    queryFn: () => api.getFunnel(websiteId, funnel.id),
    enabled: editOpen && !!funnel.id,
    staleTime: 60_000,
  });

  function toggleExpanded() {
    setExpanded((v) => !v);
  }

  return (
    <>
      <div className="border border-line rounded-lg bg-surface-1 overflow-hidden">
        {/* Card header — expand toggle is a native <button> so keyboard focus works correctly */}
        <div className="flex items-center gap-2 px-4 py-3">
          <button
            type="button"
            className="flex-1 min-w-0 text-left rounded-sm hover:opacity-80 transition-opacity focus-visible:outline-none focus-visible:ring-1 focus-visible:ring-spark"
            onClick={toggleExpanded}
            aria-expanded={expanded}
            aria-controls={panelId}
          >
            <p className="text-sm font-medium text-ink truncate">{funnel.name}</p>
            <p className="text-xs text-ink-3 mt-1">
              {funnel.step_count} step{funnel.step_count !== 1 ? 's' : ''}
            </p>
          </button>

          <div className="flex items-center gap-1 shrink-0">
            {/* Show a spinner while the edit query is loading; gate dialog open on data ready */}
            <button
              type="button"
              onClick={() => setEditOpen(true)}
              disabled={editOpen && isLoadingFunnel}
              className="p-1 text-ink-3 hover:text-ink hover:bg-surface-2 rounded-sm transition-colors disabled:opacity-50"
              aria-label="Edit funnel"
            >
              {editOpen && isLoadingFunnel ? (
                <Loader2 className="w-4 h-4 animate-spin" />
              ) : (
                <Pencil className="w-4 h-4" />
              )}
            </button>
            <button
              type="button"
              onClick={() => setDeleteTarget(funnel)}
              className="p-1 text-ink-3 hover:text-red-400 hover:bg-red-400/10 rounded-sm transition-colors"
              aria-label="Delete funnel"
            >
              <Trash2 className="w-4 h-4" />
            </button>
            {expanded ? (
              <ChevronUp className="w-4 h-4 text-ink-3" aria-hidden />
            ) : (
              <ChevronDown className="w-4 h-4 text-ink-3" aria-hidden />
            )}
          </div>
        </div>

        {/* Expanded results panel */}
        {expanded && (
          <div id={panelId} className="border-t border-line px-4 pb-4">
            <FunnelResultsPanel websiteId={websiteId} funnelId={funnel.id} />
          </div>
        )}
      </div>

      {/* Edit dialog — gated on data being ready to prevent blank-step flicker */}
      <FunnelBuilderDialog
        websiteId={websiteId}
        open={editOpen && !isLoadingFunnel}
        onClose={() => setEditOpen(false)}
        editingFunnel={fullFunnelData?.data ?? null}
      />

      {/* Delete confirm */}
      <FunnelDeleteConfirm
        websiteId={websiteId}
        funnel={deleteTarget}
        onClose={() => setDeleteTarget(null)}
      />
    </>
  );
}
