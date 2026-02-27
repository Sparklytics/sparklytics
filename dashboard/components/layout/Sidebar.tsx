'use client';

import {
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
  Bell,
  Users,
  Target,
  Filter,
  Route,
  RefreshCw,
  LineChart,
  DollarSign,
  Link2,
  Image,
  Bot,
  X,
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
  onNavigate?: () => void;
}

export function Sidebar({ websiteId, currentPath, onAddWebsite, onNavigate }: SidebarProps) {
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
    onNavigate?.();
  }

  const analyticsItems = [
    { label: 'Overview', path: `/dashboard/${websiteId}`, icon: LayoutDashboard },
    { label: 'Pages', path: `/dashboard/${websiteId}/pages`, icon: FileText },
    { label: 'Geolocation', path: `/dashboard/${websiteId}/geolocation`, icon: Globe },
    { label: 'Systems', path: `/dashboard/${websiteId}/systems`, icon: Monitor },
    { label: 'Events', path: `/dashboard/${websiteId}/events`, icon: Zap },
    { label: 'Sessions', path: `/dashboard/${websiteId}/sessions`, icon: Users },
  ];

  const liveItems = [
    { label: 'Realtime', path: `/dashboard/${websiteId}/realtime`, icon: Clock },
  ];

  const exploreItems = [
    { label: 'Goals', path: `/dashboard/${websiteId}/goals`, icon: Target },
    { label: 'Funnels', path: `/dashboard/${websiteId}/funnels`, icon: Filter },
    { label: 'Journey', path: `/dashboard/${websiteId}/journey`, icon: Route },
    { label: 'Retention', path: `/dashboard/${websiteId}/retention`, icon: RefreshCw },
    { label: 'Attribution', path: `/dashboard/${websiteId}/attribution`, icon: DollarSign },
    { label: 'Reports', path: `/dashboard/${websiteId}/reports`, icon: LineChart },
  ];
  const acquisitionItems = [
    { label: 'Campaign Links', path: `/dashboard/${websiteId}/acquisition/links`, icon: Link2 },
    { label: 'Tracking Pixels', path: `/dashboard/${websiteId}/acquisition/pixels`, icon: Image },
  ];

  const configItems: { label: string; path: string; icon: any }[] = [
    { label: 'General', path: `/dashboard/${websiteId}/settings/general`, icon: Settings },
    { label: 'Snippet', path: `/dashboard/${websiteId}/settings/snippet`, icon: Code },
    { label: 'Sharing', path: `/dashboard/${websiteId}/settings/sharing`, icon: Share2 },
    { label: 'Notifications', path: `/dashboard/${websiteId}/settings/notifications`, icon: Bell },
    { label: 'Bots', path: `/dashboard/${websiteId}/settings/bots`, icon: Bot },
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

  const isActive = (path: string) => {
    if (path === `/dashboard/${websiteId}`) {
      return currentPath === path || currentPath === `${path}/`;
    }
    if (path === `/dashboard/${websiteId}/settings/general`) {
      return currentPath === path || currentPath === `/dashboard/${websiteId}/settings`;
    }
    return currentPath.startsWith(path);
  };

  const NavItem = ({ label, path, icon: Icon }: { label: string; path: string; icon: any }) => (
    <button
      onClick={() => navigate(path)}
      className={cn(
        'group flex items-center gap-2 w-full pl-[10px] pr-3 py-2 text-[13px] rounded-r-md border-l-2 transition-all duration-100 text-left',
        isActive(path)
          ? 'border-spark text-ink'
          : 'border-transparent text-ink-3 hover:text-ink-2 hover:bg-ink/[0.04]'
      )}
    >
      <Icon
        className={cn(
          'w-5 h-5 shrink-0 transition-colors',
          isActive(path) ? 'text-spark' : 'text-ink-4 group-hover:text-ink-3'
        )}
      />
      {label}
    </button>
  );

  const SectionLabel = ({ label }: { label: string }) => (
    <div className="px-3 pt-4 pb-1">
      <span className="text-[10px] font-semibold text-ink-4 uppercase tracking-[0.08em]">
        {label}
      </span>
    </div>
  );

  return (
    <div className="flex h-full bg-canvas border-r border-line">
      <aside className="w-[260px] md:w-[220px] flex flex-col min-h-0">

        {/* ── Brand header ─────────────────────────── */}
        <div className="px-3 pt-4 pb-3 border-b border-line shrink-0">
          <div className="flex items-center gap-2 mb-3">
            {/* App mark */}
            <div className="w-[22px] h-[22px] rounded bg-spark flex items-center justify-center shrink-0">
              <span className="text-[9px] font-bold text-black leading-none tracking-tight">sp</span>
            </div>
            <span className="text-[13px] font-semibold text-ink tracking-tight">sparklytics</span>
            {/* Mobile close */}
            <button
              onClick={onNavigate}
              className="ml-auto md:hidden p-1 text-ink-3 hover:text-ink hover:bg-surface-1 rounded transition-colors"
              aria-label="Close menu"
            >
              <X className="w-3.5 h-3.5" />
            </button>
          </div>
          <WebsitePicker
            websites={websites}
            currentId={websiteId}
            onAddWebsite={onAddWebsite}
          />
        </div>

        {/* ── Nav ──────────────────────────────────── */}
        {websiteId ? (
          <nav className="flex-1 overflow-y-auto px-2 py-1 min-h-0">
            <SectionLabel label="Analytics" />
            <div className="space-y-px">
              {analyticsItems.map((item) => <NavItem key={item.label} {...item} />)}
            </div>

            <SectionLabel label="Live" />
            <div className="space-y-px">
              {liveItems.map((item) => <NavItem key={item.label} {...item} />)}
            </div>

            <SectionLabel label="Explore" />
            <div className="space-y-px">
              {exploreItems.map((item) => <NavItem key={item.label} {...item} />)}
            </div>

            <SectionLabel label="Acquisition" />
            <div className="space-y-px">
              {acquisitionItems.map((item) => <NavItem key={item.label} {...item} />)}
            </div>

            <div className="mt-3 pt-3 border-t border-line/50">
              <SectionLabel label="Settings" />
              <div className="space-y-px">
                {configItems.map((item) => <NavItem key={item.label} {...item} />)}
              </div>
            </div>
          </nav>
        ) : (
          <div className="flex-1 flex items-center justify-center px-4">
            <span className="text-[13px] text-ink-3 text-center">Select a website</span>
          </div>
        )}

        {/* ── Bottom: usage + logout ───────────────── */}
        <div className="shrink-0 border-t border-line px-2 py-2">
          <UsageBadge />
          <button
            onClick={handleLogout}
            className="mt-1 group flex items-center gap-2 w-full px-3 py-2 text-[13px] text-ink-3 hover:text-ink-2 hover:bg-ink/[0.04] rounded-md transition-colors"
          >
            <LogOut className="w-5 h-5 shrink-0 text-ink-4 group-hover:text-ink-3" />
            Log out
          </button>
        </div>

      </aside>
    </div>
  );
}
