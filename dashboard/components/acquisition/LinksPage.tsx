'use client';

import { useState } from 'react';
import { Copy, Link2, Trash2 } from 'lucide-react';
import { Button } from '@/components/ui/button';
import { CampaignLink } from '@/lib/api';
import {
  useCampaignLinks,
  useCreateCampaignLink,
  useDeleteCampaignLink,
  useUpdateCampaignLink,
} from '@/hooks/useCampaignLinks';
import { CreateLinkDialog } from './CreateLinkDialog';
import { EditLinkDialog } from './EditLinkDialog';

interface LinksPageProps {
  websiteId: string;
}

function SummaryCards({ links }: { links: CampaignLink[] }) {
  const totalLinks = links.length;
  const totalClicks = links.reduce((acc, l) => acc + (l.clicks ?? 0), 0);
  const totalConversions = links.reduce((acc, l) => acc + (l.conversions ?? 0), 0);
  const totalRevenue = links.reduce((acc, l) => acc + (l.revenue ?? 0), 0);

  const cards = [
    { label: 'Total Links', value: totalLinks.toLocaleString() },
    { label: 'Total Clicks', value: totalClicks.toLocaleString() },
    { label: 'Total Conversions', value: totalConversions.toLocaleString() },
    { label: 'Total Revenue', value: `$${totalRevenue.toFixed(2)}` },
  ];

  return (
    <div className="grid grid-cols-2 gap-4 sm:grid-cols-4">
      {cards.map((card) => (
        <div key={card.label} className="border border-line rounded-lg bg-surface-1 p-4">
          <p className="text-[11px] text-ink-3 uppercase tracking-[0.07em] font-medium">
            {card.label}
          </p>
          <p className="mt-1 text-2xl font-mono tabular-nums text-ink">{card.value}</p>
        </div>
      ))}
    </div>
  );
}

function SummaryCardsSkeleton() {
  return (
    <div className="grid grid-cols-2 gap-4 sm:grid-cols-4">
      {[0, 1, 2, 3].map((i) => (
        <div key={i} className="border border-line rounded-lg bg-surface-1 p-4 animate-pulse">
          <div className="h-3 w-24 bg-surface-2 rounded mb-3" />
          <div className="h-7 w-16 bg-surface-2 rounded" />
        </div>
      ))}
    </div>
  );
}

function TableSkeletonRows() {
  return (
    <>
      {[0, 1, 2].map((i) => (
        <tr key={i} className="border-t border-line animate-pulse">
          <td className="px-3 py-3">
            <div className="h-3.5 w-32 bg-surface-2 rounded mb-1.5" />
            <div className="h-3 w-48 bg-surface-2 rounded" />
          </td>
          <td className="px-3 py-3">
            <div className="h-3.5 w-20 bg-surface-2 rounded" />
          </td>
          <td className="px-3 py-3">
            <div className="h-3.5 w-56 bg-surface-2 rounded" />
          </td>
          <td className="px-3 py-3">
            <div className="h-5 w-14 bg-surface-2 rounded-sm" />
          </td>
          <td className="px-3 py-3">
            <div className="flex items-center gap-1">
              <div className="h-7 w-14 bg-surface-2 rounded" />
              <div className="h-7 w-10 bg-surface-2 rounded" />
              <div className="h-7 w-14 bg-surface-2 rounded" />
              <div className="h-7 w-7 bg-surface-2 rounded" />
            </div>
          </td>
        </tr>
      ))}
    </>
  );
}

function EmptyState() {
  return (
    <tr>
      <td colSpan={5}>
        <div className="flex flex-col items-center justify-center py-12 px-4 text-center">
          <div className="flex items-center justify-center w-12 h-12 rounded-lg border border-line bg-surface-2 mb-4">
            <Link2 className="w-5 h-5 text-ink-3" />
          </div>
          <p className="text-sm font-medium text-ink">No campaign links yet</p>
          <p className="text-xs text-ink-3 mt-1 max-w-xs">
            Campaign links let you track traffic from email campaigns, QR codes, and any channel
            where JavaScript tracking is unavailable. Create your first link above to get started.
          </p>
          <p className="text-[11px] text-ink-3 uppercase tracking-[0.07em] font-medium mt-4">
            Use the &ldquo;Create Link&rdquo; button above to add your first link
          </p>
        </div>
      </td>
    </tr>
  );
}

function LinkRow({
  link,
  onDelete,
  onToggle,
  onEdit,
}: {
  link: CampaignLink;
  onDelete: (id: string) => void;
  onToggle: (id: string, active: boolean) => void;
  onEdit: (link: CampaignLink) => void;
}) {
  return (
    <tr className="border-t border-line align-top">
      <td className="px-3 py-2">
        <p className="text-sm text-ink font-medium">{link.name}</p>
        <p className="text-xs text-ink-3 mt-0.5">{link.destination_url}</p>
      </td>
      <td className="px-3 py-2">
        <code className="text-xs text-ink bg-surface-2 px-1.5 py-0.5 rounded">{link.slug}</code>
      </td>
      <td className="px-3 py-2">
        <div className="text-xs text-ink-3">
          <span className="font-medium text-ink">{link.clicks ?? 0}</span> clicks
          {' · '}
          <span className="font-medium text-ink">{link.unique_visitors ?? 0}</span> visitors
          {' · '}
          <span className="font-medium text-ink">{link.conversions ?? 0}</span> conversions
          {' · '}
          <span className="font-medium text-ink">${(link.revenue ?? 0).toFixed(2)}</span> revenue
        </div>
      </td>
      <td className="px-3 py-2">
        <span
          className={`text-xs px-2 py-1 rounded-sm border ${
            link.is_active ? 'border-spark text-spark' : 'border-line text-ink-3'
          }`}
        >
          {link.is_active ? 'Active' : 'Inactive'}
        </span>
      </td>
      <td className="px-3 py-2">
        <div className="flex items-center gap-1">
          <Button
            type="button"
            size="sm"
            variant="outline"
            className="h-7 px-2 text-xs"
            onClick={() => navigator.clipboard.writeText(link.tracking_url)}
          >
            <Copy className="w-3 h-3 mr-1" />
            Copy
          </Button>
          <Button
            type="button"
            size="sm"
            variant="outline"
            className="h-7 px-2 text-xs"
            onClick={() => onEdit(link)}
          >
            Edit
          </Button>
          <Button
            type="button"
            size="sm"
            variant="outline"
            className="h-7 px-2 text-xs"
            onClick={() => onToggle(link.id, !link.is_active)}
          >
            {link.is_active ? 'Disable' : 'Enable'}
          </Button>
          <Button
            type="button"
            size="sm"
            variant="outline"
            className="h-7 px-2 text-xs text-down"
            onClick={() => onDelete(link.id)}
          >
            <Trash2 className="w-3 h-3" />
          </Button>
        </div>
      </td>
    </tr>
  );
}

export function LinksPage({ websiteId }: LinksPageProps) {
  const { data, isLoading } = useCampaignLinks(websiteId);
  const createLink = useCreateCampaignLink(websiteId);
  const updateLink = useUpdateCampaignLink(websiteId);
  const deleteLink = useDeleteCampaignLink(websiteId);
  const links = data?.data ?? [];
  const [editingLink, setEditingLink] = useState<CampaignLink | null>(null);

  return (
    <div className="space-y-4">
      <div>
        <h2 className="text-lg font-semibold text-ink">Campaign Links</h2>
        <p className="text-xs text-ink-3 mt-0.5">
          Generate redirect links for channels where JavaScript tracking is unavailable.
        </p>
      </div>

      {isLoading ? <SummaryCardsSkeleton /> : <SummaryCards links={links} />}

      <CreateLinkDialog
        isPending={createLink.isPending}
        onCreate={(payload) => createLink.mutate(payload)}
      />

      <div className="border border-line rounded-lg bg-surface-1 overflow-hidden">
        <table className="w-full text-left">
          <thead className="bg-surface-2">
            <tr className="text-xs text-ink-3">
              <th className="px-3 py-2 font-medium">Link</th>
              <th className="px-3 py-2 font-medium">Slug</th>
              <th className="px-3 py-2 font-medium">Stats</th>
              <th className="px-3 py-2 font-medium">Status</th>
              <th className="px-3 py-2 font-medium">Actions</th>
            </tr>
          </thead>
          <tbody>
            {isLoading ? (
              <TableSkeletonRows />
            ) : links.length === 0 ? (
              <EmptyState />
            ) : (
              links.map((link) => (
                <LinkRow
                  key={link.id}
                  link={link}
                  onDelete={(id) => deleteLink.mutate(id)}
                  onEdit={setEditingLink}
                  onToggle={(id, active) =>
                    updateLink.mutate({ linkId: id, payload: { is_active: active } })
                  }
                />
              ))
            )}
          </tbody>
        </table>
      </div>

      <EditLinkDialog
        link={editingLink}
        isPending={updateLink.isPending}
        onSave={(linkId, payload) => {
          updateLink.mutate({ linkId, payload }, { onSuccess: () => setEditingLink(null) });
        }}
        onClose={() => setEditingLink(null)}
      />
    </div>
  );
}
