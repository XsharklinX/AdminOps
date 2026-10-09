// Piezas comunes de las secciones de Ajustes.
import { CircleHelp } from "lucide-react";
import { createContext, useContext, useState } from "react";
import { ToggleLabelCtx } from "../../components/ui";
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
  // Una línea por ajuste: la explicación larga, solo si se pide (o al llegar desde el buscador).
  const [help, setHelp] = useState(false);
  const showHelp = help || highlighted;
  return (
    <div
      data-setting={title}
      className={`flex flex-wrap items-center gap-x-4 gap-y-1 border-t border-line/60 py-2.5 first:border-t-0 first:pt-0 ${
        highlighted
          ? "-mx-2 rounded-lg bg-neon/10 px-2 ring-1 ring-neon/40"
          : ""
      }`}
    >
      <div className="flex min-w-0 flex-1 items-center gap-1.5">
        <span className="text-sm text-ink">{title}</span>
        {sub && (
          <button
            type="button"
            onClick={() => setHelp((v) => !v)}
            aria-expanded={showHelp}
            aria-label={showHelp ? `Ocultar la explicación de «${title}»` : `Qué hace «${title}»`}
            title={showHelp ? "Ocultar la explicación" : sub}
            className={`shrink-0 rounded p-0.5 transition-colors ${showHelp ? "text-neon" : "text-mute hover:text-ink"}`}
          >
            <CircleHelp size={13} />
          </button>
        )}
      </div>
      <div className="shrink-0">
        <ToggleLabelCtx.Provider value={title}>{children}</ToggleLabelCtx.Provider>
      </div>
      {sub && showHelp && <p className="basis-full text-xs text-mute">{sub}</p>}
    </div>
  );
}

export const selectClass =
  "rounded-md border border-line bg-void/60 px-3 py-1.5 text-sm text-ink outline-none focus:border-neon/50";
