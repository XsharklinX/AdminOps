// Checklist que se marca sola: cada punto con `auto` se da por hecho cuando el
// diario de AdminOps (o un diagnóstico) muestra que esa tarea se hizo en la sesión.
import type { JournalEntry } from "./api";

export const AUTO_TASKS: { id: string; label: string }[] = [
  { id: "", label: "A mano" },
  { id: "diagnostic", label: "Diagnóstico" },
  { id: "restorepoint", label: "Punto de restauración" },
  { id: "cleanup", label: "Limpieza" },
  { id: "updates", label: "Actualizar programas o Windows" },
  { id: "startup", label: "Desactivar algo del inicio" },
  { id: "antivirus", label: "Análisis antivirus" },
  { id: "bloatware", label: "Quitar bloatware" },
  { id: "install", label: "Instalar programas" },
  { id: "users", label: "Crear usuario" },
  { id: "domain", label: "Unir al dominio" },
  { id: "privacy", label: "Ajustes de privacidad" },
  { id: "performance", label: "Ajustes de rendimiento" },
  { id: "network", label: "Reparar la red" },
  { id: "drivers", label: "Copia de drivers" },
  { id: "backup", label: "Copia de datos" },
  { id: "printer", label: "Impresoras" },
  { id: "repair", label: "Reparaciones de Windows (SFC, DISM…)" },
];

const starts = (t: string, ...p: string[]) => p.some((x) => t.startsWith(x));

function matches(auto: string, e: JournalEntry): boolean {
  const t = e.title;
  const id = e.tweakId ?? "";
  switch (auto) {
    case "restorepoint":
      return e.op === "restorePoint";
    case "cleanup":
      return id.startsWith("cleanup.");
    case "updates":
      return starts(t, "Actualizar ", "Windows Update");
    case "startup":
      return starts(t, "Inicio: desactivar");
    case "antivirus":
      return id === "repair.defender-scan";
    case "bloatware":
      return starts(t, "Quitar app:");
    case "install":
      return starts(t, "Instalar ");
    case "users":
      return starts(t, "Usuarios: crear");
    case "domain":
      return starts(t, "Dominio: unir");
    case "privacy":
      return id.startsWith("privacy.") && e.op === "apply";
    case "performance":
      return id.startsWith("performance.") && e.op === "apply";
    case "network":
      return starts(t, "Reparar la red") || id === "repair.network-reset";
    case "drivers":
      return t === "Copia de seguridad de drivers";
    case "backup":
      return starts(t, "Copia de datos");
    case "printer":
      return id === "repair.print-queue" || /impresor/i.test(t);
    case "repair":
      return id.startsWith("repair.");
    default:
      return false;
  }
}

/** ¿Se hizo la tarea `auto` durante la sesión? */
export function autoDone(auto: string, work: JournalEntry[], snapshots: number[], started: number, baseline: number): boolean {
  if (!auto) return false;
  if (auto === "diagnostic") return snapshots.some((ts) => ts > started && ts !== baseline);
  return work.some((e) => e.ok && e.timestamp >= started && matches(auto, e));
}
