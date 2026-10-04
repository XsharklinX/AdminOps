// Mantenimiento a fondo y seguridad.
import { invoke } from "./core";
import { type SoftwareUpdate } from "./tools";

// ---------- Mantenimiento a fondo (v0.15) ----------

export interface InstalledProgram {
  id: string;
  name: string;
  publisher: string | null;
  version: string | null;
  installed: string | null;
  size: number | null;
  installLocation: string | null;
  silent: boolean;
  /** MSI | Inno Setup | NSIS | Propio (vacío: solo con su asistente). */
  silentKind: string;
  repairable: boolean;
  /** Runtimes, redistribuibles, drivers. */
  component: boolean;
  orphan: boolean;
  perUser: boolean;
}

export interface Leftover {
  kind: "folder" | "shortcut" | "registry";
  path: string;
  size: number;
  files: number;
}

export const programsApi = {
  list: () => invoke<InstalledProgram[]>("list_programs"),
  uninstall: (id: string, silent: boolean) => invoke<{ removed: boolean; message: string; leftovers: Leftover[] }>("uninstall_program", { id, silent }),
  removeLeftovers: (id: string, paths: string[]) => invoke<number>("remove_leftovers", { id, paths }),
  removeOrphan: (id: string) => invoke<void>("remove_orphan_entry", { id }),
  scanLeftovers: (id: string) => invoke<Leftover[]>("scan_leftovers", { id }),
  repair: (id: string) => invoke<string>("repair_program", { id }),
};

export interface UpdateHistoryEntry {
  title: string;
  date: string | null;
  result: "ok" | "partial" | "failed" | "aborted" | "progress";
  code: string | null;
  explanation: string | null;
}

export interface PendingUpdate {
  id: string;
  title: string;
  kb: string | null;
  size: number;
  hidden: boolean;
  driver: boolean;
}

export const updateApi = {
  history: () => invoke<UpdateHistoryEntry[]>("update_history"),
  pauseState: () => invoke<{ pausedUntil: string | null }>("update_pause_state"),
  pause: (days: number) => invoke<void>("update_pause", { days }),
  pending: () => invoke<PendingUpdate[]>("pending_updates"),
  setHidden: (id: string, title: string, hidden: boolean) => invoke<void>("set_update_hidden", { id, title, hidden }),
};

export interface BootAnalysis {
  boots: { time: string; totalMs: number; mainMs: number; postMs: number }[];
  culprits: { kind: "app" | "driver" | "service" | "device" | "other"; name: string; times: number; avgDelayMs: number }[];
}

export interface MaintenanceSchedule {
  weeks: number;
  day: string;
  time: string;
  tasks: string[];
}

export const maintenanceApi = {
  boot: () => invoke<BootAnalysis>("boot_analysis"),
  restoreStorage: () => invoke<{ used: number; max: number; count: number; oldest: string | null; newest: string | null }>("restore_storage"),
  deleteOldRestorePoints: () => invoke<number>("delete_old_restore_points"),
  driverBackups: () => invoke<{ name: string; drivers: number; size: number }[]>("list_driver_backups"),
  restoreDrivers: (name: string) => invoke<string>("restore_drivers", { name }),
  schedule: () =>
    invoke<{
      enabled: boolean;
      schedule: MaintenanceSchedule;
      available: [string, string][];
      lastRun: string | null;
      lastResult: number | null;
      nextRun: string | null;
      portable: boolean;
    }>("maintenance_schedule"),
  setSchedule: (schedule: MaintenanceSchedule | null) => invoke<void>("set_maintenance_schedule", { schedule }),
};

// ---------- Seguridad (v0.16) ----------

export interface SecurityCheck {
  id: string;
  label: string;
  /** "info": un dato; no suma ni resta en la nota. */
  status: "ok" | "warn" | "bad" | "unknown" | "info";
  detail: string;
  weight: number;
  fix: { label: string; page: string | null; focus: string | null; tool: string | null } | null;
}

export interface SecurityAudit {
  score: number;
  checks: SecurityCheck[];
  vulnerable: SoftwareUpdate[];
}

export interface BitlockerVolume {
  drive: string;
  protection: number;
  conversion: number;
  percent: number;
  hasRecoveryKey: boolean;
}

export interface SuspiciousItem {
  kind: "task" | "service" | "startup" | "hosts";
  name: string;
  detail: string;
  reason: string;
  id: string | null;
}

export const securityApi = {
  audit: () => invoke<SecurityAudit>("security_audit"),
  bitlocker: () => invoke<BitlockerVolume[]>("bitlocker_status"),
  keys: () => invoke<{ drive: string; id: string; key: string }[]>("bitlocker_keys"),
  exportKeys: () => invoke<string | null>("bitlocker_export"),
  suspicious: () => invoke<SuspiciousItem[]>("suspicious_items"),
  disableTask: (id: string) => invoke<void>("disable_task", { id }),
  extensions: () => invoke<{ browser: string; profile: string; name: string; id: string; enabled: boolean }[]>("browser_extensions"),
};
