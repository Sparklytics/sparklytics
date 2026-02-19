'use client';

import { useEffect, useState } from 'react';
import { QueryClient, QueryClientProvider } from '@tanstack/react-query';
import { Toaster } from '@/components/ui/toaster';
import { IS_CLOUD, CLERK_PUBLISHABLE_KEY } from '@/lib/config';
import { setTokenGetter } from '@/lib/api';

// Lazily imported so the bundle only includes Clerk in cloud builds.
let ClerkProvider: React.ComponentType<{ publishableKey: string; children: React.ReactNode }> | null = null;
let useAuth: (() => { getToken: () => Promise<string | null> }) | null = null;

if (IS_CLOUD) {
  // Dynamic require so tree-shaking removes Clerk from self-hosted builds.
  const clerk = require('@clerk/nextjs'); // eslint-disable-line
  ClerkProvider = clerk.ClerkProvider;
  useAuth = clerk.useAuth;
}

/** Registers Clerk's getToken with lib/api.ts so all requests carry the JWT. */
function ClerkTokenSync() {
  const auth = useAuth!();
  useEffect(() => {
    setTokenGetter(() => auth.getToken());
  }, [auth]);
  return null;
}

function QueryWrapper({ children }: { children: React.ReactNode }) {
  const [queryClient] = useState(
    () =>
      new QueryClient({
        defaultOptions: {
          queries: {
            retry: 1,
            refetchOnWindowFocus: false,
          },
        },
      })
  );

  return (
    <QueryClientProvider client={queryClient}>
      {IS_CLOUD && <ClerkTokenSync />}
      {children}
      <Toaster />
    </QueryClientProvider>
  );
}

export function Providers({ children }: { children: React.ReactNode }) {
  if (IS_CLOUD && ClerkProvider) {
    return (
      <ClerkProvider publishableKey={CLERK_PUBLISHABLE_KEY}>
        <QueryWrapper>{children}</QueryWrapper>
      </ClerkProvider>
    );
  }

  return <QueryWrapper>{children}</QueryWrapper>;
}
