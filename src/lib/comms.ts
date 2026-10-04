// Teams y el Correo: cómo se abren. La primera vez se pregunta (dentro de
// AdminOps, en el navegador o en su aplicación de Windows) y se recuerda; se
// cambia en Ajustes → Portales y correo.
import type { CommMode } from "./prefs";

export type CommKind = "teams" | "mail";

const EVENT = "adminops:open-comm";

/** Abre Teams o el Correo como haya elegido el técnico (o pregunta, la primera vez). */
export const openComm = (kind: CommKind) => window.dispatchEvent(new CustomEvent(EVENT, { detail: kind }));

export function onOpenComm(cb: (kind: CommKind) => void): () => void {
  const h = (e: Event) => cb((e as CustomEvent<CommKind>).detail);
  window.addEventListener(EVENT, h);
  return () => window.removeEventListener(EVENT, h);
}

export const COMM_NAME: Record<CommKind, string> = { teams: "Teams", mail: "el Correo" };

export const COMM_MODES: { id: Exclude<CommMode, "ask">; label: string; hint: string }[] = [
  { id: "adminops", label: "En AdminOps", hint: "Dentro de la ventana, con la cuenta guardada y sin salir de lo que estás haciendo." },
  { id: "browser", label: "En el navegador", hint: "En tu navegador de siempre, con las sesiones que ya tengas abiertas." },
  { id: "app", label: "En la aplicación", hint: "La aplicación de Windows instalada en el equipo (Teams, Outlook)." },
];
