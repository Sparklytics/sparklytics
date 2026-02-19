'use client';

import { BarChart2, Clock, Settings, LogOut } from 'lucide-react';
import { cn } from '@/lib/utils';
import { WebsitePicker } from './WebsitePicker';
import { useWebsites } from '@/hooks/useWebsites';
import { api } from '@/lib/api';

interface SidebarProps {
  websiteId: string;
  currentPath: string;
}

export function Sidebar({ websiteId, currentPath }: SidebarProps) {
  const { data } = useWebsites();
  const websites = data?.data ?? [];

  const navItems = [
    { label: 'Analytics', path: `/dashboard/${websiteId}`, icon: BarChart2 },
    { label: 'Realtime', path: `/dashboard/${websiteId}/realtime`, icon: Clock },
    { label: 'Settings', path: `/dashboard/${websiteId}/settings`, icon: Settings },
  ];

  async function handleLogout() {
    await api.logout();
    window.location.href = '/login';
  }

  function navigate(path: string) {
    window.history.pushState({}, '', path);
    window.dispatchEvent(new PopStateEvent('popstate'));
  }

  return (
    <aside className="w-[200px] shrink-0 border-r border-line flex flex-col h-screen sticky top-0">
      {/* Logo */}
      <div className="px-4 py-4 border-b border-line">
        <span className="text-sm font-semibold tracking-tight text-ink">
          spark<span className="text-spark">lytics</span>
        </span>
      </div>

      {/* Website picker */}
      <div className="px-2 py-3 border-b border-line">
        <WebsitePicker websites={websites} currentId={websiteId} />
      </div>

      {/* Nav */}
      <nav className="flex-1 px-2 py-3 space-y-1">
        {navItems.map(({ label, path, icon: Icon }) => {
          const isActive = currentPath === path;
          return (
            <button
              key={label}
              onClick={() => navigate(path)}
              className={cn(
                'flex items-center gap-2 w-full px-3 py-2 rounded-md text-sm transition-colors duration-100 text-left border-l-2',
                isActive
                  ? 'text-ink border-spark'
                  : 'text-ink-2 hover:text-ink hover:bg-surface-1 border-transparent'
              )}
            >
              <Icon className="w-5 h-5" />
              {label}
            </button>
          );
        })}
      </nav>

      {/* Logout */}
      <div className="px-2 py-3 border-t border-line">
        <button
          onClick={handleLogout}
          className="flex items-center gap-2 w-full px-3 py-2 text-sm text-ink-3 hover:text-ink hover:bg-surface-1 rounded transition-colors duration-100"
        >
          <LogOut className="w-5 h-5" />
          Log out
        </button>
      </div>
    </aside>
  );
}
