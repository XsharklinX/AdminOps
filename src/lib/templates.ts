// Plantillas de texto: variables automáticas ({equipo}, {fecha}…) y preguntas
// propias ({?Nombre del usuario}) que se rellenan antes de copiar.
import { libraryApi, sheetApi, workApi } from "./api";

export const AUTO_VARS: [string, string][] = [
  ["fecha", "Fecha de hoy"],
  ["hora", "Hora actual"],
  ["equipo", "Nombre de este equipo"],
  ["red", "Red a la que está conectado"],
  ["cliente", "Cliente de la sesión en curso"],
  ["tecnico", "Tu nombre (Ajustes)"],
  ["empresa", "Tu empresa (Ajustes)"],
  ["usuario", "Usuario conectado en el equipo"],
  ["ip", "IP del equipo"],
  ["dominio", "Dominio o grupo de trabajo"],
  ["modelo", "Fabricante y modelo"],
  ["serie", "Número de serie"],
];

/** Lo que hace falta la ficha del equipo para resolverlo (tarda un par de segundos). */
const NEEDS_SHEET = ["usuario", "ip", "dominio", "modelo", "serie"];

/** Preguntas propias de una plantilla: {?Etiqueta}. */
export function questions(body: string): string[] {
  const out: string[] = [];
  for (const m of body.matchAll(/\{\?([^{}]{1,60})\}/g)) if (!out.includes(m[1].trim())) out.push(m[1].trim());
  return out;
}

export async function autoValues(body: string): Promise<Record<string, string>> {
  const used = new Set([...body.matchAll(/\{([a-zñ]+)\}/gi)].map((m) => m[1].toLowerCase()));
  const v: Record<string, string> = {};
  const now = new Date();
  v.fecha = now.toLocaleDateString("es", { day: "numeric", month: "long", year: "numeric" });
  v.hora = now.toLocaleTimeString("es", { hour: "2-digit", minute: "2-digit" });
  const [place, settings, session, sheet] = await Promise.all([
    used.has("equipo") || used.has("red") ? libraryApi.place().catch(() => null) : null,
    used.has("tecnico") || used.has("empresa") ? workApi.settings().catch(() => null) : null,
    used.has("cliente") ? workApi.session().catch(() => null) : null,
    NEEDS_SHEET.some((k) => used.has(k)) ? sheetApi.get().catch(() => null) : null,
  ]);
  v.equipo = place?.machineLabel ?? sheet?.host ?? "";
  v.red = place?.networkLabel ?? "";
  v.tecnico = settings?.technician ?? "";
  v.empresa = settings?.company ?? "";
  v.cliente = session?.clientName ?? "";
  v.usuario = (sheet?.user ?? "").split("\\").pop() ?? "";
  v.ip = sheet?.ip ?? "";
  v.dominio = sheet?.domain ?? "";
  v.modelo = sheet ? `${sheet.manufacturer} ${sheet.model}`.trim() : "";
  v.serie = sheet?.serial ?? "";
  return v;
}

export function fill(body: string, auto: Record<string, string>, answers: Record<string, string>): string {
  return body
    .replace(/\{\?([^{}]{1,60})\}/g, (_, k: string) => answers[k.trim()] ?? "")
    .replace(/\{([a-zñ]+)\}/gi, (whole, k: string) => (k.toLowerCase() in auto ? auto[k.toLowerCase()] : whole));
}
