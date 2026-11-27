import type { PowerSide, PowerState } from "./api/power";

const LID = ["no hace nada", "suspende", "hiberna", "se apaga"];

function mins(n: number): string {
  if (n === 0) return "nunca";
  if (n % 60 === 0) return n === 60 ? "a la hora" : `a las ${n / 60} horas`;
  return `a los ${n} min`;
}

/** «la pantalla se apaga a los 2 min y se suspende a los 10 min», o con «nunca». */
export function sideSentence(s: PowerSide): string {
  const screen = s.screenMin === 0 ? "la pantalla no se apaga nunca" : `la pantalla se apaga ${mins(s.screenMin)}`;
  const sleep = s.sleepMin === 0 ? "no se suspende nunca" : `se suspende ${mins(s.sleepMin)}`;
  return `${screen} y ${sleep}`;
}

/** Qué hace la tapa, enchufado y con batería, en una frase. */
export function lidSentence(st: PowerState): string {
  const ac = LID[st.ac.lid] ?? "no se sabe";
  const dc = LID[st.dc.lid] ?? "no se sabe";
  if (!st.battery || ac === dc) return ac === "suspende" && st.battery ? "suspende, también en la base" : ac;
  return `enchufado ${ac}; con batería ${dc}`;
}

export const LID_OPTIONS = LID;

/** Los perfiles de un clic, con el texto de la maqueta aprobada. */
export const PROFILE_TEXT: Record<string, { title: string; hint: string }> = {
  office: { title: "Oficina", hint: "pantalla 10 min · suspende 30 min" },
  present: { title: "Presentaciones", hint: "nunca se apaga mientras esté activo" },
  always: { title: "No se apaga nunca", hint: "reloj, escáner, servidor pequeño" },
  dock: { title: "Portátil en base", hint: "tapa cerrada = sigue encendido" },
};
