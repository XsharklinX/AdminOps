import { ChevronLeft, ChevronRight, Eye, EyeOff, GripVertical, LayoutDashboard, RotateCcw } from "lucide-react";
import { useState, type ReactNode } from "react";
import { arrange, dropOn, PANEL_BLOCKS, shift, type PanelBlock } from "../lib/panelLayout";
import { setPrefs, usePrefs } from "../lib/prefs";
import { Button, iconBtn } from "./ui";

const SPAN = { 1: "", 2: "lg:col-span-2", 3: "lg:col-span-3" } as const;

/**
 * Las tarjetas del Panel en el orden que eligió el técnico. «Personalizar» deja
 * moverlas (arrastrando o con las flechas) y ocultar las que no usa.
 */
export function PanelGrid({ blocks }: { blocks: Record<PanelBlock, ReactNode> }) {
  const prefs = usePrefs().panel;
  const [editing, setEditing] = useState(false);
  const [dragging, setDragging] = useState<PanelBlock | null>(null);
  const [over, setOver] = useState<PanelBlock | null>(null);
  const order = arrange(prefs.order);
  const hidden = prefs.hidden;
  const save = (next: { order?: PanelBlock[]; hidden?: string[] }) => setPrefs({ panel: { order: next.order ?? order, hidden: next.hidden ?? hidden } });
  const custom = prefs.order.length > 0 || hidden.length > 0;

  return (
    <>
      {editing && (
        <div className="flex flex-wrap items-center gap-3 rounded-xl border border-neon/40 bg-neon/5 px-4 py-2.5">
          <LayoutDashboard size={15} className="text-neon" />
          <span className="min-w-0 flex-1 text-[13px] text-dim">Arrastra las tarjetas para ordenarlas, o usa las flechas. El ojo oculta las que no uses.</span>
          <Button kind="ghost" size="sm" onClick={() => setPrefs({ panel: { order: [], hidden: [] } })} disabled={!custom}>
            <RotateCcw size={13} /> Como de fábrica
          </Button>
          <Button size="sm" onClick={() => setEditing(false)}>
            Listo
          </Button>
        </div>
      )}
      <div className="grid grid-flow-dense gap-4 lg:grid-cols-3">
        {order.map((id, i) => {
          const off = hidden.includes(id);
          if (off && !editing) return null;
          const { label, span } = PANEL_BLOCKS[id];
          if (!editing) {
            // Una tarjeta que ahora no tiene nada que enseñar no deja hueco.
            return (
              <div key={id} className={`flex min-w-0 flex-col gap-5 empty:hidden [&>section]:h-full ${SPAN[span]}`}>
                {blocks[id]}
              </div>
            );
          }
          return (
            <div
              key={id}
              draggable
              onDragStart={(e) => {
                setDragging(id);
                e.dataTransfer.effectAllowed = "move";
              }}
              onDragEnd={() => {
                setDragging(null);
                setOver(null);
              }}
              onDragOver={(e) => {
                if (!dragging || dragging === id) return;
                e.preventDefault();
                setOver(id);
              }}
              onDrop={(e) => {
                e.preventDefault();
                if (dragging) save({ order: dropOn(order, dragging, id) });
                setDragging(null);
                setOver(null);
              }}
              className={`relative flex min-w-0 cursor-grab flex-col rounded-xl border border-dashed p-1.5 transition-colors ${SPAN[span]} ${
                over === id ? "border-neon bg-neon/10" : "border-line-2"
              } ${dragging === id ? "opacity-40" : ""}`}
            >
              <div className="mb-1.5 flex items-center gap-1 px-1 text-[12px] text-dim">
                <GripVertical size={13} className="shrink-0 text-mute" />
                <span className={`min-w-0 flex-1 truncate ${off ? "line-through" : ""}`}>{label}</span>
                <button onClick={() => save({ order: shift(order, id, -1) })} disabled={i === 0} className={iconBtn} title="Antes" aria-label={`Poner «${label}» antes`}>
                  <ChevronLeft size={13} />
                </button>
                <button onClick={() => save({ order: shift(order, id, 1) })} disabled={i === order.length - 1} className={iconBtn} title="Después" aria-label={`Poner «${label}» después`}>
                  <ChevronRight size={13} />
                </button>
                <button
                  onClick={() => save({ hidden: off ? hidden.filter((h) => h !== id) : [...hidden, id] })}
                  className={iconBtn}
                  title={off ? "Volver a enseñarla" : "Ocultarla"}
                  aria-label={off ? `Volver a enseñar «${label}»` : `Ocultar «${label}»`}
                >
                  {off ? <EyeOff size={13} /> : <Eye size={13} />}
                </button>
              </div>
              {/* Mientras se ordena, las tarjetas no responden a los clics. */}
              <div className={`pointer-events-none flex max-h-40 min-w-0 flex-col gap-5 overflow-hidden [&>section]:h-full ${off ? "opacity-30" : ""}`}>{blocks[id]}</div>
            </div>
          );
        })}
      </div>
      {!editing && (
        <button onClick={() => setEditing(true)} className="flex items-center gap-1.5 self-end text-xs text-mute transition-colors hover:text-ink">
          <LayoutDashboard size={13} /> Personalizar el panel
        </button>
      )}
    </>
  );
}
