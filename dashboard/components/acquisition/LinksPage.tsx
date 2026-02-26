'use client';

import { useState } from 'react';
import { Copy, Trash2 } from 'lucide-react';
import { Button } from '@/components/ui/button';
import { CampaignLink } from '@/lib/api';
import {
  useCampaignLinks,
  useCreateCampaignLink,
  useDeleteCampaignLink,
  useUpdateCampaignLink,
} from '@/hooks/useCampaignLinks';
import { CreateLinkDialog } from './CreateLinkDialog';

interface LinksPageProps {
  websiteId: string;
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
        <span className={`text-xs px-2 py-1 rounded border ${link.is_active ? 'border-spark text-spark' : 'border-line text-ink-3'}`}>
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
            className="h-7 px-2 text-xs text-red-400"
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

  function onEdit(link: CampaignLink) {
    const name = window.prompt('Link name', link.name);
    if (name === null || !name.trim()) return;

    const destination = window.prompt('Destination URL', link.destination_url);
    if (destination === null || !destination.trim()) return;

    const utmSource = window.prompt('UTM source (optional)', link.utm_source ?? '');
    if (utmSource === null) return;
    const utmMedium = window.prompt('UTM medium (optional)', link.utm_medium ?? '');
    if (utmMedium === null) return;
    const utmCampaign = window.prompt('UTM campaign (optional)', link.utm_campaign ?? '');
    if (utmCampaign === null) return;

    updateLink.mutate({
      linkId: link.id,
      payload: {
        name: name.trim(),
        destination_url: destination.trim(),
        utm_source: utmSource.trim() || null,
        utm_medium: utmMedium.trim() || null,
        utm_campaign: utmCampaign.trim() || null,
      },
    });
  }

  return (
    <div className="space-y-4">
      <div>
        <h2 className="text-lg font-semibold text-ink">Campaign Links</h2>
        <p className="text-xs text-ink-3 mt-0.5">
          Generate redirect links for channels where JavaScript tracking is unavailable.
        </p>
      </div>

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
              <tr>
                <td className="px-3 py-6 text-sm text-ink-3" colSpan={5}>
                  Loading campaign links...
                </td>
              </tr>
            ) : links.length === 0 ? (
              <tr>
                <td className="px-3 py-6 text-sm text-ink-3" colSpan={5}>
                  No campaign links created yet.
                </td>
              </tr>
            ) : (
              links.map((link) => (
                <LinkRow
                  key={link.id}
                  link={link}
                  onDelete={(id) => deleteLink.mutate(id)}
                  onEdit={onEdit}
                  onToggle={(id, active) => updateLink.mutate({ linkId: id, payload: { is_active: active } })}
                />
              ))
            )}
          </tbody>
        </table>
      </div>
    </div>
  );
}
