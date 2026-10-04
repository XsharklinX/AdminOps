// Piezas comunes de las secciones de Ajustes.
import { createContext, useContext } from "react";
import { type Settings } from "../../lib/api";

export type SettingsProps = { s: Settings; set: (patch: Partial<Settings>) => void };

/** Ajuste al que ha llevado el buscador (se resalta un momento). */
export const HighlightCtx = createContext<string | null>(null);

export function Row({
  title,
  sub,
  children,
}: {
  title: string;
  sub?: string;
  children: React.ReactNode;
}) {
  const highlighted = useContext(HighlightCtx) === title;
  return (
    <div
      data-setting={title}
      className={`flex flex-wrap items-center gap-4 border-t border-line/60 py-3 first:border-t-0 first:pt-0 ${
        highlighted
          ? "-mx-2 rounded-lg bg-neon/10 px-2 ring-1 ring-neon/40"
          : ""
      }`}
    >
      <div className="min-w-0 flex-1">
        <div className="text-sm text-ink">{title}</div>
        {sub && <div className="text-xs text-mute">{sub}</div>}
      </div>
      <div className="shrink-0">{children}</div>
    </div>
  );
}

export const selectClass =
  "rounded-md border border-line bg-void/60 px-3 py-1.5 text-sm text-ink outline-none focus:border-neon/50";
