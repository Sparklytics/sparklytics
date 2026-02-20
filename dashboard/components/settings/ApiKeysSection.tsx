'use client';

import { useState } from 'react';
import { Key, Loader2, Trash2 } from 'lucide-react';
import { Button } from '@/components/ui/button';
import { useApiKeys, useCreateApiKey, useDeleteApiKey } from '@/hooks/useApiKeys';

export function ApiKeysSection() {
  const { data, isLoading } = useApiKeys();
  const createKey = useCreateApiKey();
  const deleteKey = useDeleteApiKey();
  const [newKeyName, setNewKeyName] = useState('');
  const [createdKey, setCreatedKey] = useState<string | null>(null);

  const keys = data?.data ?? [];

  async function handleCreate(e: React.FormEvent) {
    e.preventDefault();
    if (!newKeyName.trim()) return;
    const result = await createKey.mutateAsync(newKeyName.trim());
    setNewKeyName('');
    if (result?.data?.key) {
      setCreatedKey(result.data.key);
    }
  }

  return (
    <div className="space-y-4">
      {createdKey && (
        <div className="bg-spark/10 border border-spark/30 rounded-lg p-4">
          <p className="text-xs text-ink-2 mb-1">New API key (copy now — it will not be shown again):</p>
          <code className="text-xs font-mono text-spark break-all">{createdKey}</code>
          <button
            onClick={() => {
              navigator.clipboard.writeText(createdKey);
              setCreatedKey(null);
            }}
            className="block mt-2 text-xs text-ink-3 hover:text-ink transition-colors"
          >
            Copy and dismiss
          </button>
        </div>
      )}

      <form onSubmit={handleCreate} className="flex items-end gap-2">
        <label className="flex-1">
          <span className="text-xs text-ink-2 mb-1 block">Key name</span>
          <input
            value={newKeyName}
            onChange={(e) => setNewKeyName(e.target.value)}
            placeholder="e.g. CI pipeline"
            className="w-full bg-canvas border border-line rounded-md px-3 py-2 text-sm text-ink placeholder:text-ink-4 focus:outline-none focus:border-spark"
          />
        </label>
        <Button type="submit" size="sm" disabled={createKey.isPending || !newKeyName.trim()} className="text-xs">
          {createKey.isPending && <Loader2 className="w-3 h-3 mr-1 animate-spin" />}
          Create
        </Button>
      </form>

      {isLoading ? (
        <div className="space-y-2">
          {[1, 2].map((i) => (
            <div key={i} className="h-10 bg-canvas border border-line rounded-md animate-pulse" />
          ))}
        </div>
      ) : keys.length === 0 ? (
        <p className="text-xs text-ink-3">No API keys yet.</p>
      ) : (
        <div className="space-y-2">
          {keys.map((key) => (
            <div
              key={key.id}
              className="flex items-center justify-between bg-canvas border border-line rounded-md px-3 py-2"
            >
              <div className="flex items-center gap-2 min-w-0">
                <Key className="w-4 h-4 text-ink-3 shrink-0" />
                <div className="min-w-0">
                  <p className="text-sm text-ink truncate">{key.name}</p>
                  <p className="text-xs text-ink-4 font-mono">{key.prefix}...</p>
                </div>
              </div>
              <Button
                variant="ghost"
                size="sm"
                onClick={() => deleteKey.mutate(key.id)}
                disabled={deleteKey.isPending}
                className="text-xs text-ink-3 hover:text-down shrink-0"
              >
                <Trash2 className="w-3 h-3" />
              </Button>
            </div>
          ))}
        </div>
      )}
    </div>
  );
}
