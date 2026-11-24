// Windows Update: lo que dice cada fila (el avance) y qué escalón toca después de un fallo.
// Lógica pura, sin pantalla, para poder probarla.
import type { UpdateCause, UpdateProgress } from "./api";
import { bytes } from "./format";
import { nextRung, type Rung } from "./fixLadder";

/** El texto del botón de cada arreglo que puede tocar. */
const TRY: Record<string, string> = {
  "repair.esp-fonts": "Probar: liberar la partición reservada",
  "repair.wu-services": "Probar: reiniciar los servicios de Windows Update",
  "repair.dism": "Probar: reparar componentes (DISM)",
  "repair.windows-update": "Probar: restablecer Windows Update",
  "cleanup.windows-update-cache": "Probar: vaciar las descargas de Windows Update",
  "repair.time-sync": "Probar: sincronizar la hora",
  "repair.sfc": "Probar: comprobar los archivos (SFC)",
};

export const tryLabel = (fixId: string) => TRY[fixId] ?? "Probar este arreglo";

/** Dónde está una fila que falló: el arreglo que toca ahora y los que ya se probaron. */
export interface Ladder {
  /** El arreglo (id del catálogo) que ofrece el botón, o un paso manual. */
  step: { kind: "fix"; id: string } | { kind: "manual"; label: string; detail: string } | null;
  tried: string[];
}

/** El primer escalón: el arreglo de la causa medida. Sin causa o sin arreglo, no hay botón. */
export function startLadder(cause: UpdateCause | null): Ladder {
  return { step: cause?.fix ? { kind: "fix", id: cause.fix } : null, tried: [] };
}

/** Se probó el arreglo actual: queda anotado (el botón sigue hasta que se pida «Siguiente paso»). */
export function markTried(l: Ladder, fixId: string): Ladder {
  return l.tried.includes(fixId) ? l : { ...l, tried: [...l.tried, fixId] };
}

const rungToStep = (r: Rung | null): Ladder["step"] => (r ? (r.kind === "fix" ? { kind: "fix", id: r.id } : { kind: "manual", label: r.label, detail: r.detail }) : null);

/** El siguiente escalón de la escalera. `null` si ya no queda ninguno. */
export function nextStep(l: Ladder): Ladder["step"] {
  if (!l.step || l.step.kind === "manual") return null;
  // El arreglo actual cuenta como probado dos veces: aquí nunca se repite el mismo (la escalera de un arreglo suelto sí lo hace).
  const cur = l.step.id;
  return rungToStep(nextRung(cur, [...l.tried.filter((t) => t !== cur), cur, cur]));
}

export function advance(l: Ladder): Ladder {
  const step = nextStep(l);
  if (!step) return l;
  return { step, tried: l.step?.kind === "fix" && !l.tried.includes(l.step.id) ? [...l.tried, l.step.id] : l.tried };
}

/** «Descargando 81 % · 52 MB», «Instalando 48 %», «Instalada», «Pide reiniciar». */
export function progressText(p: UpdateProgress): string {
  switch (p.phase) {
    case "downloading":
      return `Descargando ${p.percent ?? 0} %${p.bytes ? ` · ${bytes(p.bytes, 0)}` : ""}`;
    case "installing":
      return `Instalando ${p.percent ?? 0} %`;
    case "done":
      return "Instalada";
    case "reboot":
      return "Pide reiniciar";
    case "failed":
      return "Falló";
  }
}

/** Una fila en curso enseña barra; las terminadas, no. */
export const showsBar = (p: UpdateProgress) => p.phase === "downloading" || p.phase === "installing";

/** Una clave estable para una fila del historial (no tienen id). */
export const historyKey = (h: { title: string; date: string | null }) => `${h.title}|${h.date ?? ""}`;
