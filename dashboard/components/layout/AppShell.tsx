'use client';

import { useEffect, useState } from 'react';
import { Sidebar } from './Sidebar';
import { Header } from './Header';
import { CreateWebsiteDialog } from '@/components/settings/CreateWebsiteDialog';

interface AppShellProps {
  websiteId: string;
  children: React.ReactNode;
}

export function AppShell({ websiteId, children }: AppShellProps) {
  const [currentPath, setCurrentPath] = useState('');
  const [showCreate, setShowCreate] = useState(false);
  const [sidebarOpen, setSidebarOpen] = useState(false);

  useEffect(() => {
    setCurrentPath(window.location.pathname);
    function onPop() {
      setCurrentPath(window.location.pathname);
      setSidebarOpen(false);
    }
    window.addEventListener('popstate', onPop);
    return () => window.removeEventListener('popstate', onPop);
  }, []);

  return (
    <div className="flex h-screen overflow-hidden bg-canvas">
      {/* Mobile overlay */}
      {sidebarOpen && (
        <div
          className="fixed inset-0 z-30 bg-black/50 md:hidden"
          onClick={() => setSidebarOpen(false)}
        />
      )}

      {/* Sidebar — always visible on md+ (flex item), slide-in drawer on mobile (fixed) */}
      <div className={`fixed inset-y-0 left-0 md:relative md:inset-auto z-40 md:z-20 flex-shrink-0 transition-transform duration-200 md:translate-x-0 ${sidebarOpen ? 'translate-x-0' : '-translate-x-full'}`}>
        <Sidebar
          websiteId={websiteId}
          currentPath={currentPath}
          onAddWebsite={() => setShowCreate(true)}
          onNavigate={() => setSidebarOpen(false)}
        />
      </div>

      <div className="flex-1 flex flex-col min-w-0 overflow-hidden">
        <Header onMenuClick={() => setSidebarOpen((o) => !o)} />
        <main className="flex-1 overflow-y-auto p-4 md:p-6">
          {children}
        </main>
      </div>
      <CreateWebsiteDialog open={showCreate} onClose={() => setShowCreate(false)} />
    </div>
  );
}
