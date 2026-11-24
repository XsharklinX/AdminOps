// Tamaño de toda la interfaz con Ctrl + / Ctrl − / Ctrl+0 y Ctrl + rueda.
// Se recuerda por monitor: la ventana avisa (Rust) cuando pasa a otro monitor y
// se aplica el tamaño de ese monitor o, si no tiene uno, el general.
import { invoke } from "@tauri-apps/api/core";
import { listen } from "@tauri-apps/api/event";
import { useEffect, useRef, useState } from "react";
import { applyAppearance, effectiveZoom, setMonitorName, setZoomHere } from "../lib/prefs";
import { nextZoom, zoomKeyDir, zoomPercent } from "../lib/zoom";

const SHOW_MS = 1000;

export function ZoomKeys({ disabled }: { disabled: boolean }) {
  const [shown, setShown] = useState<number | null>(null);
  const timer = useRef(0);
  const disabledRef = useRef(disabled);
  disabledRef.current = disabled;

  useEffect(() => {
    // Monitor de la ventana: el tamaño de ese monitor se aplica al abrir y cada vez que cambia.
    const sync = (name: string | null) => {
      setMonitorName(name);
      applyAppearance();
      window.setTimeout(() => window.dispatchEvent(new Event("resize")), 150);
    };
    invoke<string | null>("current_monitor_name")
      .then((n) => n && sync(n))
      .catch(() => {});
    const un = listen<string | null>("monitor-changed", ({ payload }) => sync(payload ?? null));
    return () => void un.then((f) => f());
  }, []);

  useEffect(() => {
    const change = (dir: 1 | -1 | 0) => {
      const now = effectiveZoom();
      const to = nextZoom(now, dir);
      if (to !== now) {
        setZoomHere(to);
        // Las vistas web incrustadas (portales) vuelven a medir su sitio cuando ya cambió el tamaño.
        window.setTimeout(() => window.dispatchEvent(new Event("resize")), 150);
      }
      setShown(to);
      window.clearTimeout(timer.current);
      timer.current = window.setTimeout(() => setShown(null), SHOW_MS);
    };
    const onKey = (ev: KeyboardEvent) => {
      if (disabledRef.current) return;
      const dir = zoomKeyDir(ev);
      if (dir === null) return;
      ev.preventDefault();
      change(dir);
    };
    let lastWheel = 0;
    const onWheel = (ev: WheelEvent) => {
      if (!ev.ctrlKey || disabledRef.current) return;
      ev.preventDefault();
      // Una rueda suelta manda muchos avisos seguidos: un paso cada vez.
      if (ev.deltaY === 0 || ev.timeStamp - lastWheel < 120) return;
      lastWheel = ev.timeStamp;
      change(ev.deltaY < 0 ? 1 : -1);
    };
    window.addEventListener("keydown", onKey);
    window.addEventListener("wheel", onWheel, { passive: false });
    return () => {
      window.clearTimeout(timer.current);
      window.removeEventListener("keydown", onKey);
      window.removeEventListener("wheel", onWheel);
    };
  }, []);

  if (shown === null) return null;
  return (
    <div role="status" className="pointer-events-none fixed top-3 left-1/2 z-[96] -translate-x-1/2 rounded-full border border-line-2 bg-panel px-4 py-1.5 text-center text-sm text-ink shadow-xl">
      Tamaño {zoomPercent(shown)}
      {shown !== 1 && <span className="ml-2 text-xs text-mute">Ctrl+0 para volver</span>}
    </div>
  );
}
