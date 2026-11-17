// Impresoras y escáneres RICOH y EPSON: marca y modelo, estado del driver,
// biblioteca de drivers, actualizar con vuelta atrás y probar el escaneo.
// El motor está en src-tauri/src/printvendor.rs; los nombres de los campos son los suyos.
import { listen } from "@tauri-apps/api/event";
import { invoke } from "./core";

export type VendorKey = "ricoh" | "epson" | "otra";

/** El aparato, tal cual lo devuelve el inventario: se pasa sin tocar a `driver_candidates`. */
export interface VendorDeviceRef {
  kind: string;
  name: string;
  vendor: VendorKey;
  brand: string;
  model: string;
  hardwareIds: string[];
  driverName: string;
  driverVersion: string;
  driverManufacturer: string;
}

export type DriverStatus = "alDia" | "viejo" | "generico" | "falta" | "otraMarca" | "sinComparar";

export interface VendorDriverState {
  status: DriverStatus;
  /** Texto corto del chip. */
  label: string;
  /** El motivo, para la ficha que sale al pasar el ratón. */
  reason: string;
  newVersion: string | null;
  /** «Windows Update» o «la biblioteca». */
  source: string | null;
}

export interface VendorDriver {
  name: string;
  version: string;
  date: string;
  manufacturer: string;
  inf: string;
  /** fabricante | generico | otraMarca | ninguno */
  kind: string;
}

export interface VendorConnection {
  /** usb | red | wsd | servidor | otra */
  kind: string;
  ip: string | null;
  port: string;
}

export interface VendorPrinter {
  name: string;
  vendor: VendorKey;
  brand: string;
  model: string;
  connection: VendorConnection;
  driver: VendorDriver;
  default: boolean;
  shared: boolean;
  hardwareIds: string[];
  snmp: { model: string; serial: string; pages: number | null } | null;
  state: VendorDriverState;
  device: VendorDeviceRef;
}

/** Aparato conectado al que le falta el driver. */
export interface VendorDetected {
  name: string;
  instanceId: string;
  class: string;
  problem: number;
  vendor: VendorKey;
  model: string;
  usbId: string;
  hardwareIds: string[];
  device: VendorDeviceRef;
}

export interface VendorPrinterInventory {
  printers: VendorPrinter[];
  notInstalled: VendorDetected[];
  windowsUpdateAt: number | null;
}

export interface VendorScanner {
  name: string;
  vendor: VendorKey;
  model: string;
  instanceId: string;
  hardwareIds: string[];
  /** funciona | falta */
  health: string;
  problem: number;
  driver: VendorDriver | null;
  interfaces: string[];
  state: VendorDriverState;
  device: VendorDeviceRef;
}

export interface ScanSoftware {
  /** epsonscan2 | paperstream-capture | paperstream-ip | scansnap-home */
  id: string;
  name: string;
  vendor: string;
  installed: boolean;
  version: string;
}

export interface VendorScannerInventory {
  scanners: VendorScanner[];
  notInstalled: VendorDetected[];
  software: ScanSoftware[];
  windowsUpdateAt: number | null;
}

export interface LibDriver {
  id: string;
  package: string;
  file: string;
  relPath: string;
  class: string;
  provider: string;
  vendor: VendorKey;
  version: string;
  date: string;
  models: string[];
  modelCount: number;
}

export interface LibInstaller {
  id: string;
  name: string;
  relPath: string;
  size: number;
}

export interface DriverLibrary {
  /** La carpeta elegida (con el usuario tapado); null si se usa la de por defecto. */
  customDir: string | null;
  isDefault: boolean;
  exists: boolean;
  scannedAt: number;
  drivers: LibDriver[];
  installers: LibInstaller[];
}

export interface LibAddResult {
  /** carpeta | zip | exeAbierto | instalador */
  kind: string;
  name: string;
  driversFound: number;
  message: string;
}

export interface DriverCandidate {
  id: string;
  /** biblioteca | windowsUpdate */
  source: string;
  title: string;
  version: string;
  date: string;
  model: string;
  /** hardwareId | modelo | mismoDriver | universal */
  matchedBy: string;
  comparable: boolean;
  newer: boolean | null;
  package: string;
}

export interface DriverCandidates {
  candidates: DriverCandidate[];
  state: VendorDriverState;
  windowsUpdate: { searched: boolean; fromCache: boolean; searchedAt: number | null; error: string | null; pendingTotal: number };
}

export interface DriverUpdateResult {
  undoId: string;
  printer: string;
  previousDriver: string;
  previousVersion: string;
  newDriver: string;
  newVersion: string;
  changed: boolean;
  testPageSent: boolean;
  rebootRequired: boolean;
  message: string;
}

export interface PrinterInstallResult {
  printer: string;
  createdQueue: boolean;
  driver: string;
  rebootRequired: boolean;
  message: string;
}

export interface InstallerResult {
  name: string;
  finished: boolean;
  exitCode: number | null;
  rebootRequired: boolean;
  message: string;
}

export interface VendorStepEvent {
  printer: string;
  /** backup | install | driver | assign | test | queue | run */
  step: string;
  index: number;
  total: number;
  state: "running" | "ok" | "skipped" | "error";
  message: string;
}

export interface VendorDownloadEvent {
  /** «dl:…»: se pasa a `driver_library_add` (no lleva ruta). */
  id: string;
  name: string;
  size: number;
  kind: "exe" | "zip";
  readable: boolean;
  version: string;
  provider: string;
  vendor: VendorKey;
  models: string[];
}

export interface VendorWatchEvent {
  state: "started" | "ended" | "stopped";
  minutes: number;
}

export const vendorApi = {
  // Solo lectura.
  printers: () => invoke<VendorPrinterInventory>("printer_inventory"),
  scanners: () => invoke<VendorScannerInventory>("scanner_inventory"),
  library: (rescan?: boolean) => invoke<DriverLibrary>("driver_library", { rescan }),
  candidates: (device: VendorDeviceRef, searchUpdate?: boolean) => invoke<DriverCandidates>("driver_candidates", { device, searchUpdate }),
  openVendorPage: (brand: string, model: string) => invoke<{ url: string; copied: boolean }>("open_vendor_page", { brand, model }),
  scanTest: (device: string) => invoke<{ dataUrl: string; bytes: number }>("scanner_test", { device }),
  // Cambian la biblioteca (no el equipo).
  libraryAdd: (path: string) => invoke<LibAddResult>("driver_library_add", { path }),
  setLibraryDir: (dir: string | null) => invoke<void>("set_driver_library_dir", { dir }),
  watchDownloads: (minutes: number) => invoke<void>("watch_downloads", { minutes }),
  // Cambian el equipo (piden administrador).
  update: (printer: string, candidate: string) => invoke<DriverUpdateResult>("printer_driver_update", { printer, candidate }),
  undo: (id: string) => invoke<string>("printer_driver_undo", { id }),
  install: (device: VendorDeviceRef, candidate: string) => invoke<PrinterInstallResult>("printer_install", { device, candidate }),
  runInstaller: (id: string) => invoke<InstallerResult>("run_vendor_installer", { id }),
};

/** Se suscribe a un evento del motor; devuelve cómo darse de baja. */
export function onVendorEvent<T>(name: "printvendor:step" | "printvendor:download" | "printvendor:watch", cb: (payload: T) => void): () => void {
  let off: (() => void) | null = null;
  let gone = false;
  listen<T>(name, (e) => cb(e.payload))
    .then((u) => {
      if (gone) u();
      else off = u;
    })
    .catch(() => {});
  return () => {
    gone = true;
    off?.();
  };
}
