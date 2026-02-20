'use client';

import { useState } from 'react';
import { Download, Loader2 } from 'lucide-react';
import { Button } from '@/components/ui/button';
import { useExport } from '@/hooks/useExport';
import { useToast } from '@/hooks/use-toast';

interface ExportButtonProps {
  websiteId: string;
  startDate: string;
  endDate: string;
}

export function ExportButton({ websiteId, startDate, endDate }: ExportButtonProps) {
  const [loading, setLoading] = useState(false);
  const { triggerExport } = useExport(websiteId);
  const { toast } = useToast();

  function handleExport() {
    setLoading(true);
    try {
      triggerExport(startDate, endDate);
    } catch {
      toast({ title: 'Export failed', description: 'Could not generate CSV export.', variant: 'destructive' });
    } finally {
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
