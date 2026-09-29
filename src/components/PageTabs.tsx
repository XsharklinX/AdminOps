import type { ReactNode } from "react";

export interface PageTab<T extends string> {
  id: T;
  label: string;
  icon?: ReactNode;
}

/** Pestañas dentro de una página que agrupa varias vistas (Actualizaciones, Mi red…). */
export function PageTabs<T extends string>({ tabs, value, onChange }: { tabs: PageTab<T>[]; value: T; onChange: (t: T) => void }) {
  return (
    <div className="mx-auto flex w-full max-w-6xl shrink-0 gap-1 border-b border-line px-6 pt-4">
      {tabs.map((t) => (
        <button
          key={t.id}
          onClick={() => onChange(t.id)}
          className={`-mb-px flex items-center gap-1.5 border-b-2 px-3.5 py-2 text-sm transition-colors ${
            value === t.id ? "border-neon font-medium text-ink" : "border-transparent text-dim hover:text-ink"
          }`}
        >
          {t.icon}
          {t.label}
        </button>
      ))}
    </div>
  );
}

/**
 * Mantiene montadas las pestañas ya visitadas (conservan sus datos al cambiar),
 * igual que las páginas de la app.
 */
export function TabPanels<T extends string>({ value, visited, render }: { value: T; visited: T[]; render: (t: T) => ReactNode }) {
  return (
    <>
      {visited.map((t) => (
        // Ocupa el alto que queda: una web incrustada (portal) necesita su área completa.
        <div key={t} hidden={t !== value} className="min-h-0 flex-1 overflow-y-auto">
          {render(t)}
        </div>
      ))}
    </>
  );
}
