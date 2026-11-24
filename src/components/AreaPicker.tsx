import { useEffect, useRef, useState } from "react";
import { faultRecApi, type FaultRegion } from "../lib/api";
import { logQuietly } from "../lib/api/core";
import { dragRegion, MIN_REGION, regionLabel } from "../lib/faultrec";

type Pt = { x: number; y: number };

/**
 * «Una zona» de Grabar el fallo: una capa transparente encima de todo el monitor (faultrec.rs, ventana
 * `recarea`). Se arrastra para marcar la zona; Intro o «Grabar esta zona» la confirma y Esc cancela.
 * Lo de fuera de la zona se oscurece, para que se vea qué entra en el vídeo.
 */
export function AreaPicker() {
  const [a, setA] = useState<Pt | null>(null);
  const [b, setB] = useState<Pt | null>(null);
  const [dragging, setDragging] = useState(false);
  const sent = useRef(false);
  const dpr = window.devicePixelRatio || 1;
  const maxW = Math.round(window.innerWidth * dpr);
  const maxH = Math.round(window.innerHeight * dpr);
  const region: FaultRegion | null = a && b ? dragRegion(a, b, dpr, maxW, maxH) : null;

  const finish = (r: FaultRegion | null) => {
    if (sent.current) return;
    sent.current = true;
    faultRecApi.regionDone(r).catch((e) => {
      sent.current = false;
      logQuietly("AreaPicker.done")(e);
    });
  };

  // La ventana es transparente: la página también.
  useEffect(() => {
    for (const el of [document.documentElement, document.body, document.getElementById("root")]) if (el) el.style.background = "transparent";
  }, []);

  useEffect(() => {
    const key = (e: KeyboardEvent) => {
      if (e.key === "Escape") finish(null);
      if (e.key === "Enter" && region && !dragging) finish(region);
    };
    window.addEventListener("keydown", key);
    return () => window.removeEventListener("keydown", key);
  });

  const box = a && b ? { left: Math.min(a.x, b.x), top: Math.min(a.y, b.y), width: Math.abs(a.x - b.x), height: Math.abs(a.y - b.y) } : null;
  // Los botones van debajo de la zona o, si no caben, encima por dentro.
  const below = box ? box.top + box.height + 56 < window.innerHeight : true;

  return (
    <div
      className="fixed inset-0 cursor-crosshair select-none"
      onPointerDown={(e) => {
        if (e.button !== 0 || (e.target as HTMLElement).closest("button")) return;
        e.currentTarget.setPointerCapture(e.pointerId);
        setA({ x: e.clientX, y: e.clientY });
        setB({ x: e.clientX, y: e.clientY });
        setDragging(true);
      }}
      onPointerMove={(e) => dragging && setB({ x: e.clientX, y: e.clientY })}
      onPointerUp={() => setDragging(false)}
      onContextMenu={() => finish(null)}
    >
      {/* Sin zona, todo oscurecido; con zona, solo lo de fuera (la sombra enorme del rectángulo). */}
      {!box && <div className="absolute inset-0 bg-black/45" />}
      {box && (
        <div
          className="absolute rounded-sm border-2 border-neon"
          style={{ ...box, boxShadow: "0 0 0 100vmax rgb(0 0 0 / 0.45)" }}
        >
          <span className="absolute -top-7 left-0 rounded bg-void/90 px-2 py-0.5 font-mono text-[11px] whitespace-nowrap text-ink">
            {region ? regionLabel(region) : `Más grande: al menos ${MIN_REGION} × ${MIN_REGION}`}
          </span>
          {!dragging && region && (
            <span className={`absolute right-0 flex gap-2 ${below ? "top-full mt-2" : "bottom-2 right-2"}`}>
              <button type="button" onClick={() => finish(region)} className="rounded-md bg-neon px-3 py-1.5 text-[13px] font-medium text-void shadow-lg">
                Grabar esta zona
              </button>
              <button type="button" onClick={() => finish(null)} className="rounded-md border border-line-2 bg-void/90 px-3 py-1.5 text-[13px] text-ink shadow-lg">
                Cancelar
              </button>
            </span>
          )}
        </div>
      )}
      {!dragging && (
        <div className="pointer-events-none absolute top-6 left-1/2 -translate-x-1/2 rounded-lg border border-line-2 bg-void/90 px-4 py-2.5 text-center text-[13px] text-ink shadow-2xl">
          {box ? "Vuelve a arrastrar para cambiarla · Intro para grabarla · Esc para cancelar" : "Arrastra para marcar la zona que se graba · Esc para cancelar"}
        </div>
      )}
    </div>
  );
}
