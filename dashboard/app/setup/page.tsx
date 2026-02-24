'use client';

import { useEffect, useState } from 'react';
import { useRouter } from 'next/navigation';
import { Loader2, Check } from 'lucide-react';
import { api } from '@/lib/api';

const MIN_PASSWORD_LENGTH = 12;

export default function SetupPage() {
  const router = useRouter();
  const [password, setPassword] = useState('');
  const [confirm, setConfirm] = useState('');
  const [error, setError] = useState('');
  const [loading, setLoading] = useState(false);
  const [ready, setReady] = useState(false);

  useEffect(() => {
    let cancelled = false;

    (async () => {
      try {
        const status = await api.getAuthStatus();
        if (cancelled) return;

        // SPARKLYTICS_AUTH=none: setup should not be shown.
        if (status === null) {
          router.replace('/dashboard');
          return;
        }

        // Setup only applies to local mode before first setup.
        if (status.mode !== 'local' || !status.setup_required) {
          router.replace(status.authenticated ? '/dashboard' : '/login');
          return;
        }
      } catch {
        // If status check fails, still allow user to attempt setup.
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
    if (password !== confirm) {
      setError('Passwords do not match');
      return;
    }
    if (password.trim().length === 0) {
      setError('Password cannot be empty or whitespace-only');
      return;
    }
    if (password.length < MIN_PASSWORD_LENGTH) {
      setError(`Password must be at least ${MIN_PASSWORD_LENGTH} characters`);
      return;
    }
    setLoading(true);
    try {
      await api.setup(password);
      router.push('/dashboard');
    } catch (err: unknown) {
      setError(err instanceof Error ? err.message : 'Setup failed');
    } finally {
      setLoading(false);
    }
  }

  const checks = [
    { label: `At least ${MIN_PASSWORD_LENGTH} characters`, met: password.length >= MIN_PASSWORD_LENGTH },
    { label: 'Not whitespace-only', met: password.trim().length > 0 },
    { label: 'Passwords match', met: password.length > 0 && password === confirm },
  ];

  if (!ready) {
    return (
      <div className="min-h-screen bg-canvas flex items-center justify-center">
        <div className="flex items-center gap-2 text-sm text-ink-3">
          <Loader2 className="w-4 h-4 animate-spin" />
          Checking setup status...
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
          <h1 className="text-sm font-medium text-ink mb-1">Set up your instance</h1>
          <p className="text-xs text-ink-3 mb-6">Create an admin password to protect your analytics.</p>

          <form onSubmit={handleSubmit} className="space-y-4">
            <div>
              <label htmlFor="password" className="block text-xs text-ink-2 mb-2">
                Password
              </label>
              <input
                id="password"
                type="password"
                value={password}
                onChange={(e) => setPassword(e.target.value)}
                required
                autoFocus
                className="w-full bg-surface-input border border-line rounded-md px-3 py-2 text-sm text-ink placeholder-ink-4 focus:outline-none focus:border-line-3 focus:ring-2 focus:ring-spark/20 focus:ring-offset-2 focus:ring-offset-surface-1 transition-colors"
                placeholder="Choose a password"
              />
            </div>

            <div>
              <label htmlFor="confirm" className="block text-xs text-ink-2 mb-2">
                Confirm password
              </label>
              <input
                id="confirm"
                type="password"
                value={confirm}
                onChange={(e) => setConfirm(e.target.value)}
                required
                className="w-full bg-surface-input border border-line rounded-md px-3 py-2 text-sm text-ink placeholder-ink-4 focus:outline-none focus:border-line-3 focus:ring-2 focus:ring-spark/20 focus:ring-offset-2 focus:ring-offset-surface-1 transition-colors"
                placeholder="Repeat password"
              />
            </div>

            {/* Password checks */}
            {password && (
              <div className="space-y-1">
                {checks.map((c) => (
                  <div key={c.label} className="flex items-center gap-2 text-xs">
                    <Check
                      className={`w-3.5 h-3.5 transition-colors ${
                        c.met ? 'text-spark' : 'text-ink-4'
                      }`}
                    />
                    <span className={c.met ? 'text-ink-2' : 'text-ink-4'}>{c.label}</span>
                  </div>
                ))}
              </div>
            )}

            {error && <p className="text-xs text-down">{error}</p>}

            <button
              type="submit"
              disabled={loading}
              className="w-full bg-spark hover:bg-spark-dim text-canvas font-medium text-sm py-2 rounded transition-colors disabled:opacity-50 disabled:cursor-not-allowed flex items-center justify-center gap-2"
            >
              {loading && <Loader2 className="w-4 h-4 animate-spin" />}
              Create account
            </button>
          </form>
        </div>
      </div>
    </div>
  );
}
