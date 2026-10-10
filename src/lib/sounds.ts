// Sonidos suaves y opcionales (Ajustes → Apariencia): un «tic» al terminar una
// tarea larga y otro tono si algo falla. Se generan al momento, sin archivos.
// Apagados de fábrica y en silencio mientras el micrófono o la cámara están en
// uso (una llamada de Teams, una reunión): ahí nadie quiere oír a AdminOps.
import { invoke } from "@tauri-apps/api/core";
import { getPrefs } from "./prefs";

export type SoundKind = "done" | "error";

/** Notas de cada sonido: frecuencia (Hz), inicio y duración (s). */
export const NOTES: Record<SoundKind, [number, number, number][]> = {
  done: [
    [660, 0, 0.09],
    [990, 0.08, 0.16],
  ],
  error: [
    [330, 0, 0.14],
    [247, 0.12, 0.22],
  ],
};

let ctx: AudioContext | null = null;
let busyUntil = 0;
let busy = false;

/** ¿Hay una llamada o una reunión? (micrófono o cámara en uso; se pregunta como mucho cada 30 s). */
async function inCall(): Promise<boolean> {
  if (Date.now() < busyUntil) return busy;
  busy = await invoke<boolean>("media_in_use").catch(() => false);
  busyUntil = Date.now() + 30_000;
  return busy;
}

/** Suena si están encendidos y no hay una llamada. `force`: para probar desde Ajustes. */
export async function playSound(kind: SoundKind, force = false): Promise<void> {
  const p = getPrefs();
  if ((!p.sounds && !force) || p.soundVolume <= 0) return;
  if (!force && (await inCall())) return;
  try {
    ctx ??= new AudioContext();
    const now = ctx.currentTime;
    for (const [freq, at, dur] of NOTES[kind]) {
      const osc = ctx.createOscillator();
      const gain = ctx.createGain();
      osc.type = "sine";
      osc.frequency.value = freq;
      const peak = 0.18 * p.soundVolume;
      gain.gain.setValueAtTime(0, now + at);
      gain.gain.linearRampToValueAtTime(peak, now + at + 0.01);
      gain.gain.exponentialRampToValueAtTime(0.0001, now + at + dur);
      osc.connect(gain).connect(ctx.destination);
      osc.start(now + at);
      osc.stop(now + at + dur + 0.02);
    }
  } catch {
    /* sin audio en este equipo: nada que hacer */
  }
}
