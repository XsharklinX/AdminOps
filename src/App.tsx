import { useEffect, useState } from "react";
import { AdminBanner } from "./components/AdminBanner";
import { NAV, Sidebar, type PageId } from "./components/Sidebar";
import { api } from "./lib/api";
import { Dashboard } from "./pages/Dashboard";
import { Placeholder } from "./pages/Placeholder";

export default function App() {
  const [page, setPage] = useState<PageId>("dashboard");
  const [isAdmin, setIsAdmin] = useState<boolean | null>(null);

  useEffect(() => {
    api.isAdmin().then(setIsAdmin).catch(() => setIsAdmin(false));
  }, []);

  const nav = NAV.find((n) => n.id === page)!;

  return (
    <div className="flex h-full">
      <Sidebar active={page} onSelect={setPage} isAdmin={isAdmin} />
      <main className="flex min-w-0 flex-1 flex-col">
        {isAdmin === false && <AdminBanner />}
        <header className="flex items-center justify-between border-b border-line px-6 py-4">
          <h1 className="text-xl font-semibold tracking-tight">{nav.label}</h1>
          <span className="font-mono text-[11px] text-mute">en vivo · 1.5 s</span>
        </header>
        <div className="flex-1 overflow-y-auto">
          {page === "dashboard" ? <Dashboard /> : <Placeholder label={nav.label} icon={nav.icon} phase={nav.phase} />}
        </div>
      </main>
    </div>
  );
}
