'use client';

import { useEffect, useState } from 'react';
import { useRouter } from 'next/navigation';
import { useQueryClient } from '@tanstack/react-query';
import { Loader2 } from 'lucide-react';
import { api } from '@/lib/api';
import { AUTH_QUERY_KEY } from '@/hooks/useAuth';

export default function ForcePasswordPage() {
  const router = useRouter();
  const queryClient = useQueryClient();
  const [currentPassword, setCurrentPassword] = useState('');
  const [newPassword, setNewPassword] = useState('');
  const [confirmPassword, setConfirmPassword] = useState('');
  const [error, setError] = useState('');
  const [loading, setLoading] = useState(false);
  const [ready, setReady] = useState(false);

  useEffect(() => {
    let cancelled = false;

    (async () => {
      try {
        const status = await api.getAuthStatus();
        if (cancelled) return;

        if (status === null) {
          router.replace('/dashboard');
          return;
        }

        if (status.setup_required) {
          router.replace('/setup');
          return;
        }

        if (!status.authenticated) {
          router.replace('/login');
          return;
        }

        if (!status.password_change_required) {
          router.replace('/dashboard');
          return;
        }
      } catch {
        router.replace('/login');
        return;
      }

      if (!cancelled) {
        setReady(true);
      }
    })();

    return () => {
      cancelled = true;
    };
  }, [router]);

  async function handleSubmit(e: React.FormEvent) {
    e.preventDefault();
    setError('');

    if (!currentPassword || !newPassword) {
      setError('Both passwords are required');
      return;
    }

    if (newPassword !== confirmPassword) {
      setError('Passwords do not match');
      return;
    }

    setLoading(true);
    try {
      await api.changePassword(currentPassword, newPassword);
      await queryClient.removeQueries({ queryKey: AUTH_QUERY_KEY, exact: true });
      router.replace('/login');
    } catch (err: unknown) {
      setError(err instanceof Error ? err.message : 'Password update failed');
    } finally {
      setLoading(false);
    }
  }

  if (!ready) {
    return (
      <div className="min-h-screen bg-canvas flex items-center justify-center">
        <div className="flex items-center gap-2 text-sm text-ink-3">
          <Loader2 className="w-4 h-4 animate-spin" />
          Checking security status...
        </div>
      </div>
    );
  }

  return (
    <div className="min-h-screen bg-canvas flex items-center justify-center">
      <div className="w-full max-w-sm px-6">
        <div className="mb-8 text-center">
          <span className="text-lg font-semibold tracking-tight text-ink">
            spark<span className="text-spark">lytics</span>
          </span>
        </div>

        <div className="bg-surface-1 border border-line rounded-lg p-6">
          <h1 className="text-sm font-medium text-ink mb-1">Change password before continuing</h1>
          <p className="text-xs text-ink-3 mb-3">
            This instance was initialized with the default bootstrap password.
          </p>
          <p className="text-xs text-ink-4 mb-6">
            Set a new admin password now. After saving, you will sign in again with the new one.
          </p>

          <form onSubmit={handleSubmit} className="space-y-4">
            <div>
              <label htmlFor="current-password" className="block text-xs text-ink-2 mb-2">
                Current password
              </label>
              <input
                id="current-password"
                type="password"
                value={currentPassword}
                onChange={(e) => setCurrentPassword(e.target.value)}
                autoFocus
                required
                className="w-full bg-surface-input border border-line rounded-md px-3 py-2 text-sm text-ink placeholder-ink-4 focus:outline-none focus:border-line-3 focus:ring-2 focus:ring-spark focus:ring-offset-2 focus:ring-offset-surface-1 transition-colors"
                placeholder="Enter your current admin password"
              />
            </div>

            <div>
              <label htmlFor="new-password" className="block text-xs text-ink-2 mb-2">
                New password
              </label>
              <input
                id="new-password"
                type="password"
                value={newPassword}
                onChange={(e) => setNewPassword(e.target.value)}
                required
                className="w-full bg-surface-input border border-line rounded-md px-3 py-2 text-sm text-ink placeholder-ink-4 focus:outline-none focus:border-line-3 focus:ring-2 focus:ring-spark focus:ring-offset-2 focus:ring-offset-surface-1 transition-colors"
                placeholder="Choose a strong new password"
              />
            </div>

            <div>
              <label htmlFor="confirm-password" className="block text-xs text-ink-2 mb-2">
                Confirm new password
              </label>
              <input
                id="confirm-password"
                type="password"
                value={confirmPassword}
                onChange={(e) => setConfirmPassword(e.target.value)}
                required
                className="w-full bg-surface-input border border-line rounded-md px-3 py-2 text-sm text-ink placeholder-ink-4 focus:outline-none focus:border-line-3 focus:ring-2 focus:ring-spark focus:ring-offset-2 focus:ring-offset-surface-1 transition-colors"
                placeholder="Repeat the new password"
              />
            </div>

            {error && <p className="text-xs text-down">{error}</p>}

            <button
              type="submit"
              disabled={loading}
              className="w-full bg-spark hover:bg-spark-dim text-canvas font-medium text-sm py-2 rounded transition-colors disabled:opacity-50 disabled:cursor-not-allowed flex items-center justify-center gap-2"
            >
              {loading && <Loader2 className="w-4 h-4 animate-spin" />}
              Update password
            </button>
          </form>
        </div>
      </div>
    </div>
  );
}
