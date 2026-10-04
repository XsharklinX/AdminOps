// Perfiles, datos de la app y el propio AdminOps.
import { invoke } from "./core";
import { type Risk, type TweakStatus } from "./system";

// ---------- Perfiles y datos de la app (Fase 5) ----------

export interface ProfileItem {
  id: string;
  name: string;
  kind: "toggle" | "action";
  risk: Risk;
  status: TweakStatus;
  supported: boolean;
  hasBackup: boolean;
}

export interface ProfileView {
  custom: boolean;
  id: string;
  name: string;
  icon: string;
  description: string;
  items: ProfileItem[];
  needsAdmin: boolean;
}

export interface ProfileResult {
  results: {
    id: string;
    name: string;
    outcome: "applied" | "reverted" | "ran" | "skipped" | "failed";
    message: string | null;
  }[];
  restorePointCreated: boolean;
  reboot: boolean;
}

export interface AppInfo {
  version: string;
  portable: boolean;
  dataDir: string;
  reportsDir: string;
  startPage: string | null;
}

export const profilesApi = {
  list: () => invoke<ProfileView[]>("list_profiles"),
  apply: (id: string, skipRestorePoint = false) => invoke<ProfileResult>("apply_profile", { id, skipRestorePoint }),
  revert: (id: string) => invoke<ProfileResult>("revert_profile", { id }),
};

export interface StartupTiming {
  steps: { name: string; atMs: number; ms: number }[];
  /** ms desde que arrancó el proceso hasta la llamada. */
  nowMs: number;
}

export const appApi = {
  info: () => invoke<AppInfo>("get_app_info"),
  /** Pasos del arranque del programa (Ajustes → Rendimiento). */
  startupTiming: () => invoke<StartupTiming>("startup_timing"),
  logTiming: (summary: string) => invoke<void>("log_timing", { summary }),
  /** La interfaz ya está pintada: el programa enseña la ventana. */
  uiReady: () => invoke<void>("ui_ready").catch(() => {}),
  /** El código de la interfaz arrancó (aún sin pintar): WebView2 está vivo. */
  uiBooting: () => invoke<void>("ui_booting").catch(() => {}),
  cancelTask: (task: string) => invoke<boolean>("cancel_task", { task }),
  readLog: (lines = 400) => invoke<string>("read_log", { lines }),
  supportPackage: () => invoke<string>("support_package"),
  /** Prepara el correo para el autor con la descripción y el paquete de soporte. `manual`: sin adjunto (correo web). */
  reportProblem: (what: string, steps: string, contact: string, manual: boolean) => invoke<void>("report_problem", { what, steps, contact, manual }),
  /** Los términos de uso, en texto plano. */
  terms: () => invoke<string>("terms_of_use"),
  logError: (message: string) => invoke<void>("log_frontend_error", { message }).catch(() => {}),
  openLogsFolder: () => invoke<void>("open_logs_folder"),
  openFolder: (kind: "data" | "reports" | "logs") => invoke<void>("open_app_folder", { kind }),
};
