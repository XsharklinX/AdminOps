// Barrita fina arriba mientras termina de cargarse lo de fondo. Si algo tarda
// de verdad, se dice qué es, en lugar de dejar al técnico mirando una pantalla quieta.
import { useEffect, useState, useSyncExternalStore } from "react";
import { bootPending, onBootChange, slowOnes } from "../lib/boot";

let snapshot = bootPending();
const subscribe = (cb: () => void) =>
  onBootChange(() => {
    snapshot = bootPending();
    cb();
  });

export function BootBar() {
  const list = useSyncExternalStore(subscribe, () => snapshot);
  const [now, setNow] = useState(() => Date.now());
  useEffect(() => {
    if (!list.length) return;
    const t = window.setInterval(() => setNow(Date.now()), 1000);
    return () => window.clearInterval(t);
  }, [list.length]);
  if (!list.length) return null;
  const slow = slowOnes(list, now);
  return (
    <div className="pointer-events-none fixed inset-x-0 top-0 z-[55]" role="status" aria-live="polite">
      <div className="h-0.5 overflow-hidden bg-neon/10">
        <div className="h-full w-1/3 bg-neon/70" style={{ animation: "bootbar 1.2s ease-in-out infinite" }} />
      </div>
      {slow.length > 0 && (
        <p className="mx-auto mt-1 w-fit rounded-md border border-line-2 bg-panel/95 px-2 py-0.5 text-[11px] text-mute shadow">
          Esperando a: {slow.join(", ")}
        </p>
      )}
    </div>
  );
}
