import type { AudioDev, AudioView, MicPrivacy } from "./api/audio";

/** Pico a partir del cual se dice «te oye» (la voz normal da 0,1–0,5; el ruido de fondo, menos de 0,02). */
export const HEARD = 0.05;

/** «Micrófono (Jabra Evolve 20)» → «Jabra Evolve 20»: el aparato, para emparejar micro y altavoz. */
export function gadget(name: string): string {
  // Como en audiofix.rs: el último paréntesis entero, también con otros dentro («Micrófono (Realtek(R) Audio)»).
  const n = name.trim();
  if (!n.endsWith(")")) return n;
  let depth = 0;
  for (let i = n.length - 1; i >= 0; i--) {
    if (n[i] === ")") depth++;
    else if (n[i] === "(" && --depth === 0) {
      const inner = n.slice(i + 1, -1).trim();
      return inner || n;
    }
  }
  return n;
}

/** Los permisos que faltan, en palabras. */
export function privacyProblems(p: MicPrivacy): string[] {
  const out: string[] = [];
  if (!p.device) out.push("El micrófono está apagado para todo el equipo.");
  else if (!p.apps) out.push("Windows no deja usar el micrófono a las aplicaciones.");
  else {
    if (!p.desktop) out.push("Las aplicaciones de escritorio (Teams clásico, Zoom, el navegador) no tienen permiso de micrófono.");
    if (p.teamsNew === false) out.push("Teams no tiene permiso de micrófono.");
  }
  return out;
}

export interface AudioAdvice {
  /** Frase de lo que pasa (vacía si todo está bien). */
  problem: string;
  /** El micrófono que conviene usar. */
  mic: AudioDev | null;
  /** Su altavoz o cascos (mismo aparato), si lo hay. */
  speaker: AudioDev | null;
}

/**
 * Qué conviene hacer tras escuchar: si el predeterminado no oye y otro sí, usar ese; si el
 * predeterminado oye pero está bajo o en silencio, subirlo. `peaks`: el pico máximo de cada micrófono.
 */
export function advise(view: AudioView, peaks: Record<string, number | null>): AudioAdvice {
  const def = view.capture.find((d) => d.default) ?? null;
  const heard = view.capture.filter((d) => (peaks[d.id] ?? 0) >= HEARD).sort((a, b) => (peaks[b.id] ?? 0) - (peaks[a.id] ?? 0));
  const best = heard[0] ?? null;
  const pair = (mic: AudioDev | null) => (mic ? view.render.find((r) => gadget(r.name).toLowerCase() === gadget(mic.name).toLowerCase()) ?? null : null);
  if (!view.capture.length) return { problem: "No hay ningún micrófono conectado.", mic: null, speaker: null };
  if (def && best && best.id !== def.id && (peaks[def.id] ?? 0) < HEARD)
    return { problem: `El predeterminado es «${gadget(def.name)}» y no te oye; «${gadget(best.name)}» sí.`, mic: best, speaker: pair(best) };
  if (!def && best) return { problem: "No hay micrófono predeterminado.", mic: best, speaker: pair(best) };
  const target = def ?? best;
  if (target && (target.muted || target.volume < 50))
    return { problem: target.muted ? `«${gadget(target.name)}» está en silencio.` : `«${gadget(target.name)}» está al ${target.volume} %.`, mic: target, speaker: pair(target) };
  if (target && !target.defaultComm) return { problem: `Para las llamadas Windows usa otro micrófono, no «${gadget(target.name)}».`, mic: target, speaker: pair(target) };
  return { problem: "", mic: target, speaker: null };
}
