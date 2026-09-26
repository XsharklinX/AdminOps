import { useEffect, useState } from "react";
import { AdminBanner } from "./components/AdminBanner";
import { ToastProvider } from "./components/feedback";
import { NAV, Sidebar, type PageId } from "./components/Sidebar";
import { api, systemApi, type TargetUser } from "./lib/api";
import { Bloatware } from "./pages/Bloatware";
import { Diagnostics } from "./pages/Diagnostics";
import { Report } from "./pages/Report";
import { Dashboard } from "./pages/Dashboard";
import { History } from "./pages/History";
import { Placeholder } from "./pages/Placeholder";
import { Startup } from "./pages/Startup";
import { TweaksPage } from "./pages/TweaksPage";

/** Páginas que son una lista de ajustes del catálogo, por categoría. */
const TWEAK_PAGES: Partial<Record<PageId, string>> = {
  privacy: "privacy",
  performance: "performance",
  repair: "repair",
  services: "services",
  cleanup: "cleanup",
};

export default function App() {
  const [page, setPage] = useState<PageId>("dashboard");
  const [isAdmin, setIsAdmin] = useState<boolean | null>(null);
  const [targetUser, setTargetUser] = useState<TargetUser | null>(null);

  useEffect(() => {
    api.isAdmin().then(setIsAdmin).catch(() => setIsAdmin(false));
    systemApi.targetUser().then(setTargetUser).catch(() => {});
  }, []);

  const nav = NAV.find((n) => n.id === page)!;
  const category = TWEAK_PAGES[page];

  let content;
  if (page === "dashboard") content = <Dashboard />;
  else if (page === "history") content = <History isAdmin={!!isAdmin} />;
  else if (page === "bloatware") content = <Bloatware isAdmin={!!isAdmin} />;
  else if (page === "startup") content = <Startup isAdmin={!!isAdmin} />;
  else if (page === "diagnostics") content = <Diagnostics />;
  else if (page === "report") content = <Report />;
  else if (category) content = <TweaksPage key={category} category={category} isAdmin={!!isAdmin} />;
  else content = <Placeholder label={nav.label} icon={nav.icon} phase={nav.phase} />;

  return (
    <ToastProvider>
      <div className="flex h-full">
        <Sidebar active={page} onSelect={setPage} isAdmin={isAdmin} targetUser={targetUser} />
        <main className="flex min-w-0 flex-1 flex-col">
          {isAdmin === false && <AdminBanner />}
          <header className="flex items-center justify-between border-b border-line px-6 py-4">
            <h1 className="text-xl font-semibold tracking-tight">{nav.label}</h1>
            {page === "dashboard" && <span className="font-mono text-[11px] text-mute">en vivo · 1.5 s</span>}
          </header>
          <div className="flex-1 overflow-y-auto">{content}</div>
        </main>
      </div>
    </ToastProvider>
  );
}
