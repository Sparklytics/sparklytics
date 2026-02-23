'use client';

import { Globe, Plus, ChevronDown } from 'lucide-react';
import { useState, useRef, useEffect } from 'react';
import { cn } from '@/lib/utils';
import type { Website } from '@/lib/api';

interface WebsitePickerProps {
  websites: Website[];
  currentId: string;
  onAddWebsite?: () => void;
}

export function WebsitePicker({ websites, currentId, onAddWebsite }: WebsitePickerProps) {
  const [open, setOpen] = useState(false);
  const ref = useRef<HTMLDivElement>(null);
  const current = websites.find((w) => w.id === currentId);

  useEffect(() => {
    function handleClickOutside(e: MouseEvent) {
      if (ref.current && !ref.current.contains(e.target as Node)) {
        setOpen(false);
      }
    }
    document.addEventListener('mousedown', handleClickOutside);
    return () => document.removeEventListener('mousedown', handleClickOutside);
  }, []);

  function selectSite(id: string) {
    window.history.pushState({}, '', `/dashboard/${id}`);
    window.dispatchEvent(new PopStateEvent('popstate'));
    setOpen(false);
  }

  if (websites.length === 0) {
    return (
      <button
        onClick={onAddWebsite}
        className="flex items-center gap-2 w-full px-3 py-2 text-sm text-ink-2 hover:text-ink transition-colors duration-150 rounded"
      >
        <Plus className="w-4 h-4 text-spark" />
        <span>Add your first website</span>
      </button>
    );
  }

  return (
    <div ref={ref} className="relative">
      <button
        onClick={() => setOpen(!open)}
        className="flex items-center gap-2 w-full px-2.5 py-1.5 rounded-md hover:bg-white/[0.04] transition-colors duration-100"
      >
        <Globe className="w-3.5 h-3.5 text-ink-4 shrink-0" />
        <span className="flex-1 text-left text-[13px] text-ink-2 truncate">
          {current?.name ?? 'Select website'}
        </span>
        <ChevronDown
          className={cn(
            'w-3.5 h-3.5 text-ink-4 transition-transform duration-150',
            open && 'rotate-180'
          )}
        />
      </button>

      {open && (
        <div className="absolute top-full left-0 right-0 mt-1 bg-surface-2 border border-line rounded-lg z-50 overflow-hidden shadow-lg">
          {websites.map((site) => (
            <button
              key={site.id}
              onClick={() => selectSite(site.id)}
              className={cn(
                'flex items-center gap-2 w-full px-3 py-2 text-[13px] hover:bg-surface-1 transition-colors duration-100',
                site.id === currentId ? 'text-ink' : 'text-ink-3'
              )}
            >
              <span className="truncate">{site.name}</span>
              {site.id === currentId && (
                <span className="ml-auto text-spark text-[10px]">●</span>
              )}
            </button>
          ))}
        </div>
      )}
    </div>
  );
}
