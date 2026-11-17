// Diagnóstico de experto (1.3): por qué no responde un programa, quién tiene un
// archivo, el lector de errores y el usuario de prueba. Todo lo que cambia algo
// va por sus propios botones, con aviso.
import { invoke } from "./core";

export interface WaitReason {
  /** network · deadlock · antivirus · program · service · lock */
  kind: string;
  text: string;
  threads: number;
}

export interface NetTarget {
  /** «Z:» si es una unidad. */
  drive: string;
  unc: string;
  host: string;
  reachable: boolean;
  used: boolean;
}

export interface WaitReport {
  pid: number;
  name: string;
  hung: boolean;
  threads: number;
  readable: number;
  reasons: WaitReason[];
  nets: NetTarget[];
  connections: { remote: string; port: number; state: string }[];
  verdict: { kind: string; text: string };
  /** Unidad que se puede reconectar con un clic. */
  reconnect: string | null;
}

export interface Holder {
  pid: number;
  name: string;
  exe: string;
  /** app · service · explorer · system */
  kind: string;
  closable: boolean;
  note: string;
}

export interface NetHolder {
  fileId: string;
  user: string;
  client: string;
  path: string;
}

export interface HolderReport {
  path: string;
  /** file · folder · drive */
  kind: "file" | "folder" | "drive";
  local: Holder[];
  network: NetHolder[];
  netNote: string;
  scanned: number;
  truncated: boolean;
}

export interface DriveInfo {
  letter: string;
  /** removable · fixed · network · other */
  kind: string;
}

export interface CloseResult {
  closed: boolean;
  message: string;
}

export interface ErrorFound {
  raw: string;
  info: {
    code: string;
    name: string;
    area: string;
    what: string;
    why: string;
    todo: string;
    page: string;
  } | null;
}

export interface ErrorLens {
  text: string;
  codes: ErrorFound[];
}

export interface TestUser {
  name: string;
  /** Segundos Unix en los que se borra solo. */
  expires: number;
  hasProfile: boolean;
  signedIn: boolean;
}

export interface CreatedTestUser {
  name: string;
  password: string;
  expires: number;
}

export const diagnoseApi = {
  /** A qué espera un programa (solo mira). */
  whyNotResponding: (pid: number, name: string) => invoke<WaitReport>("why_not_responding", { pid, name }),
  holders: (path: string) => invoke<HolderReport>("file_holders", { path }),
  drives: () => invoke<DriveInfo[]>("holder_drives"),
  pick: (folder: boolean) => invoke<string | null>("holder_pick", { folder }),
  closeHolder: (pid: number, name: string, force: boolean) => invoke<CloseResult>("close_holder", { pid, name, force }),
  closeSmbFile: (fileId: string) => invoke<void>("close_smb_file", { fileId }),
  lensText: (text: string) => invoke<ErrorLens>("error_lens_text", { text }),
  /** Imagen pegada o soltada, en base64 (con o sin «data:…;base64,»). */
  lensImage: (data: string) => invoke<ErrorLens>("error_lens_image", { data }),
  lensFile: (path: string) => invoke<ErrorLens>("error_lens_file", { path }),
  lensClipboard: () => invoke<ErrorLens>("error_lens_clipboard"),
  testUser: () => invoke<TestUser | null>("test_user_status"),
  createTestUser: () => invoke<CreatedTestUser>("create_test_user"),
  deleteTestUser: () => invoke<void>("delete_test_user"),
  /** Apunta en el diario (y por tanto en el caso) qué cambio era el culpable. */
  bisectNote: (culprit: string, total: number, questions: number) => invoke<void>("bisect_note", { culprit, total, questions }),
};
