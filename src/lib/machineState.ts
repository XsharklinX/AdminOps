// El estado del equipo que se ve siempre: la barra de arriba (nombre, dominio,
// administrador, Internet, disco, avisos) y las marcas junto a las secciones de
// la barra lateral («3 avisos» en Diagnóstico, «dominio» en Dominio…).
//
// Todo son lecturas rápidas (sin PowerShell) y se repiten cada minuto mientras
// la ventana está a la vista; el diagnóstico es el último guardado, no uno nuevo.
import { useCallback, useEffect, useState } from "react";
import { contextApi, diagApi, logQuietly, type InternetProbe, type MachineContext } from "./api";
import { DIAG_COUNT_CHANGED, DIAG_UPDATED } from "./diagRun";
import { useOnJournalChange } from "./journalEvents";
import { navKey } from "./sections";

export type Tone = "ok" | "warn" | "bad" | "neutral";

export interface Badge {
  tone: Tone;
  text: string;
  /** Lo que significa, para el ratón y los lectores de pantalla. */
  title: string;
}

export interface MachineState {
  ctx: MachineContext | null;
  net: InternetProbe | null;
  /** Avisos del último diagnóstico: problemas y avisos, o null si nunca se hizo. */
  findings: { bad: number; warn: number } | null;
  /** Marcas de la barra lateral, por clave de línea («users:domain», «router»…). */
  badges: Record<string, Badge>;
}

const GB = 1024 ** 3;
const REFRESH_MS = 60_000;

/** Cómo de preocupante es el espacio libre: poco es menos de un 10 % o de 15 GB. */
export function diskTone(free: number, total: number): Tone {
  if (total <= 0) return "neutral";
  if (free < 5 * GB || free / total < 0.05) return "bad";
  if (free < 15 * GB || free / total < 0.1) return "warn";
  return "ok";
}

export const gbText = (b: number) => `${b >= 100 * GB ? Math.round(b / GB) : (b / GB).toFixed(b < 10 * GB ? 1 : 0)} GB`;

export function badgesOf(s: Omit<MachineState, "badges">): Record<string, Badge> {
  const out: Record<string, Badge> = {};
  if (s.findings) {
    const { bad, warn } = s.findings;
    const n = bad + warn;
    out[navKey("machine", "diagnostics")] = n
      ? { tone: bad ? "bad" : "warn", text: String(n), title: `${n} ${n === 1 ? "cosa" : "cosas"} que atender según el último diagnóstico` }
      : { tone: "ok", text: "bien", title: "El último diagnóstico no encontró nada que atender" };
  }
  if (s.ctx) {
    const { join, joinName, freeBytes, totalBytes } = s.ctx;
    if (join === "domain") out[navKey("users", "domain")] = { tone: "ok", text: "unido", title: `Este equipo está en el dominio ${joinName ?? ""}`.trim() };
    else if (join === "workgroup") out[navKey("users", "domain")] = { tone: "neutral", text: "no", title: "Este equipo no está en ningún dominio" };
    const tone = diskTone(freeBytes, totalBytes);
    if (tone !== "ok" && tone !== "neutral") out[navKey("space", "space")] = { tone, text: gbText(freeBytes), title: `Quedan ${gbText(freeBytes)} libres en ${s.ctx.systemDrive}` };
  }
  if (s.net && !s.net.online) out[navKey("router")] = { tone: "bad", text: "sin red", title: "No hay conexión a Internet" };
  return out;
}

/** Lee el estado del equipo al abrir AdminOps y cada minuto (si la ventana se ve). */
export function useMachineState(): MachineState & { refresh: () => void } {
  const [ctx, setCtx] = useState<MachineContext | null>(null);
  const [net, setNet] = useState<InternetProbe | null>(null);
  const [findings, setFindings] = useState<MachineState["findings"]>(null);

  const refresh = useCallback(() => {
    contextApi.machine().then(setCtx).catch(logQuietly("machineState"));
    contextApi.internet().then(setNet).catch(logQuietly("machineState"));
    diagApi
      .latest()
      .then((d) =>
        setFindings(
          d ? { bad: d.findings.filter((f) => f.severity === "bad").length, warn: d.findings.filter((f) => f.severity === "warn").length } : null,
        ),
      )
      .catch(logQuietly("machineState"));
  }, []);

  useEffect(() => {
    // La primera lectura, un momento después de abrir: la primera página va antes.
    const first = window.setTimeout(refresh, 1200);
    const t = window.setInterval(() => {
      if (document.visibilityState === "visible") refresh();
    }, REFRESH_MS);
    const onVisible = () => document.visibilityState === "visible" && refresh();
    document.addEventListener("visibilitychange", onVisible);
    // Un hallazgo aceptado, o un análisis que se completa, cambian los avisos al momento.
    window.addEventListener(DIAG_COUNT_CHANGED, refresh);
    window.addEventListener(DIAG_UPDATED, refresh);
    return () => {
      window.removeEventListener(DIAG_COUNT_CHANGED, refresh);
      window.removeEventListener(DIAG_UPDATED, refresh);
      window.clearTimeout(first);
      window.clearInterval(t);
      document.removeEventListener("visibilitychange", onVisible);
    };
  }, [refresh]);
  // Un cambio deshecho desde otro sitio se ve enseguida.
  useOnJournalChange(refresh);

  return { ctx, net, findings, badges: badgesOf({ ctx, net, findings }), refresh };
}
