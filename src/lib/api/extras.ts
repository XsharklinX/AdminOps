// Ideas del banco de mejoras: etiquetas, valoración del equipo, copias de seguridad,
// reglas de alerta, repuestos y traductor de hallazgos.
import { invoke } from "./core";

// ---------- Etiquetas con QR ----------

export interface LabelInfo {
  host: string;
  owner: string;
  place: string;
  lastVisit: string;
  phone: string;
  extension: string;
}

export const labelsApi = {
  /** El QR (SVG) de la ficha. */
  qr: (info: LabelInfo) => invoke<string>("label_qr", { info }),
  /** Crea el PDF de etiquetas (cols × rows por hoja A4) y lo abre. */
  sheet: (labels: LabelInfo[], cols: number, rows: number) => invoke<string>("label_sheet", { labels, cols, rows }),
};

// ---------- ¿Reparar o cambiar? ----------

export interface LifeCosts {
  ssd: number;
  ram: number;
  battery: number;
  newPc: number;
  labor: number;
}

export const DEFAULT_COSTS: LifeCosts = { ssd: 55, ram: 35, battery: 60, newPc: 620, labor: 20 };

export interface LifeReport {
  /** keep · upgrade · replace */
  decision: "keep" | "upgrade" | "replace";
  headline: string;
  detail: string;
  parts: { name: string; score: number; text: string; limiter: boolean }[];
  upgrades: { label: string; cost: number }[];
  upgradeTotal: number;
  newCost: number;
  yearsLeft: number;
  clientText: string;
}

export const lifeApi = {
  report: (costs: LifeCosts) => invoke<LifeReport>("life_report", { costs }),
};

// ---------- Copias de seguridad ----------

export interface BackupSet {
  id: string;
  name: string;
  dest: string;
  sources: string[];
}

export interface BackupCheck {
  id: string;
  label: string;
  level: "ok" | "warn" | "bad";
  text: string;
}

export interface BackupStatus {
  id: string;
  name: string;
  dest: string;
  checked: number;
  level: "ok" | "warn" | "bad";
  newest: number | null;
  coverage: { source: string; found: number; total: number }[];
  checks: BackupCheck[];
}

export const backupsApi = {
  get: () => invoke<BackupSet[]>("backups_get"),
  status: () => invoke<BackupStatus[]>("backups_status"),
  defaults: () => invoke<string[]>("backups_defaults"),
  save: (sets: BackupSet[]) => invoke<BackupSet[]>("backups_save", { sets }),
  /** Tarea «backup-check». Una copia o todas; devuelve el estado de todas. */
  check: (id: string | null) => invoke<BackupStatus[]>("backups_check", { id }),
  /** Tarea «backup-run». Copia lo nuevo o cambiado, sin borrar nada. */
  run: (id: string) => invoke<string>("backups_run", { id }),
};
