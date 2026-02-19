'use client';

import { useState } from 'react';
import { Download, Loader2 } from 'lucide-react';
import { Button } from '@/components/ui/button';
import { useExport } from '@/hooks/useExport';

interface ExportButtonProps {
  websiteId: string;
  startDate: string;
  endDate: string;
}

export function ExportButton({ websiteId, startDate, endDate }: ExportButtonProps) {
  const [loading, setLoading] = useState(false);
  const { triggerExport } = useExport(websiteId);

  function handleExport() {
    setLoading(true);
    try {
      triggerExport(startDate, endDate);
    } finally {
      // Give browser a moment to start the download before removing loading state.
      setTimeout(() => setLoading(false), 1000);
    }
  }

  return (
    <Button
      variant="outline"
      size="sm"
      onClick={handleExport}
      disabled={loading || !websiteId}
      className="gap-2 text-xs"
    >
      {loading ? (
        <Loader2 className="w-4 h-4 animate-spin" />
      ) : (
        <Download className="w-4 h-4" />
      )}
      Export CSV
    </Button>
  );
}
