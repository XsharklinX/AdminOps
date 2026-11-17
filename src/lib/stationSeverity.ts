// Orden de gravedad de los puestos: lo que necesita atención primero.
// Fijo y explicado en la pantalla: no responde, disco, seguridad, reinicio
// pendiente, poco espacio, otros avisos y, al final, lo que está bien.
import type { Station } from "./api";

export type Severity = "offline" | "disk" | "security" | "reboot" | "space" | "other" | "ok";

/** De más a menos grave. */
export const SEVERITY_ORDER: Severity[] = ["offline", "disk", "security", "reboot", "space", "other", "ok"];

export const SEVERITY_LABEL: Record<Severity, string> = {
  offline: "No responde",
  disk: "Disco",
  security: "Seguridad",
  reboot: "Reinicio pendiente",
  space: "Poco espacio",
  other: "Otros avisos",
  ok: "Bien",
};

/** El problema más grave de un puesto, con la frase corta que lo explica. */
export function severityOf(s: Station): { severity: Severity; reason: string } {
  if (!s.online) return { severity: "offline", reason: s.remoteError };
  const r = s.remote;
  if (r) {
    if (r.diskHealth === "bad") return { severity: "disk", reason: "un disco está fallando" };
    if (r.diskHealth === "warn") return { severity: "disk", reason: "un disco da avisos de salud" };
    if (r.avOk === false) return { severity: "security", reason: "antivirus apagado o caducado" };
    if (r.updateDays != null && r.updateDays > 45) return { severity: "security", reason: `${Math.floor(r.updateDays)} días sin actualizaciones` };
    if (r.pendingReboot === true) return { severity: "reboot", reason: "reinicio pendiente" };
    if (r.bootDays != null && r.bootDays > 14) return { severity: "reboot", reason: `${Math.floor(r.bootDays)} días sin reiniciar` };
    if (r.freeGb != null && r.totalGb != null && (r.freeGb < 10 || (r.totalGb > 0 && r.freeGb / r.totalGb < 0.1))) {
      return { severity: "space", reason: `poco disco: ${r.freeGb.toFixed(1).replace(".", ",")} GB libres` };
    }
    if (r.bsods != null && r.bsods > 0) return { severity: "other", reason: `${r.bsods} pantallazo(s) azul(es) en 30 días` };
    if (s.warnings.length > 0) return { severity: "other", reason: s.warnings[0] };
    // Revisado a fondo y sin nada: bien.
    return { severity: "ok", reason: "bien" };
  }
  if (s.warnings.length > 0) return { severity: "other", reason: s.warnings[0] };
  // Responde, pero no se pudo mirar por dentro (o fue la comprobación rápida): nunca «Bien».
  return { severity: "other", reason: "sin revisar" };
}

export const severityRank = (s: Station): number => SEVERITY_ORDER.indexOf(severityOf(s).severity);

/** Copia de la lista, de lo más grave a lo más sano; a igual gravedad, por nombre. */
export function sortBySeverity(list: Station[]): Station[] {
  return list
    .map((s) => ({ s, k: severityRank(s) }))
    .sort((a, b) => a.k - b.k || a.s.host.localeCompare(b.s.host, "es", { numeric: true, sensitivity: "base" }))
    .map((x) => x.s);
}

/** Los que hay que volver a mirar: no respondían o tenían algún aviso. */
export const failed = (list: Station[]): Station[] => list.filter((s) => severityOf(s).severity !== "ok");
