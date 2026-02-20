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
        <div className="text-center">
          <h2 className="text-base font-medium text-ink mb-2">Share link not found</h2>
          <p className="text-sm text-ink-3">This link may have been disabled or never existed.</p>
        </div>
      </div>
    );
  }

  return <ShareDashboard shareId={shareId} />;
}
