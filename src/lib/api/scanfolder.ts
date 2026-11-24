import { invoke } from "./core";

// ---------- Escanear a carpeta (Impresoras → Escáneres) ----------

export interface ScanStatus {
  isAdmin: boolean;
  computer: string;
  folder: string;
  folderExists: boolean;
  userExists: boolean;
  shareExists: boolean;
  /** El recurso compartido apunta a la carpeta de AdminOps. */
  shareOk: boolean;
  /** `Private`, `Public` o `DomainAuthenticated`. */
  networkCategory: string | null;
  networkName: string | null;
  localIp: string | null;
  sameSubnet: boolean | null;
  requireSigning: boolean | null;
  smb1Enabled: boolean | null;
  passwordMin: number | null;
  /** La contraseña más larga que genera AdminOps. */
  passwordMax: number;
  passwordComplexity: boolean | null;
  firewallOff: number | null;
}

export type StepState = "running" | "ok" | "warn" | "error";

export interface ScanStep {
  id: string;
  label: string;
  state: StepState;
  detail: string | null;
}

export interface ScanSetup {
  /** `\NOMBRE-DEL-EQUIPO\Escaneos`. */
  path: string;
  /** `\10.0.0.23\Escaneos`. */
  ipPath: string | null;
  user: string;
  password: string;
  /** «Mesa, guion, Verde, guion, cuatro, dos». */
  speech: string;
  entryId: number;
  steps: ScanStep[];
  /** La red es pública: el cortafuegos no se abrió. */
  publicNetwork: boolean;
  folderHadFiles: boolean;
}

export interface ScanDiagnosis {
  kind: "auth" | "smb1" | "signing" | "session" | "nothing" | "unknown";
  sentence: string;
  details: string[];
  /** La firma SMB obligatoria está puesta: se puede ofrecer quitarla. */
  signingRequired: boolean;
}

export interface ScanWatch {
  running: boolean;
  /** `idle`, `waiting`, `arrived` o `timeout`. */
  state: "idle" | "waiting" | "arrived" | "timeout";
  startedMs: number;
  file: string | null;
  pages: number | null;
  arrivedMs: number | null;
  diagnosis: ScanDiagnosis | null;
}

export const scanFolderApi = {
  status: (ip: string | null) => invoke<ScanStatus>("scanfolder_status", { ip }),
  /** Pide administrador. Cada paso llega también como evento `scanfolder-step`. */
  setup: (ip: string) => invoke<ScanSetup>("scanfolder_setup", { ip }),
  newPassword: () => invoke<{ password: string; speech: string; user: string }>("scanfolder_new_password"),
  privateNetwork: (ip: string) => invoke<string>("scanfolder_private_network", { ip }),
  disableSigning: () => invoke<string>("scanfolder_disable_signing"),
  watchStart: (ip: string) => invoke<ScanWatch>("scanfolder_watch_start", { ip }),
  watchStatus: () => invoke<ScanWatch>("scanfolder_watch_status"),
  watchStop: () => invoke<void>("scanfolder_watch_stop"),
  diagnose: (ip: string, sinceMs?: number) => invoke<ScanDiagnosis>("scanfolder_diagnose", { ip, sinceMs: sinceMs ?? null }),
  openFolder: () => invoke<void>("scanfolder_open_folder"),
  desktopShortcut: () => invoke<string>("scanfolder_desktop_shortcut"),
};
