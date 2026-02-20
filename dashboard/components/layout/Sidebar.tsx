'use client';

import {
  BarChart2,
  Clock,
  Settings,
  LogOut,
  FileText,
  Globe,
  Monitor,
  Zap,
  LayoutDashboard,
  Code,
  Share2,
  Key,
  Shield,
  AlertTriangle,
  Users,
  Target
} from 'lucide-react';
import { cn } from '@/lib/utils';
import { WebsitePicker } from './WebsitePicker';
import { UsageBadge } from './UsageBadge';
import { useWebsites } from '@/hooks/useWebsites';
import { useAuth } from '@/hooks/useAuth';
import { api } from '@/lib/api';

interface SidebarProps {
  websiteId: string;
  currentPath: string;
  onAddWebsite?: () => void;
}

export function Sidebar({ websiteId, currentPath, onAddWebsite }: SidebarProps) {
  const { data } = useWebsites();
  const { data: authStatus } = useAuth();
  const websites = data?.data ?? [];
  const showApiKeys = authStatus?.mode === 'password' || authStatus?.mode === 'local';
  const showSecurity = authStatus?.mode === 'local';

  async function handleLogout() {
    await api.logout();
    window.location.href = '/login';
  }

  function navigate(path: string) {
    window.history.pushState({}, '', path);
    window.dispatchEvent(new PopStateEvent('popstate'));
  }

  const analyticsItems = [
    { label: 'Overview', path: `/dashboard/${websiteId}`, icon: LayoutDashboard },
    { label: 'Pages', path: `/dashboard/${websiteId}/pages`, icon: FileText },
    { label: 'Geolocation', path: `/dashboard/${websiteId}/geolocation`, icon: Globe },
    { label: 'Systems', path: `/dashboard/${websiteId}/systems`, icon: Monitor },
    { label: 'Events',   path: `/dashboard/${websiteId}/events`,   icon: Zap },
    { label: 'Sessions', path: `/dashboard/${websiteId}/sessions`, icon: Users },
  ];

  const liveItems = [
    { label: 'Realtime', path: `/dashboard/${websiteId}/realtime`, icon: Clock },
  ];

  const exploreItems = [
    { label: 'Goals', path: `/dashboard/${websiteId}/goals`, icon: Target },
  ];

  const configItems = [
    { label: 'General', path: `/dashboard/${websiteId}/settings/general`, icon: Settings },
    { label: 'Snippet', path: `/dashboard/${websiteId}/settings/snippet`, icon: Code },
    { label: 'Sharing', path: `/dashboard/${websiteId}/settings/sharing`, icon: Share2 },
    { label: 'Danger Zone', path: `/dashboard/${websiteId}/settings/danger`, icon: AlertTriangle },
  ];
  if (showApiKeys) {
    configItems.splice(3, 0, {
      label: 'API Keys',
      path: `/dashboard/${websiteId}/settings/keys`,
      icon: Key,
    });
  }
  if (showSecurity) {
    const insertIndex = showApiKeys ? 4 : 3;
    configItems.splice(insertIndex, 0, {
      label: 'Security',
      path: `/dashboard/${websiteId}/settings/security`,
      icon: Shield,
    });
  }

  // Helper to determine exact or partial path match
  const isActive = (path: string) => {
    if (path === `/dashboard/${websiteId}`) {
      return currentPath === path || currentPath === `${path}/`;
    }
    if (path === `/dashboard/${websiteId}/settings/general`) {
      return currentPath === path || currentPath === `/dashboard/${websiteId}/settings`;
    }
    return currentPath.startsWith(path);
  };

  const NavItem = ({ label, path, icon: Icon }: { label: string, path: string, icon: any }) => (
    <button
      key={label}
      onClick={() => navigate(path)}
      className={cn(
        'group flex items-center gap-2 w-full px-2 py-1.5 rounded-md text-sm transition-colors duration-100 text-left',
        isActive(path)
          ? 'text-ink bg-surface-1 font-medium'
          : 'text-ink-3 hover:text-ink hover:bg-surface-1/50'
      )}
    >
      <Icon className={cn("w-4 h-4", isActive(path) ? "text-spark" : "text-ink-3 group-hover:text-ink-2")} />
      {label}
    </button>
  );

  return (
    <div className="flex h-screen sticky top-0 border-r border-line bg-canvas z-20">
      {/* Tier 1 - Global Context */}
      <div className="w-[64px] shrink-0 border-r border-line flex flex-col items-center py-4 bg-surface-1/30">
        <button
          onClick={() => navigate('/settings')}
          className="w-10 h-10 flex items-center justify-center rounded-xl bg-ink text-canvas font-bold text-lg mb-4 hover:opacity-90 transition-opacity shadow-sm"
          title="All Websites"
        >
          s<span className="text-canvas/70 font-medium text-sm">p</span>
        </button>

        <div className="flex-1" />
        <button
          onClick={handleLogout}
          className="p-2 text-ink-3 hover:text-ink hover:bg-surface-1 rounded-md transition-colors"
          title="Log out"
        >
          <LogOut className="w-5 h-5" />
        </button>
      </div>

      {/* Tier 2 - Local Context */}
      <aside className="w-[200px] shrink-0 flex flex-col bg-canvas">
        {websiteId ? (
          <>
            <div className="px-3 py-3 border-b border-line flex items-center justify-center min-h-[64px]">
              <div className="w-full">
                <WebsitePicker websites={websites} currentId={websiteId} onAddWebsite={onAddWebsite} />
              </div>
            </div>

            <nav className="flex-1 overflow-y-auto px-3 py-4 space-y-6">
              <div>
                <h3 className="px-2 text-[11px] font-semibold text-ink-3 uppercase tracking-wider mb-2">Analytics</h3>
                <div className="space-y-0.5">
                  {analyticsItems.map(NavItem)}
                </div>
              </div>

              <div>
                <h3 className="px-2 text-[11px] font-semibold text-ink-3 uppercase tracking-wider mb-2">Live</h3>
                <div className="space-y-0.5">
                  {liveItems.map(NavItem)}
                </div>
              </div>

              <div>
                <h3 className="px-2 text-[11px] font-semibold text-ink-3 uppercase tracking-wider mb-2">Explore</h3>
                <div className="space-y-0.5">
                  {exploreItems.map(NavItem)}
                </div>
              </div>

              <div>
                <h3 className="px-2 text-[11px] font-semibold text-ink-3 uppercase tracking-wider mb-2">Configuration</h3>
                <div className="space-y-0.5">
                  {configItems.map(NavItem)}
                </div>
              </div>
            </nav>

            <UsageBadge />
          </>
        ) : (
          <div className="flex-1 flex px-4 items-center justify-center text-center">
            <span className="text-sm text-ink-3">Select a website</span>
          </div>
        )}
      </aside>
    </div>
  );
}
