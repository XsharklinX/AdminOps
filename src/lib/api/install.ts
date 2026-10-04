// Instalar programas y red avanzada.
import { invoke } from "./core";

// ---------- Instalar programas y red avanzada (v0.12) ----------

export interface CatalogApp {
  id: string;
  name: string;
  category: string;
  source: string;
  /** Uso personal u ocio: no sale en la vista «Empresa». */
  home?: boolean;
}

/** Qué enseña el catálogo de programas: lo decide el técnico. */
export interface CatalogView {
  /** Solo lo que se usa en una empresa. */
  business: boolean;
  hiddenApps: string[];
  hiddenCategories: string[];
}

export interface AppList {
  id: string;
  name: string;
  description?: string;
  apps: CatalogApp[];
}

export interface AppCatalog {
  apps: CatalogApp[];
  presets: { id: string; name: string; description: string; apps: string[] }[];
  lists: AppList[];
  view: CatalogView;
}

export interface InstallResult {
  id: string;
  name: string;
  ok: boolean;
  message: string;
}

export interface Preflight {
  online: boolean;
  free: number;
  lowSpace: boolean;
  winget: boolean;
  elevated: boolean;
  /** Avisos en claro, listos para mostrar. */
  warnings: string[];
}

export const appsApi = {
  /** Red, espacio, winget y permisos antes de una instalación larga. */
  preflight: (count: number) => invoke<Preflight>("install_preflight", { count }),
  catalog: () => invoke<AppCatalog>("app_catalog"),
  setCatalogView: (view: CatalogView) => invoke<void>("set_catalog_view", { view }),
  installed: () => invoke<string[]>("installed_apps"),
  search: (query: string) => invoke<(CatalogApp & { version: string })[]>("search_apps", { query }),
  install: (apps: CatalogApp[]) => invoke<InstallResult[]>("install_apps", { apps }),
  saveList: (list: AppList) => invoke<AppList>("save_app_list", { list }),
  deleteList: (id: string) => invoke<void>("delete_app_list", { id }),
};

export interface Probe {
  seq: number;
  from: string | null;
  ms: number | null;
  status: "ok" | "timeout" | "unreachable" | "error";
  reached: boolean;
}

export interface PortEntry {
  protocol: "TCP" | "UDP";
  localAddress: string;
  localPort: number;
  remoteAddress: string | null;
  remotePort: number | null;
  state: string;
  pid: number;
  process: string | null;
}

export interface DnsAdapter {
  index: number;
  name: string;
  description: string;
  virtual: boolean;
  dns: string[];
  manual: boolean;
}

export const netApi = {
  ping: (host: string, count: number) => invoke<{ target: string; ip: string }>("start_ping", { host, count }),
  trace: (host: string) => invoke<{ target: string; ip: string }>("start_trace", { host }),
  stop: () => invoke<void>("stop_probe"),
  ports: () => invoke<PortEntry[]>("list_ports"),
  dnsAdapters: () => invoke<DnsAdapter[]>("list_dns_adapters"),
  setDns: (index: number, servers: string[]) => invoke<void>("set_dns", { index, servers }),
  readHosts: () => invoke<string>("read_hosts"),
  saveHosts: (content: string) => invoke<void>("save_hosts", { content }),
};

export interface PrinterInfo {
  name: string;
  driver: string | null;
  port: string | null;
  default: boolean;
  network: boolean;
  shared: boolean;
  offline: boolean;
  status: number;
  error: string | null;
  jobs: number;
  virtual: boolean;
}

/** Qué le pasa a una impresora y qué hacer. */
export interface PrinterCheck {
  level: "ok" | "warn" | "bad";
  title: string;
  text: string;
  spooler: boolean;
  /** Dirección del puerto TCP/IP, si es de red. */
  host: string;
  /** Responde en la red (null: no es de red). */
  reachable: boolean | null;
  jobs: number;
  oldestJobMin: number;
  /** Lo que cuenta la propia impresora por SNMP (null si no contesta). */
  device: PrinterDevice | null;
}

export interface PrinterSupply {
  name: string;
  /** 0-100, o null si la impresora no lo sabe. */
  percent: number | null;
  /** Tóner o tinta (lo que se cambia a menudo). */
  consumable: boolean;
}

export interface PrinterDevice {
  reachable: boolean;
  name: string;
  model: string;
  pages: number | null;
  supplies: PrinterSupply[];
  /** Avisos del aparato, en español («Atasco de papel»). */
  alerts: string[];
}

/** Impresora vista en la red. */
export interface FoundPrinter {
  ip: string;
  /** Puertos de impresión abiertos: 9100 RAW, 631 IPP, 515 LPD. */
  ports: number[];
  installed: boolean;
  /** Nombre y modelo con que se anuncia ("" si no lo dice). */
  name: string;
  model: string;
  /** mdns · wsd · ports */
  via: string;
}

export const printersApi = {
  list: () => invoke<PrinterInfo[]>("list_printers"),
  check: (name: string) => invoke<PrinterCheck>("check_printer", { name }),
  findOnNetwork: () => invoke<FoundPrinter[]>("find_network_printers"),
  clearQueue: (name: string) => invoke<void>("clear_printer_queue", { name }),
  testPage: (name: string) => invoke<void>("print_test_page", { name }),
  setDefault: (name: string) => invoke<void>("set_default_printer", { name }),
  remove: (name: string) => invoke<void>("remove_printer", { name }),
};
