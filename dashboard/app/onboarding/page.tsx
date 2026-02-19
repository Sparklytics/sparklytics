'use client';

import { OnboardingWizard } from '@/components/onboarding/OnboardingWizard';

export default function OnboardingPage() {
  function handleComplete(websiteId: string) {
    window.location.href = `/dashboard/${websiteId}`;
  }

  return (
    <div className="min-h-screen bg-canvas flex items-center justify-center px-4">
      <div className="w-full max-w-lg">
        <div className="text-center mb-8">
          <h1 className="text-xl font-semibold text-ink">
            Welcome to <span className="text-spark">spark</span>lytics
          </h1>
          <p className="text-sm text-ink-3 mt-1">Set up your first website to get started.</p>
        </div>
        <OnboardingWizard onComplete={handleComplete} />
      </div>
    </div>
  );
}
