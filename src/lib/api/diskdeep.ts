// Discos a fondo: particiones y arranque, recuperación por firmas y clonado.
import { invoke } from "./core";

// ---------- Particiones ----------

export interface PartitionInfo {
  index: number;
  startLba: number;
  sectors: number;
  offset: number;
  size: number;
  kind: string;
  /** NTFS · FAT32 · exFAT · BitLocker… (lo que hay dentro) */
  fs: string;
  boot: boolean;
  typeGuid: string;
  name: string;
}

export interface PartitionLayout {
  /** gpt · mbr · none */
  scheme: string;
  sector: number;
  size: number;
  diskGuid: string;
  partitions: PartitionInfo[];
  free: { offset: number; size: number }[];
  issues: { level: "warn" | "bad"; text: string }[];
  bootable: boolean;
}

export interface FoundPartition {
  offset: number;
  size: number;
  fs: string;
  /** alta · media · baja */
  confidence: string;
  detail: string;
  label: string;
}

export interface TableBackup {
  path: string;
  created: number;
  scheme: string;
  size: number;
}

export const partitionsApi = {
  layout: (number: number) => invoke<PartitionLayout>("partition_layout", { number }),
  backup: (number: number, model: string) => invoke<TableBackup>("partition_backup", { number, model }),
  backups: (number: number) => invoke<TableBackup[]>("partition_backups", { number }),
  restoreTable: (path: string, number: number) => invoke<PartitionLayout>("partition_table_restore", { path, number }),
  /** Tarea «part-scan:N». */
  findLost: (number: number) => invoke<FoundPartition[]>("partition_find_lost", { number }),
  restore: (number: number, model: string, found: FoundPartition) => invoke<PartitionLayout>("partition_restore", { number, model, found }),
  /** bcdboot sobre la instalación de Windows de esa unidad. */
  repairBoot: (letter: string) => invoke<string>("boot_repair", { letter }),
};

// ---------- Recuperación por firmas ----------

export interface CarveSource {
  /** disk · image */
  kind: "disk" | "image";
  number: number;
  path: string;
  offset: number;
  length: number;
}

export interface CarveItem {
  id: number;
  ext: string;
  /** photos · documents · videos · music · archives · databases */
  group: string;
  label: string;
  offset: number;
  length: number;
  /** full · partial */
  quality: "full" | "partial";
}

export interface RecoverSummary {
  folder: string;
  copied: number;
  bytes: number;
  failed: number;
}

export const carveApi = {
  /** Tarea «carve:disk:N» o «carve:image:RUTA». */
  scan: (source: CarveSource, groups: string[]) => invoke<CarveItem[]>("carve_scan", { source, groups }),
  found: (source: CarveSource) => invoke<CarveItem[]>("carve_found", { source }),
  preview: (source: CarveSource, id: number) => invoke<string | null>("carve_preview", { source, id }),
  recover: (source: CarveSource, ids: number[], dest: string) => invoke<RecoverSummary>("carve_recover", { source, ids, dest }),
};

export const carveTask = (s: CarveSource) => `carve:${s.kind === "image" ? `image:${s.path}` : `disk:${s.number}`}`;

// ---------- Clonado ----------

export interface CloneTarget {
  kind: "image" | "disk";
  path: string;
  number: number;
}

export interface CloneLive {
  cells: string;
  percent: number;
  pass: number;
  good: number;
  lost: number;
  pending: number;
  mbps: number;
  etaSecs: number;
}

export interface CloneResult {
  finished: boolean;
  cells: string;
  size: number;
  good: number;
  lost: number;
  pending: number;
  lostRanges: number;
  level: "ok" | "warn" | "bad";
  text: string;
  target: string;
}

export const cloneApi = {
  /** Tarea «disk-clone:N». Solo lee el origen. */
  start: (number: number, target: CloneTarget, retries: number, resume: boolean) => invoke<CloneResult>("disk_clone", { number, target, retries, resume }),
  live: (number: number) => invoke<CloneLive | null>("disk_clone_live", { number }),
  last: (number: number) => invoke<CloneResult | null>("disk_clone_last", { number }),
};
