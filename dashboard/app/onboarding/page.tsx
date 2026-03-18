'use client';

import { useEffect } from 'react';
import { AppShell } from '@/components/layout/AppShell';
import { OnboardingWizard } from '@/components/onboarding/OnboardingWizard';
import { useAuth } from '@/hooks/useAuth';

export default function OnboardingPage() {
  const { data: authStatus, isSuccess: authLoaded } = useAuth();

  useEffect(() => {
    if (!authLoaded) return;
    if (authStatus === null) return;
    if (authStatus.setup_required) {
      window.location.href = '/setup';
      return;
    }
    if (authStatus.password_change_required) {
      window.location.href = '/force-password';
      return;
    }
    if (!authStatus.authenticated) {
      window.location.href = '/login';
    }
  }, [authLoaded, authStatus]);

  function handleComplete(websiteId: string) {
    window.location.href = `/dashboard/${websiteId}`;
  }

  return (
    <AppShell websiteId="">
      <div className="w-full max-w-lg mx-auto mt-24">
        <div className="text-center mb-8">
          <h1 className="text-xl font-semibold text-ink">
            Welcome to <span className="text-spark">spark</span>lytics
          </h1>
          <p className="text-sm text-ink-3 mt-1">Set up your first website to get started.</p>
        </div>
        <OnboardingWizard onComplete={handleComplete} />
      </div>
    </AppShell>
  );
}
