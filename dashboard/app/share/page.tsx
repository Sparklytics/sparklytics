'use client';

import { useEffect, useState } from 'react';
import { ShareDashboard } from '@/components/share/ShareDashboard';

function useShareIdFromUrl(): string {
  const [shareId, setShareId] = useState('');
  useEffect(() => {
    function read() {
      // URL pattern: /share/<shareId>
      const segs = window.location.pathname.split('/').filter(Boolean);
      setShareId(segs[1] ?? '');
    }
    read();
    window.addEventListener('popstate', read);
    return () => window.removeEventListener('popstate', read);
  }, []);
  return shareId;
}

export default function SharePage() {
  const shareId = useShareIdFromUrl();

  if (!shareId) {
    return (
      <div className="min-h-screen bg-canvas flex items-center justify-center">
        <p className="text-sm text-ink-3">Loading…</p>
      </div>
    );
  }

  return <ShareDashboard shareId={shareId} />;
}
