import { useEffect, useRef } from "react";
import { faultRecApi } from "../lib/api";
import { logQuietly } from "../lib/api/core";
import { STROKE_LIFE_MS, strokeAlpha } from "../lib/faultrec";

type Stroke = { pts: [number, number][]; endedAt: number | null };

/** El color del trazo y su grosor (en píxeles de la página). */
const STROKE_COLOR = "#ff3b30";
const STROKE_WIDTH = 4;

/**
 * «Dibujar» de Grabar el fallo: una capa transparente encima del monitor grabado (faultrec.rs, ventana `recdraw`).
 * Con el ratón se pinta en rojo; cada trazo se borra solo a los 3 s de soltarlo (se desvanece al final). Esc la cierra.
 * Como es una ventana más del monitor, sale en el vídeo.
 */
export function DrawLayer() {
  const canvas = useRef<HTMLCanvasElement>(null);
  const strokes = useRef<Stroke[]>([]);
  const current = useRef<Stroke | null>(null);

  // La ventana es transparente: la página también.
  useEffect(() => {
    for (const el of [document.documentElement, document.body, document.getElementById("root")]) if (el) el.style.background = "transparent";
  }, []);

  useEffect(() => {
    const close = () => void faultRecApi.draw(false).catch(logQuietly("DrawLayer.close"));
    const key = (e: KeyboardEvent) => {
      if (e.key === "Escape") close();
    };
    window.addEventListener("keydown", key);
    return () => window.removeEventListener("keydown", key);
  }, []);

  // Se redibuja a cada fotograma: los trazos nuevos, y los viejos con su desvanecido hasta borrarse.
  useEffect(() => {
    const c = canvas.current;
    const ctx = c?.getContext("2d");
    if (!c || !ctx) return;
    let raf = 0;
    const draw = () => {
      const dpr = window.devicePixelRatio || 1;
      const w = Math.round(window.innerWidth * dpr);
      const h = Math.round(window.innerHeight * dpr);
      if (c.width !== w || c.height !== h) {
        c.width = w;
        c.height = h;
      }
      ctx.clearRect(0, 0, w, h);
      const now = performance.now();
      strokes.current = strokes.current.filter((s) => s.endedAt === null || now - s.endedAt < STROKE_LIFE_MS);
      ctx.lineCap = "round";
      ctx.lineJoin = "round";
      ctx.strokeStyle = STROKE_COLOR;
      ctx.lineWidth = STROKE_WIDTH * dpr;
      for (const s of strokes.current) {
        ctx.globalAlpha = s.endedAt === null ? 1 : strokeAlpha(now - s.endedAt);
        ctx.beginPath();
        s.pts.forEach(([x, y], i) => (i === 0 ? ctx.moveTo(x * dpr, y * dpr) : ctx.lineTo(x * dpr, y * dpr)));
        // Un clic sin mover deja un punto.
        if (s.pts.length === 1) ctx.lineTo(s.pts[0][0] * dpr + 0.01, s.pts[0][1] * dpr);
        ctx.stroke();
      }
      ctx.globalAlpha = 1;
      raf = requestAnimationFrame(draw);
    };
    raf = requestAnimationFrame(draw);
    return () => cancelAnimationFrame(raf);
  }, []);

  const end = () => {
    if (current.current) current.current.endedAt = performance.now();
    current.current = null;
  };

  return (
    <canvas
      ref={canvas}
      className="fixed inset-0 h-screen w-screen cursor-crosshair select-none"
      aria-label="Dibujar en pantalla: arrastra con el ratón; Esc para quitarlo"
      onPointerDown={(e) => {
        if (e.button !== 0) return;
        e.currentTarget.setPointerCapture(e.pointerId);
        // Al pulsarla, la capa sube encima de la píldora y la barra: se vuelven a poner arriba.
        void faultRecApi.raiseControls().catch(logQuietly("DrawLayer.raise"));
        const s: Stroke = { pts: [[e.clientX, e.clientY]], endedAt: null };
        current.current = s;
        strokes.current.push(s);
      }}
      onPointerMove={(e) => current.current?.pts.push([e.clientX, e.clientY])}
      onPointerUp={end}
      onPointerCancel={end}
    />
  );
}
