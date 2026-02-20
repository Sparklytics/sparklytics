'use client';

import { useState } from 'react';
import { Loader2 } from 'lucide-react';
import { Button } from '@/components/ui/button';
import { api } from '@/lib/api';
import { useToast } from '@/hooks/use-toast';

export function ChangePasswordSection() {
  const { toast } = useToast();
  const [currentPassword, setCurrentPassword] = useState('');
  const [newPassword, setNewPassword] = useState('');
  const [saving, setSaving] = useState(false);

  const [confirmPassword, setConfirmPassword] = useState('');

  async function handleSubmit(e: React.FormEvent) {
    e.preventDefault();
    if (!currentPassword || !newPassword) return;
    if (newPassword !== confirmPassword) {
      toast({ title: 'Passwords do not match', variant: 'destructive' });
      return;
    }
    setSaving(true);
    try {
      await api.changePassword(currentPassword, newPassword);
      toast({ title: 'Password updated' });
      setCurrentPassword('');
      setNewPassword('');
      setConfirmPassword('');
    } catch (err) {
      toast({
        title: 'Failed to change password',
        description: err instanceof Error ? err.message : 'Unknown error',
        variant: 'destructive',
      });
    } finally {
      setSaving(false);
    }
  }

  return (
    <form onSubmit={handleSubmit} className="space-y-4">
      <label className="block">
        <span className="text-xs text-ink-2 mb-1 block">Current password</span>
        <input
          type="password"
          value={currentPassword}
          onChange={(e) => setCurrentPassword(e.target.value)}
          className="w-full bg-canvas border border-line rounded-md px-3 py-2 text-sm text-ink focus:outline-none focus:border-spark"
        />
      </label>
      <label className="block">
        <span className="text-xs text-ink-2 mb-1 block">New password</span>
        <input
          type="password"
          value={newPassword}
          onChange={(e) => setNewPassword(e.target.value)}
          className="w-full bg-canvas border border-line rounded-md px-3 py-2 text-sm text-ink focus:outline-none focus:border-spark"
        />
      </label>
      <label className="block">
        <span className="text-xs text-ink-2 mb-1 block">Confirm new password</span>
        <input
          type="password"
          value={confirmPassword}
          onChange={(e) => setConfirmPassword(e.target.value)}
          className="w-full bg-canvas border border-line rounded-md px-3 py-2 text-sm text-ink focus:outline-none focus:border-spark"
        />
      </label>
      <Button type="submit" size="sm" disabled={saving || !currentPassword || !newPassword || !confirmPassword} className="text-xs">
        {saving && <Loader2 className="w-3 h-3 mr-1 animate-spin" />}
        Change password
      </Button>
    </form>
  );
}
