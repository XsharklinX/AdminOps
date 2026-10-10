// Bloqueo, copia de la configuración, cuidado de la app y errores de Windows.
import { invoke } from "./core";

// ---------- Bloqueo de la app y copia de la configuración (v0.23) ----------

export interface LockStatus {
  enabled: boolean;
  kind: "pin" | "password" | "";
  idleMinutes: number;
  waitSecs: number;
}

export const lockApi = {
  status: () => invoke<LockStatus>("lock_status"),
  verify: (secret: string) => invoke<boolean>("lock_verify", { secret }),
  verifyWindows: (password: string) => invoke<boolean>("lock_verify_windows", { password }),
  set: (kind: "pin" | "password", secret: string, idleMinutes: number, current: string) => invoke<void>("lock_set", { kind, secret, idleMinutes, current }),
  setIdle: (idleMinutes: number, current: string) => invoke<void>("lock_set_idle", { idleMinutes, current }),
  disable: (current: string) => invoke<void>("lock_disable", { current }),
};

export const configApi = {
  export: (prefs: unknown) => invoke<string | null>("export_config", { prefs }),
  import: () => invoke<unknown | null>("import_config"),
};

// ---------- Cuidado de la app (v1.0) ----------

export interface DataUsage {
  journalEntries: number;
  snapshots: number;
  snapshotsBytes: number;
  reports: number;
  reportsBytes: number;
  speedtests: number;
  logsBytes: number;
}

export interface UpdateInfo {
  current: string;
  latest: string;
  newer: boolean;
  url: string;
  notes: string;
  /** Hay un archivo publicado que AdminOps puede descargar e instalar en este equipo. */
  installable: boolean;
  size: number;
  /** Hay alguna versión publicada en GitHub. */
  published: boolean;
}

export const appcareApi = {
  autostart: () => invoke<boolean>("autostart_enabled"),
  setAutostart: (enabled: boolean) => invoke<void>("set_autostart", { enabled }),
  usage: () => invoke<DataUsage>("data_usage"),
  cleanup: (months: number, journal: boolean, snapshots: boolean, reports: boolean) =>
    invoke<{ journal: number; snapshots: number; reports: number; freed: number }>("data_cleanup", { months, journal, snapshots, reports }),
  checkUpdate: () => invoke<UpdateInfo>("check_update"),
  /** Descarga la versión nueva y abre su instalador ("installer") o enseña el .zip ("portable"), comprobando su huella SHA-256. */
  installUpdate: () => invoke<{ how: "installer" | "portable"; verified: "both" | "one" | "none"; sha256: string }>("install_update"),
  openRelease: (url: string) => invoke<void>("open_release_page", { url }),
};

// ---------- Errores de Windows (vigilancia) ----------

export interface WindowsAlert {
  id: string;
  time: number;
  key: string;
  level: "bad" | "warn" | "info";
  title: string;
  detail: string;
  explanation: string;
  advice: string;
  page: string | null;
  count: number;
  read: boolean;
}

/** Un aviso silenciado en este equipo. */
export interface MutedAlert {
  key: string;
  title: string;
  detail: string;
  at: number;
}

export const alertsApi = {
  list: () => invoke<WindowsAlert[]>("list_windows_alerts"),
  markRead: () => invoke<void>("mark_windows_alerts_read"),
  clear: () => invoke<void>("clear_windows_alerts"),
  dismiss: (id: string) => invoke<void>("dismiss_windows_alert", { id }),
  mute: (id: string) => invoke<void>("mute_windows_alert", { id }),
  unmute: (key: string) => invoke<void>("unmute_windows_alert", { key }),
  muted: () => invoke<MutedAlert[]>("muted_windows_alerts"),
  checkNow: () => invoke<WindowsAlert[]>("check_windows_now"),
};
