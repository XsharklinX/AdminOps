// AdminOps en miniatura: el caso de ahora, arreglos rápidos y el reloj.
import { Loader2, Maximize2 } from "lucide-react";
import { useEffect, useState } from "react";
import { invoke } from "@tauri-apps/api/core";
import { casesApi, logQuietly, tweaksApi } from "../lib/api";
import { elapsed } from "../lib/currentCase";
import { useToast } from "./feedback";
import type { Case } from "../lib/api";

/** Arreglos rápidos que se muestran siempre (con caso o sin él). */
const QUICK_FIXES = [
  { id: "repair.print-queue", label: "Vaciar cola de impresión" },
  { id: "cleanup.dns-cache", label: "Vaciar caché DNS" },
  { id: "repair.explorer", label: "Reiniciar Explorador" },
  { id: "cleanup.user-temp", label: "Limpiar temporales" },
  { id: "repair.audio", label: "Reiniciar audio" },
  { id: "repair.time-sync", label: "Sincronizar hora" },
];

export function AdminOpsMini() {
  const [current, setCurrent] = useState<Case | null>(null);
  const [now, setNow] = useState(() => Date.now() / 1000);
  const [running, setRunning] = useState<string | null>(null);
  const toast = useToast();

  useEffect(() => {
    const reload = () => void casesApi.current().then(setCurrent).catch(logQuietly("AdminOpsMini"));
    reload();
    const id = window.setInterval(reload, 10_000);
    const onKey = (ev: KeyboardEvent) => ev.code === "Escape" && openFull();
    window.addEventListener("keydown", onKey);
    return () => {
      window.clearInterval(id);
      window.removeEventListener("keydown", onKey);
    };
  }, []);

  useEffect(() => {
    if (!current) return;
    const id = window.setInterval(() => setNow(Date.now() / 1000), 1000);
    return () => window.clearInterval(id);
  }, [current]);

  const runFix = async (id: string, label: string) => {
    setRunning(id);
    try {
      const r = await tweaksApi.run(id);
      const bad = r.message.toLowerCase().includes("error") || r.message.toLowerCase().includes("falló") || r.message.toLowerCase().includes("no se pudo");
      toast(bad ? "error" : "ok", `${label}: ${r.message}`);
    } catch (e) {
      toast("error", `${label}: ${String(e)}`);
    } finally {
      setRunning(null);
    }
  };

  const openFull = () => void invoke("close_adminops_mini").catch(logQuietly("AdminOpsMini"));

  return (
    <div className="flex h-screen flex-col gap-2.5 overflow-hidden bg-void p-3 text-ink select-none">
      <div className="flex items-center justify-between">
        <span className="text-xs font-semibold text-neon">AdminOps</span>
        <button onClick={openFull} className="rounded p-1 text-mute hover:bg-panel-2 hover:text-ink" title="Abrir entera">
          <Maximize2 size={13} />
        </button>
      </div>

      {current ? (
        <div className="rounded-lg border border-neon/20 bg-neon/[0.06] p-2">
          <p className="text-[10px] uppercase tracking-wide text-neon">Caso · {elapsed(now - current.started)}</p>
          <p className="truncate text-xs font-medium text-ink">{current.person || "Sin persona"}</p>
          <p className="truncate text-[11px] text-dim">{current.machine}</p>
          <p className="mt-1 line-clamp-2 text-[11px] leading-snug text-ink">{current.notes || "Sin notas"}</p>
        </div>
      ) : (
        <div className="rounded-lg border border-line bg-panel/40 p-2">
          <p className="text-xs text-dim">No hay caso abierto.</p>
        </div>
      )}

      <div className="flex-1 overflow-y-auto">
        <p className="mb-1 text-[10px] uppercase tracking-wide text-mute">Arreglos rápidos</p>
        <div className="flex flex-col gap-1">
          {QUICK_FIXES.map((f) => (
            <button
              key={f.id}
              disabled={running !== null}
              onClick={() => void runFix(f.id, f.label)}
              className="flex items-center justify-between rounded-md border border-line-2 bg-panel/60 px-2 py-1.5 text-[11px] text-ink hover:bg-panel-2 disabled:opacity-50"
            >
              <span className="truncate">{f.label}</span>
              {running === f.id && <Loader2 size={11} className="animate-spin text-neon" />}
            </button>
          ))}
        </div>
      </div>

      <button
        onClick={openFull}
        className="flex h-8 items-center justify-center gap-1.5 rounded-lg bg-neon text-[12px] font-medium text-on-neon hover:brightness-110"
      >
        <Maximize2 size={12} /> Abrir entera
      </button>
      <p className="text-center text-[9px] text-mute">Ctrl+Alt+A alterna · Esc cierra</p>
    </div>
  );
}
