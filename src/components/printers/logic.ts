// Lógica pura de las pantallas de impresoras y escáneres RICOH y EPSON: qué
// decir de cada conexión y de cada estado, qué candidato es el mejor, a qué
// impresoras alcanza «Actualizar las RICOH», el resumen para el ticket y los
// pasos en vivo. Sin React, para poder probarla.
import type {
  DriverCandidate,
  DriverStatus,
  LibInstaller,
  ScanSoftware,
  VendorConnection,
  VendorDetected,
  VendorDownloadEvent,
  VendorKey,
  VendorPrinter,
  VendorStepEvent,
} from "../../lib/api";

export const VENDOR_NAME: Record<VendorKey, string> = { ricoh: "RICOH", epson: "EPSON", otra: "" };

/** Cómo está conectada, en corto: «USB», «Red · 10.20.4.31». */
export function connectionText(c: VendorConnection): string {
  switch (c.kind) {
    case "usb":
      return "USB";
    case "red":
      return c.ip ? `Red · ${c.ip}` : "Red";
    case "wsd":
      return c.ip ? `Red (WSD) · ${c.ip}` : "Red (WSD)";
    case "servidor":
      return "Desde un servidor";
    default:
      return c.port || "Sin puerto";
  }
}

export type Tone = "ok" | "warn" | "bad" | "mute";

/** Color del chip del estado del driver. */
export const STATUS_TONE: Record<DriverStatus, Tone> = {
  alDia: "ok",
  viejo: "warn",
  generico: "warn",
  falta: "bad",
  otraMarca: "bad",
  sinComparar: "mute",
};

/** Palabra del chip: «Viejo · hay 4.42» cuando se sabe la versión nueva. */
export function chipText(status: DriverStatus, label: string, newVersion: string | null): string {
  if (status === "viejo" && newVersion) return `Viejo · hay ${shortVersion(newVersion)}`;
  return label || "—";
}

/** «4.42.0.0» → «4.42»: los dos primeros números si el resto son ceros. */
export function shortVersion(v: string): string {
  const parts = v.trim().split(".");
  while (parts.length > 2 && parts[parts.length - 1] === "0") parts.pop();
  return parts.join(".");
}

const MATCH_RANK: Record<string, number> = { hardwareId: 0, modelo: 1, mismoDriver: 2, universal: 3 };

/**
 * El candidato que se ofrece primero: el que encaja mejor con el aparato (por ID
 * de hardware, luego por modelo) y, a igualdad, uno más nuevo que lo instalado.
 * Uno que se sabe más viejo no se elige nunca.
 */
export function bestCandidate(cands: DriverCandidate[]): DriverCandidate | null {
  const ok = cands.filter((c) => c.newer !== false);
  if (ok.length === 0) return null;
  return [...ok].sort((a, b) => (MATCH_RANK[a.matchedBy] ?? 9) - (MATCH_RANK[b.matchedBy] ?? 9) || Number(b.newer === true) - Number(a.newer === true))[0];
}

/** ¿Merece actualizarse? Driver viejo o genérico. */
export const needsUpdate = (p: VendorPrinter): boolean => p.state.status === "viejo" || p.state.status === "generico";

/** Las impresoras de una marca que se pueden actualizar en lote (las de servidor no: se hace en el servidor). */
export function batchTargets(printers: VendorPrinter[], vendor: VendorKey): VendorPrinter[] {
  return printers.filter((p) => p.vendor === vendor && needsUpdate(p) && p.connection.kind !== "servidor");
}

/** «Actualizar las RICOH» solo sale si hay más de una con el driver viejo o genérico. */
export const offersBatch = (printers: VendorPrinter[], vendor: VendorKey): boolean => batchTargets(printers, vendor).length > 1;

export interface BatchResult {
  printer: string;
  ok: boolean;
  /** Driver y versión de antes y de después, para el resumen. */
  from: string;
  to: string;
  /** Lo que pasó, en claro. */
  message: string;
  undoId?: string;
}

const driverLine = (name: string, version: string) => [name, version].filter(Boolean).join(" ").trim() || "sin driver";

export const describeDriver = driverLine;

/** El resumen para pegar en el ticket. */
export function ticketSummary(vendor: VendorKey, results: BatchResult[], host?: string): string {
  const brand = VENDOR_NAME[vendor] || "Impresoras";
  const done = results.filter((r) => r.ok);
  const failed = results.filter((r) => !r.ok);
  const lines: string[] = [];
  lines.push(`Drivers ${brand} actualizados${host ? ` en ${host}` : ""}: ${done.length} de ${results.length}.`);
  for (const r of done) lines.push(`- ${r.printer}: ${r.from} → ${r.to}`);
  for (const r of failed) lines.push(`- ${r.printer}: sin cambios. ${r.message}`);
  if (done.length > 0) lines.push("Copia del driver anterior guardada; se puede deshacer desde AdminOps.");
  return lines.join("\n");
}

// ---------- Pasos en vivo ----------

export interface StepView {
  step: string;
  index: number;
  state: VendorStepEvent["state"];
  label: string;
  message: string;
}

/** Los cinco pasos de actualizar un driver, con el nombre que se enseña. */
export const UPDATE_STEPS: { step: string; label: string }[] = [
  { step: "backup", label: "Copia del driver actual" },
  { step: "install", label: "Instalar el driver nuevo" },
  { step: "driver", label: "Registrar el driver en Windows" },
  { step: "assign", label: "Cambiar la impresora al driver nuevo" },
  { step: "test", label: "Página de prueba" },
];

/** Los tres de instalar una impresora detectada. */
export const INSTALL_STEPS: { step: string; label: string }[] = [
  { step: "install", label: "Instalar el driver" },
  { step: "driver", label: "Registrar el driver en Windows" },
  { step: "queue", label: "Comprobar que aparece la impresora" },
];

/** Aplica un evento de paso a la lista (los de otra impresora no cuentan). */
export function applyStep(steps: StepView[], ev: VendorStepEvent, printer: string, table = UPDATE_STEPS): StepView[] {
  if (ev.printer !== printer) return steps;
  const label = table.find((t) => t.step === ev.step)?.label ?? ev.step;
  const next = steps.filter((s) => s.step !== ev.step);
  next.push({ step: ev.step, index: ev.index, state: ev.state, label, message: ev.state === "running" ? "" : ev.message });
  return next.sort((a, b) => a.index - b.index);
}

// ---------- Mensajes ----------

/** ¿El error dice que falta ser administrador? */
export const isAdminError = (message: string): boolean => /administrador/i.test(message) && !/modo auditor/i.test(message);

/** ¿El error es el del modo auditoría? */
export const isAuditError = (message: string): boolean => /modo auditor/i.test(message);

export function bytesText(n: number): string {
  if (n >= 1024 ** 3) return `${(n / 1024 ** 3).toFixed(1).replace(".", ",")} GB`;
  if (n >= 1024 ** 2) return `${Math.round(n / 1024 ** 2)} MB`;
  return `${Math.max(1, Math.round(n / 1024))} KB`;
}

export const pagesText = (n: number | null | undefined): string => (n == null ? "" : `${n.toLocaleString("es")} páginas`);

/** Fecha de un archivo del motor (segundos desde 1970) en corto. */
export const dayText = (secs: number | null | undefined): string => (secs ? new Date(secs * 1000).toLocaleString("es", { day: "numeric", month: "short", hour: "2-digit", minute: "2-digit" }) : "nunca");

// ---------- Descargas que llegan ----------

const norm = (s: string) => s.toLowerCase().replace(/[^a-z0-9]/g, "");

/** ¿El nombre de un modelo del paquete corresponde a este modelo? (sin marca, sin «Series») */
function sameModel(a: string, b: string): boolean {
  const strip = (s: string) => norm(s).replace(/series$/, "").replace(/^(ricoh|epson)/, "");
  const x = strip(a);
  const y = strip(b);
  if (x.length < 3 || y.length < 3) return false;
  return x === y || x.includes(y) || y.includes(x);
}

/** Impresoras (instaladas o por instalar) que cubre un paquete recién bajado. */
export function coveredBy(ev: VendorDownloadEvent, printers: VendorPrinter[], pending: VendorDetected[]): { printers: VendorPrinter[]; pending: VendorDetected[] } {
  if (!ev.readable || ev.models.length === 0) return { printers: [], pending: [] };
  const hit = (vendor: VendorKey, model: string) => vendor === ev.vendor && ev.models.some((m) => sameModel(m, model));
  return {
    printers: printers.filter((p) => hit(p.vendor, p.model)),
    pending: pending.filter((d) => hit(d.vendor, d.model)),
  };
}

/** Nombre que se enseña de un paquete que llegó. */
export function downloadTitle(ev: VendorDownloadEvent): string {
  const bits = [ev.name];
  if (ev.version) bits.push(`versión ${ev.version}`);
  if (ev.models.length) bits.push(`cubre ${ev.models.slice(0, 2).join(", ")}${ev.models.length > 2 ? ` y ${ev.models.length - 2} más` : ""}`);
  else if (!ev.readable) bits.push("instalador del fabricante");
  return bits.join(" · ");
}

// ---------- Programas de escaneo ----------

const SOFT_PATTERN: Record<string, RegExp> = {
  epsonscan2: /epsonscan|es2|escan2/,
  "paperstream-capture": /paperstream.?capture|psc/i,
  "paperstream-ip": /paperstream.?ip|psip|twain/i,
  "scansnap-home": /scansnap/,
};

/** El instalador de la biblioteca que corresponde a un programa de escaneo, si lo hay. */
export function installerFor(soft: ScanSoftware, installers: LibInstaller[]): LibInstaller | null {
  const re = SOFT_PATTERN[soft.id];
  if (!re) return null;
  return installers.find((i) => re.test(norm(i.name))) ?? null;
}

/** Programas que hacen falta según las marcas de los escáneres conectados. */
export function missingSoftware(software: ScanSoftware[], vendors: VendorKey[]): ScanSoftware[] {
  return software.filter((s) => !s.installed && vendors.includes(s.vendor as VendorKey));
}
