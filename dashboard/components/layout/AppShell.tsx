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

  useEffect(() => {
    setCurrentPath(window.location.pathname);
    function onPop() { setCurrentPath(window.location.pathname); }
    window.addEventListener('popstate', onPop);
    return () => window.removeEventListener('popstate', onPop);
  }, []);

  return (
    <div className="flex min-h-screen bg-canvas">
      <Sidebar websiteId={websiteId} currentPath={currentPath} onAddWebsite={() => setShowCreate(true)} />
      <div className="flex-1 flex flex-col min-w-0">
        <Header />
        <main className="flex-1 p-4 md:p-6">
          {children}
        </main>
      </div>
      <CreateWebsiteDialog open={showCreate} onClose={() => setShowCreate(false)} />
    </div>
  );
}
