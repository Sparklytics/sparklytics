'use client';

import { AppShell } from '@/components/layout/AppShell';
import { OnboardingWizard } from '@/components/onboarding/OnboardingWizard';

export default function OnboardingPage() {
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
