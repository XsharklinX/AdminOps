import { ExternalLink, Wrench } from "lucide-react";
import { useEffect, useState } from "react";
import { toolboxApi, type ToolView } from "../lib/api";
import { useToast } from "./feedback";
import { Card } from "./ui";

export interface ToolLink {
  /** Id del acceso en el catálogo de Herramientas. */
  id: string;
  /** Qué se hace ahí, en una línea. */
  what: string;
}

/**
 * Accesos directos a las herramientas de Windows que completan una página
 * (se abren con el catálogo de Herramientas: mismos permisos y avisos).
 */
export function WindowsTools({ title = "Más opciones en Windows", links, className = "" }: { title?: string; links: ToolLink[]; className?: string }) {
  const [tools, setTools] = useState<Record<string, ToolView>>({});
  const toast = useToast();
  useEffect(() => {
    toolboxApi
      .list()
      .then((v) => setTools(Object.fromEntries(v.tools.map((t) => [t.id, t]))))
      .catch(() => {});
  }, []);

  return (
    <Card title={title} icon={<Wrench size={14} />} className={className}>
      <div className="grid gap-2 sm:grid-cols-2">
        {links.map((l) => {
          const t = tools[l.id];
          const off = t?.unavailable ?? null;
          return (
            <button
              key={l.id}
              disabled={!!off}
              onClick={() => toolboxApi.launch(l.id).catch((e) => toast("error", String(e)))}
              title={off ?? t?.description}
              className="group flex items-start gap-2.5 rounded-lg border border-line px-3 py-2 text-left transition-colors hover:border-neon/40 hover:bg-neon/5 disabled:opacity-40 disabled:hover:border-line disabled:hover:bg-transparent"
            >
              <ExternalLink size={13} className="mt-0.5 shrink-0 text-mute group-hover:text-neon" />
              <span className="min-w-0">
                <span className="block text-sm text-ink">{t?.name ?? l.id}</span>
                <span className="block text-[11px] text-mute">{off ?? l.what}</span>
              </span>
            </button>
          );
        })}
      </div>
    </Card>
  );
}
