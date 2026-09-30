import type { ReactNode } from "react";
import { PageActiveContext, usePageActive } from "../lib/pageActive";
import { PageHelp } from "./PageHelp";

export interface PageTab<T extends string> {
  id: T;
  label: string;
  icon?: ReactNode;
  /** Qué hace esta pestaña (el «?» de la derecha, junto a la tira). */
  help?: string;
}

/** Pestañas dentro de una página que agrupa varias vistas (Actualizaciones, Mi red…). */
export function PageTabs<T extends string>({ tabs, value, onChange }: { tabs: PageTab<T>[]; value: T; onChange: (t: T) => void }) {
  return (
    <div className="no-scrollbar mx-auto flex w-full max-w-6xl shrink-0 gap-1 overflow-x-auto border-b border-line px-6 pt-4">
      {tabs.map((t) => (
        <button
          key={t.id}
          onClick={() => onChange(t.id)}
          title={t.help}
          className={`-mb-px flex shrink-0 items-center gap-1.5 border-b-2 px-3.5 py-2 text-sm whitespace-nowrap transition-colors ${
            value === t.id ? "border-neon font-medium text-ink" : "border-transparent text-dim hover:text-ink"
          }`}
        >
          {t.icon}
          {t.label}
        </button>
      ))}
      {/* Qué hace la pestaña en la que estás. */}
      {tabs.find((t) => t.id === value)?.help && (
        <span className="ml-auto flex shrink-0 items-center pb-1.5">
          <PageHelp text={tabs.find((t) => t.id === value)!.help!} />
        </span>
      )}
    </div>
  );
}

/**
 * Mantiene montadas las pestañas ya visitadas (conservan sus datos al cambiar),
 * igual que las páginas de la app.
 */
export function TabPanels<T extends string>({ value, visited, render }: { value: T; visited: T[]; render: (t: T) => ReactNode }) {
  // La página puede estar visible y la pestaña no: lo que la pestaña tenga en
  // marcha (temperaturas, métricas, vistas web) debe pararse igual que al
  // cambiar de página. Por eso cada pestaña recibe su propio «estoy a la vista».
  const pageActive = usePageActive();
  return (
    <>
      {visited.map((t) => (
        // Ocupa el alto que queda: una web incrustada (portal) necesita su área completa.
        <div key={t} hidden={t !== value} className="min-h-0 flex-1 overflow-y-auto">
          <PageActiveContext.Provider value={pageActive && t === value}>{render(t)}</PageActiveContext.Provider>
        </div>
      ))}
    </>
  );
}
