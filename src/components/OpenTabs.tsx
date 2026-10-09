import { X } from "lucide-react";
import { pageLabel, type PageId } from "./Sidebar";

/**
 * Las pantallas abiertas, como pestañas. AdminOps ya las mantiene vivas (se
 * vuelve a ellas sin recargar): aquí se ven y se salta o se cierra con un clic,
 * o con Ctrl+Tab, Ctrl+Mayús+Tab y Ctrl+W.
 */
export function OpenTabs({ tabs, current, onPick, onClose }: { tabs: PageId[]; current: PageId; onPick: (p: PageId) => void; onClose: (p: PageId) => void }) {
  return (
    <div role="tablist" aria-label="Pantallas abiertas" className="no-scrollbar flex shrink-0 items-end gap-0.5 overflow-x-auto border-b border-line bg-panel px-3 pt-1.5">
      {tabs.map((p) => {
        const on = p === current;
        return (
          <div
            key={p}
            className={`group relative -mb-px flex max-w-52 shrink-0 items-center rounded-t-md border text-[12.5px] transition-colors ${
              on ? "border-line border-b-void bg-void text-ink" : "border-transparent text-dim hover:bg-panel-2 hover:text-ink"
            }`}
          >
            <button
              role="tab"
              aria-selected={on}
              onClick={() => onPick(p)}
              onAuxClick={(e) => e.button === 1 && onClose(p)}
              className="min-w-0 truncate py-1.5 pr-1 pl-3"
              title={`${pageLabel(p)} · clic con la rueda para cerrarla`}
            >
              {pageLabel(p)}
            </button>
            {tabs.length > 1 && (
              <button
                onClick={() => onClose(p)}
                className={`mr-1 rounded p-0.5 text-mute hover:bg-panel-2 hover:text-ink ${on ? "" : "opacity-0 group-hover:opacity-100 focus:opacity-100"}`}
                title="Cerrar (Ctrl+W)"
                aria-label={`Cerrar ${pageLabel(p)}`}
              >
                <X size={12} />
              </button>
            )}
          </div>
        );
      })}
    </div>
  );
}
