// La oficina: carpetas, impresoras, correo, portales…
import { invoke } from "./core";

// ---------- La oficina completa (v0.22) ----------

export interface WolAdapter {
  name: string;
  description: string;
  magicPacket: string;
  wired: boolean;
}

export interface RemoteStatus {
  rdpEnabled: boolean;
  rdpSupported: boolean;
  fastStartup: boolean;
  adapters: WolAdapter[];
  host: string;
  ip: string;
  mac: string;
}

export interface Share {
  name: string;
  path: string;
  description: string;
  access: ShareAccess[];
  openFiles: number;
  /** La carpeta compartida ya no existe en el disco. */
  missingPath: boolean;
  /** Se comparte con «Todos» pero los permisos del disco no dejan entrar. */
  ntfsBlocks: boolean;
  /** Lo que conviene revisar. */
  risks: ShareRisk[];
}

export interface ShareAccess {
  account: string;
  /** Full | Change | Read */
  right: string;
  allow: boolean;
  sid: string;
}

/** «Todos» con control total, un disco entero, carpetas personales o de Windows. */
export type ShareRisk = "everyone-full" | "whole-disk" | "profile" | "personal" | "system";
export type ShareRight = "Read" | "Change" | "Full";

/** Qué puede hacer de verdad una cuenta en una carpeta compartida, y qué se lo impide. */
export interface ShareExplain {
  verdict: "none" | "read" | "write";
  headline: string;
  shareRight: "none" | "read" | "change" | "full";
  diskRight: "none" | "read" | "write" | "unknown";
  findings: { level: "bad" | "warn" | "ok"; text: string; fix: "" | "sharing" | "permissions" }[];
}

export interface NetDrive {
  letter: string;
  path: string;
  host: string;
  /** El equipo que la sirve contesta ahora mismo. */
  reachable: boolean;
}

export interface NetDrives {
  drives: NetDrive[];
  /** Letras libres para conectar una unidad nueva. */
  free: string[];
}

export interface RemoteShare {
  name: string;
  kind: "folder" | "printer";
  remark: string;
}

export interface ShareSize {
  name: string;
  bytes: number;
  files: number;
  diskFree: number;
  diskTotal: number;
}

/** Copia diaria de una carpeta compartida a otra carpeta. */
export interface ShareBackup {
  share: string;
  dest: string;
  time: string;
  lastRun: string | null;
  lastResult: number | null;
  nextRun: string | null;
  running: boolean;
  /** La última copia terminó bien (null: aún no se ha hecho ninguna). */
  ok: boolean | null;
}

/** Archivo que alguien tiene abierto ahora mismo desde otro equipo. */
export interface OpenFile {
  name: string;
  user: string;
  locked: boolean;
}

export interface SharingStatus {
  shares: Share[];
  category: string;
  fileSharing: boolean;
  discovery: boolean;
  sessions: string[];
  open: OpenFile[];
  /** Nombre de este equipo en la red. */
  host: string;
}

export interface IpConflict {
  time: string;
  ip: string;
  mac: string;
}

export const officeApi = {
  wake: (mac: string) => invoke<void>("wake_on_lan", { mac }),
  remoteStatus: () => invoke<RemoteStatus>("remote_status"),
  enableWol: () => invoke<void>("enable_wake_on_lan"),
  setRdp: (enabled: boolean) => invoke<void>("set_remote_desktop", { enabled }),
  rdp: (host: string) => invoke<void>("open_remote_desktop", { host }),
  /** Asistencia remota de Windows (msra.exe). */
  remoteAssistance: () => invoke<void>("open_remote_assistance"),
  shares: () => invoke<SharingStatus>("list_shares"),
  createShare: (path: string, name: string, who: string, write: boolean) => invoke<void>("create_share", { path, name, who, write }),
  removeShare: (name: string) => invoke<void>("remove_share", { name }),
  enableSharing: () => invoke<void>("enable_file_sharing"),
  /** Da o cambia el acceso de una cuenta ("everyone", un usuario local o la cuenta completa). */
  shareGrant: (name: string, account: string, right: ShareRight) => invoke<void>("share_grant", { name, account, right }),
  shareRevoke: (name: string, account: string, deny: boolean) => invoke<void>("share_revoke", { name, account, deny }),
  shareExplain: (name: string, user: string) => invoke<ShareExplain>("share_explain", { name, user }),
  shareSizes: () => invoke<ShareSize[]>("share_sizes"),
  drives: () => invoke<NetDrives>("network_drives"),
  mapDrive: (letter: string, path: string) => invoke<void>("map_network_drive", { letter, path }),
  unmapDrive: (letter: string) => invoke<void>("unmap_network_drive", { letter }),
  reconnectDrive: (letter: string) => invoke<string>("reconnect_network_drive", { letter }),
  /** Abre una unidad o una ruta de red en el Explorador (Windows pide ahí las credenciales). */
  openNetworkPath: (path: string) => invoke<void>("open_network_path", { path }),
  remoteShares: (host: string) => invoke<RemoteShare[]>("remote_shares", { host }),
  shareBackups: () => invoke<ShareBackup[]>("share_backups"),
  setShareBackup: (name: string, dest: string, time: string) => invoke<void>("set_share_backup", { name, dest, time }),
  removeShareBackup: (name: string) => invoke<void>("remove_share_backup", { name }),
  runShareBackup: (name: string) => invoke<void>("run_share_backup", { name }),
  conflicts: () => invoke<IpConflict[]>("ip_conflicts"),
  exportCsv: (name: string, content: string) => invoke<string | null>("export_csv", { name, content }),
};

/** Filas → CSV con ";" (lo que espera Excel en español) y comillas donde haga falta. */
export function toCsv(rows: (string | number | null | undefined)[][]): string {
  const cell = (v: string | number | null | undefined) => {
    const t = v === null || v === undefined ? "" : String(v);
    return /[;"\n\r]/.test(t) ? `"${t.replace(/"/g, '""')}"` : t;
  };
  return rows.map((r) => r.map(cell).join(";")).join("\r\n");
}
