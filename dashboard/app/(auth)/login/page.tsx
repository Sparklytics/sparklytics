'use client';

import { useEffect, useState } from 'react';
import { useRouter } from 'next/navigation';
import { Loader2 } from 'lucide-react';
import { api } from '@/lib/api';

export default function LoginPage() {
  const router = useRouter();
  const [password, setPassword] = useState('');
  const [error, setError] = useState('');
  const [loading, setLoading] = useState(false);
  const [ready, setReady] = useState(false);

  useEffect(() => {
    let cancelled = false;

    (async () => {
      try {
        const status = await api.getAuthStatus();
        if (cancelled) return;

        // SPARKLYTICS_AUTH=none: no login needed.
        if (status === null) {
          router.replace('/dashboard');
          return;
        }

        // Local mode before setup should go to setup flow.
        if (status.setup_required) {
          router.replace('/setup');
          return;
        }

        // Already authenticated.
        if (status.authenticated) {
          router.replace('/dashboard');
          return;
        }
      } catch {
        // If status check fails, keep login available.
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
    setLoading(true);
    try {
      await api.login(password);
      router.push('/dashboard');
    } catch (err: unknown) {
      setError(err instanceof Error ? err.message : 'Login failed');
    } finally {
      setLoading(false);
    }
  }

  if (!ready) {
    return (
      <div className="min-h-screen bg-canvas flex items-center justify-center">
        <div className="flex items-center gap-2 text-sm text-ink-3">
          <Loader2 className="w-4 h-4 animate-spin" />
          Checking auth status...
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
          <h1 className="text-sm font-medium text-ink mb-6">Sign in</h1>

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
                className="w-full bg-surface-input border border-line rounded-md px-3 py-2 text-sm text-ink placeholder-ink-4 focus:outline-none focus:border-line-3 focus:ring-2 focus:ring-spark focus:ring-offset-2 focus:ring-offset-surface-1 transition-colors"
                placeholder="Enter your password"
              />
            </div>

            {error && (
              <p className="text-xs text-down">{error}</p>
            )}

            <button
              type="submit"
              disabled={loading}
              className="w-full bg-spark hover:bg-spark-dim text-canvas font-medium text-sm py-2 rounded transition-colors disabled:opacity-50 disabled:cursor-not-allowed flex items-center justify-center gap-2"
            >
              {loading && <Loader2 className="w-4 h-4 animate-spin" />}
              Sign in
            </button>
          </form>
        </div>
      </div>
    </div>
  );
}
