// Caja fuerte, borrado, recuperación y control parental.
import { invoke } from "./core";

// ---------- Caja fuerte, borrado, recuperación y control parental (v0.21) ----------

export interface VaultSupport {
  bitlocker: boolean;
  edition: string;
  elevated: boolean;
}

export interface VaultStatus {
  id: string;
  name: string;
  path: string;
  sizeGb: number;
  created: number;
  exists: boolean;
  mounted: boolean;
  letter: string | null;
  unlocked: boolean;
  fileSize: number;
}

export interface VaultCreated {
  id: string;
  letter: string;
  recovery: string;
}

export interface ZipOutcome {
  path: string;
  files: number;
  bytes: number;
}

export interface WipeOutcome {
  files: number;
  bytes: number;
  failed: string[];
}

export interface RecoverDrive {
  letter: string;
  label: string;
  fs: string;
  size: number;
  free: number;
  kind: string;
}

export interface UserHours {
  name: string;
  fullName: string;
  admin: boolean;
  microsoft: boolean;
  isSelf: boolean;
  hours: boolean[][];
  restricted: boolean;
}

export const vaultApi = {
  support: () => invoke<VaultSupport>("vault_support"),
  list: () => invoke<VaultStatus[]>("vault_list"),
  pickLocation: (name: string) => invoke<string | null>("vault_pick_location", { name }),
  create: (name: string, path: string, sizeGb: number, password: string) => invoke<VaultCreated>("vault_create", { name, path, sizeGb, password }),
  open: (id: string, password: string) => invoke<string>("vault_open", { id, password }),
  close: (id: string) => invoke<void>("vault_close", { id }),
  changePassword: (id: string, old: string, next: string) => invoke<void>("vault_change_password", { id, old, new: next }),
  recoveryKey: (id: string, password: string) => invoke<string>("vault_recovery_key", { id, password }),
  saveRecovery: (name: string, key: string) => invoke<string | null>("vault_save_recovery", { name, key }),
  addExisting: () => invoke<VaultStatus | null>("vault_add_existing"),
  remove: (id: string, deleteFile: boolean) => invoke<void>("vault_remove", { id, deleteFile }),
  pickFolder: () => invoke<string | null>("pick_folder"),
  pickZip: () => invoke<string | null>("pick_encrypted_zip"),
  encryptFolder: (folder: string, password: string, deleteOriginal: boolean) => invoke<ZipOutcome>("encrypt_folder", { folder, password, deleteOriginal }),
  decrypt: (path: string, password: string) => invoke<ZipOutcome>("decrypt_archive", { path, password }),
};

export const wipeApi = {
  pick: (folders: boolean) => invoke<string[]>("wipe_pick", { folders }),
  items: (paths: string[]) => invoke<WipeOutcome>("wipe_items", { paths }),
  freeSpace: (drive: string) => invoke<void>("wipe_free_space", { drive }),
  openSettings: (uri: string) => invoke<void>("open_ms_settings", { uri }),
};

export const recoverApi = {
  status: () => invoke<{ installed: boolean; drives: RecoverDrive[] }>("recover_status"),
  install: () => invoke<void>("recover_install"),
  run: (source: string, dest: string, extensive: boolean, kinds: string[], folder: string) =>
    invoke<{ folder: string; files: number }>("recover_run", { source, dest, extensive, kinds, folder }),
};

export const familyApi = {
  filterStatus: () => invoke<{ active: string; adapters: string[] }>("dns_filter_status"),
  setFilter: (filter: string) => invoke<void>("set_dns_filter", { filter }),
  blocked: () => invoke<string[]>("blocked_sites"),
  setBlocked: (sites: string[]) => invoke<string[]>("set_blocked_sites", { sites }),
  hours: () => invoke<UserHours[]>("logon_hours"),
  setHours: (name: string, hours: boolean[][]) => invoke<void>("set_logon_hours", { name, hours }),
};
