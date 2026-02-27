'use client';

import { useState } from 'react';
import { Check, Loader2, ArrowRight } from 'lucide-react';
import { Button } from '@/components/ui/button';
import { TrackingSnippet } from '@/components/settings/TrackingSnippet';
import { api } from '@/lib/api';
import { TIMEZONE_GROUPS, getBrowserTimezone } from '@/lib/timezones';

type Step = 'create' | 'snippet' | 'verify';

interface OnboardingWizardProps {
  onComplete: (websiteId: string) => void;
}

const STEPS: { id: Step; label: string }[] = [
  { id: 'create', label: 'Create website' },
  { id: 'snippet', label: 'Install snippet' },
  { id: 'verify', label: 'Verify' },
];

export function OnboardingWizard({ onComplete }: OnboardingWizardProps) {
  const [step, setStep] = useState<Step>('create');
  const [name, setName] = useState('');
  const [domain, setDomain] = useState('');
  const [timezone, setTimezone] = useState(getBrowserTimezone());
  const [creating, setCreating] = useState(false);
  const [websiteId, setWebsiteId] = useState('');
  const [verifying, setVerifying] = useState(false);
  const [verified, setVerified] = useState(false);
  const [error, setError] = useState('');

  const stepIndex = STEPS.findIndex((s) => s.id === step);

  async function handleCreate() {
    if (!name.trim() || !domain.trim()) {
      setError('Name and domain are required');
      return;
    }
    setCreating(true);
    setError('');
    try {
      const result = await api.createWebsite({ name: name.trim(), domain: domain.trim(), timezone });
      setWebsiteId(result.data.id);
      setStep('snippet');
    } catch (e: unknown) {
      setError(e instanceof Error ? e.message : 'Failed to create website');
    } finally {
      setCreating(false);
    }
  }

  async function handleVerify() {
    if (!websiteId) return;
    setVerifying(true);
    try {
      // Poll stats — if pageviews > 0 the snippet is working.
      const today = new Date().toISOString().slice(0, 10);
      const result = await api.getStats(websiteId, { start_date: today, end_date: today });
      if (result.data.pageviews > 0) {
        setVerified(true);
      } else {
        setError('No events received yet. Make sure the snippet is installed, then try again.');
      }
    } catch {
      setError('Verification check failed — please try again.');
    } finally {
      setVerifying(false);
    }
  }

  return (
    <div className="max-w-lg mx-auto">
      {/* Stepper */}
      <div className="flex items-center gap-0 mb-8">
        {STEPS.map((s, i) => (
          <div key={s.id} className="flex items-center flex-1 last:flex-none">
            <div className="flex items-center gap-2">
              <div
                className={`w-6 h-6 rounded-full flex items-center justify-center text-xs font-medium border
                  ${i < stepIndex ? 'bg-spark border-spark text-canvas' : i === stepIndex ? 'border-spark text-spark' : 'border-line text-ink-3'}`}
              >
                {i < stepIndex ? <Check className="w-3 h-3" /> : i + 1}
              </div>
              <span className={`text-xs ${i === stepIndex ? 'text-ink' : 'text-ink-3'}`}>{s.label}</span>
            </div>
            {i < STEPS.length - 1 && (
              <div className={`flex-1 h-px mx-3 ${i < stepIndex ? 'bg-spark' : 'bg-line'}`} />
            )}
          </div>
        ))}
      </div>

      {/* Step: Create */}
      {step === 'create' && (
        <div className="bg-surface-1 border border-line rounded-lg p-6 space-y-4">
          <h2 className="text-base font-semibold text-ink">Add your website</h2>
          <div className="space-y-3">
            <label className="block">
              <span className="text-xs text-ink-2 mb-1 block">Website name</span>
              <input
                value={name}
                onChange={(e) => setName(e.target.value)}
                placeholder="My Blog"
                className="w-full bg-canvas border border-line rounded-md px-3 py-2 text-sm text-ink focus:outline-none focus:ring-2 focus:ring-spark focus:border-spark"
              />
            </label>
            <label className="block">
              <span className="text-xs text-ink-2 mb-1 block">Domain</span>
              <input
                value={domain}
                onChange={(e) => setDomain(e.target.value)}
                placeholder="example.com"
                className="w-full bg-canvas border border-line rounded-md px-3 py-2 text-sm text-ink focus:outline-none focus:ring-2 focus:ring-spark focus:border-spark"
              />
            </label>
            <label className="block">
              <span className="text-xs text-ink-2 mb-1 block">Timezone</span>
              <select
                value={timezone}
                onChange={(e) => setTimezone(e.target.value)}
                className="w-full bg-canvas border border-line rounded-md px-3 py-2 text-sm text-ink focus:outline-none focus:ring-2 focus:ring-spark focus:border-spark"
              >
                {Object.entries(TIMEZONE_GROUPS).map(([group, zones]) => (
                  <optgroup key={group} label={group}>
                    {zones.map((tz) => (
                      <option key={tz} value={tz}>{tz}</option>
                    ))}
                  </optgroup>
                ))}
              </select>
            </label>
          </div>
          {error && <p className="text-xs text-down">{error}</p>}
          <Button onClick={handleCreate} disabled={creating} className="w-full gap-2">
            {creating ? <Loader2 className="w-4 h-4 animate-spin" /> : <ArrowRight className="w-4 h-4" />}
            Create website
          </Button>
        </div>
      )}

      {/* Step: Snippet */}
      {step === 'snippet' && websiteId && (
        <div className="bg-surface-1 border border-line rounded-lg p-6 space-y-4">
          <h2 className="text-base font-semibold text-ink">Install the tracking snippet</h2>
          <p className="text-xs text-ink-3">
            Copy the snippet below and paste it inside the{' '}
            <code className="text-ink-2">&lt;head&gt;</code> tag of every page you want to track.
          </p>
          <TrackingSnippet websiteId={websiteId} />
          <Button onClick={() => { setStep('verify'); setError(''); }} className="w-full gap-2">
            <ArrowRight className="w-4 h-4" />
            Done, verify installation
          </Button>
        </div>
      )}

      {/* Step: Verify */}
      {step === 'verify' && websiteId && (
        <div className="bg-surface-1 border border-line rounded-lg p-6 space-y-4">
          <h2 className="text-base font-semibold text-ink">Verify installation</h2>
          <p className="text-xs text-ink-3">
            Visit your website to generate a test pageview, then click the button below to check
            if Sparklytics received it.
          </p>
          {verified ? (
            <div className="flex items-center gap-2 text-spark text-sm">
              <Check className="w-4 h-4" />
              Tracking is working! Redirecting to your dashboard…
            </div>
          ) : (
            <>
              {error && <p className="text-xs text-down">{error}</p>}
              <Button
                onClick={handleVerify}
                disabled={verifying}
                className="w-full gap-2"
              >
                {verifying ? <Loader2 className="w-4 h-4 animate-spin" /> : <Check className="w-4 h-4" />}
                Check for pageviews
              </Button>
            </>
          )}
          {verified && (
            <Button
              onClick={() => onComplete(websiteId)}
              className="w-full"
            >
              Go to dashboard
            </Button>
          )}
        </div>
      )}
    </div>
  );
}
