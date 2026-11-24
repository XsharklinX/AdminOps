// Impresoras → Escáneres → Escanear a carpeta: los textos y la lógica del asistente. Lo que cambia el
// equipo (carpeta, usuario, recurso compartido, cortafuegos) y lo que vigila está en Rust (scanfolder.rs).
import type { ScanDiagnosis, ScanSetup, ScanStep, ScanWatch, VendorPrinter } from "./api";
import { ago } from "./connected";

/** Las impresoras que sabe escanear a carpeta con este asistente: RICOH y EPSON con IP en su puerto. */
export interface PrinterChoice {
  /** Para la lista: «RICOH IM C3000 · 10.0.0.41». */
  label: string;
  vendor: "ricoh" | "epson";
  /** «RICOH IM C3000», sin repetir la marca. */
  model: string;
  ip: string;
}

/** «RICOH» + «RICOH IM C3000» → «RICOH IM C3000»; «EPSON» + «WF-C5790» → «EPSON WF-C5790». */
export function modelName(brand: string, model: string): string {
  const b = brand.trim();
  const m = model.trim();
  if (!m) return b;
  if (!b || m.toLowerCase().startsWith(b.toLowerCase())) return m;
  return `${b} ${m}`;
}

const BRAND = { ricoh: "RICOH", epson: "EPSON" } as const;

/** Las RICOH y EPSON que AdminOps conoce con la IP de su puerto, sin repetir la IP. */
export function pickPrinters(printers: VendorPrinter[]): PrinterChoice[] {
  const out: PrinterChoice[] = [];
  const seen = new Set<string>();
  for (const p of printers) {
    const ip = p.connection.ip?.trim();
    if (!ip || (p.vendor !== "ricoh" && p.vendor !== "epson") || seen.has(ip)) continue;
    seen.add(ip);
    const model = modelName(BRAND[p.vendor], p.model || p.name);
    out.push({ label: `${model} · ${ip}`, vendor: p.vendor, model, ip });
  }
  return out;
}

/** Una IPv4 escrita a mano. */
export function isIpv4(s: string): boolean {
  const m = /^(\d{1,3})\.(\d{1,3})\.(\d{1,3})\.(\d{1,3})$/.exec(s.trim());
  return !!m && m.slice(1).every((n) => Number(n) <= 255);
}

/** «Escanear a carpeta · RICOH IM C3000 · 10.0.0.41» (sin la marca ni el modelo si se escribió la IP a mano). */
export function headerText(model: string | null, ip: string): string {
  return ["Escanear a carpeta", model?.trim() || null, ip.trim() || null].filter(Boolean).join(" · ");
}

/** «Llegó escaneo_0012.pdf · 2 páginas · hace 3 s». */
export function arrivedText(file: string, pages: number | null, arrivedMs: number, now: number): string {
  const parts = [`Llegó ${file}`];
  if (pages && pages > 0) parts.push(pages === 1 ? "1 página" : `${pages} páginas`);
  parts.push(ago(now - arrivedMs));
  return parts.join(" · ");
}

/** «Para dictarla: «Mesa, guion, Verde, guion, cuatro, dos».» */
export function dictationText(speech: string): string {
  return `Para dictarla: «${speech}».`;
}

/** Lo que va en los tres campos del panel de la impresora. */
export interface PanelField {
  id: "path" | "user" | "password";
  label: string;
  value: string;
}

export function panelFields(s: Pick<ScanSetup, "path" | "user" | "password">): PanelField[] {
  return [
    { id: "path", label: "Ruta", value: s.path },
    { id: "user", label: "Usuario", value: s.user },
    { id: "password", label: "Contraseña", value: s.password },
  ];
}

/** «o con la IP: \\10.0.0.23\Escaneos». */
export function ipPathText(ipPath: string | null): string | null {
  return ipPath ? `o con la IP: ${ipPath}` : null;
}

/** Las impresoras viejas suelen ir mejor con la IP que con el nombre del equipo. */
export const OLD_PRINTER_HINT = "Las impresoras viejas suelen ir mejor con la IP que con el nombre del equipo.";

const SETUP_PARTS = ["folder", "share", "firewall"];

/**
 * Los pasos como se enseñan: si la carpeta, el usuario y el cortafuegos salieron bien, una sola línea
 * («Carpeta, usuario y cortafuegos»); si alguno avisó o falló, cada uno con su frase.
 */
export function collapseSteps(steps: ScanStep[]): ScanStep[] {
  const parts = SETUP_PARTS.map((id) => steps.find((s) => s.id === id));
  const rest = steps.filter((s) => !SETUP_PARTS.includes(s.id));
  if (parts.every((s) => s?.state === "ok")) return [{ id: "setup", label: "Carpeta, usuario y cortafuegos", state: "ok", detail: null }, ...rest];
  return [...parts.filter((s): s is ScanStep => !!s), ...rest];
}

/** ¿Está todo listo para esperar el escaneo? Ningún paso falló. */
export function setupWorked(steps: ScanStep[]): boolean {
  return steps.length > 0 && steps.every((s) => s.state !== "error" && s.state !== "running");
}

/** El texto de espera según cómo va. */
export function watchLine(w: ScanWatch | null, now: number): string {
  if (!w || w.state === "idle") return "Sin esperar ningún escaneo.";
  if (w.state === "waiting") return "Esperando un escaneo de prueba…";
  if (w.state === "arrived") return arrivedText(w.file ?? "un archivo", w.pages, w.arrivedMs ?? now, now);
  return "Pasaron 10 minutos y no llegó ningún escaneo.";
}

/** ¿Se ofrece quitar la firma obligatoria? Solo si es la causa probable. */
export function offersSigning(d: ScanDiagnosis | null): boolean {
  return !!d && d.kind === "signing" && d.signingRequired;
}

/** Segundos que lleva esperando, para enseñar «Esperando… 1:25». */
export function waitedText(startedMs: number, now: number): string {
  const s = Math.max(0, Math.floor((now - startedMs) / 1000));
  return `${Math.floor(s / 60)}:${String(s % 60).padStart(2, "0")}`;
}
