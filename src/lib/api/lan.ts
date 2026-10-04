// Mi red y sus dispositivos.
import { invoke } from "./core";
import { type Portal } from "./people";

// ---------- Mi red y dispositivos (v0.20) ----------

export interface LanInfo {
  adapter: string;
  description: string;
  index: number;
  wireless: boolean;
  linkSpeed: string;
  mac: string;
  ip: string;
  prefix: number;
  gateway: string;
  gatewayMac: string;
  dns: string[];
  network: string;
  category: string;
  dhcp: boolean;
  ssid: string | null;
  wifiPassword: string | null;
  wifiAuth: string;
  key: string;
}

export interface PublicIp {
  ip: string;
  city: string;
  region: string;
  country: string;
  org: string;
}

export interface RouterNote {
  level: "ok" | "info" | "warn" | "bad";
  text: string;
}

export interface RouterCheck {
  url: string;
  title: string;
  brand: string | null;
  defaultHint: string;
  openPorts: number[];
  nat: "normal" | "double" | "provider" | "cgnat" | "unknown";
  notes: RouterNote[];
}

export interface RouterProfile {
  key: string;
  name: string;
  url: string;
  username: string;
  password: string;
  notes: string;
  updated: number;
  locked: boolean;
}

export interface LanDevice {
  ip: string;
  mac: string;
  name: string;
  vendor: string;
  alias: string;
  gateway: boolean;
  thisPc: boolean;
  privateMac: boolean;
  ms: number | null;
  new: boolean;
  firstSeen: number;
  kind: string;
  manufacturer: string;
  model: string;
  friendly: string;
  os: string;
  services: string[];
  ports: number[];
  netbios: string;
}

export interface LanScan {
  key: string;
  devices: LanDevice[];
  truncated: boolean;
}

export const lanApi = {
  info: () => invoke<LanInfo | null>("lan_info"),
  publicIp: () => invoke<PublicIp>("public_ip"),
  check: (gateway: string, wifiAuth: string) => invoke<RouterCheck>("router_check", { gateway, wifiAuth }),
  routers: () => invoke<RouterProfile[]>("list_routers"),
  saveRouter: (profile: RouterProfile) => invoke<RouterProfile>("save_router", { profile }),
  deleteRouter: (key: string) => invoke<void>("delete_router", { key }),
  wifiQr: (ssid: string, password: string, auth: string) => invoke<string>("wifi_qr", { ssid, password, auth }),
  scan: () => invoke<LanScan>("scan_lan"),
  identify: (key: string, devices: LanDevice[]) => invoke<LanDevice[]>("identify_lan", { key, devices }),
  setAlias: (key: string, mac: string, alias: string) => invoke<void>("set_device_alias", { key, mac, alias }),
  vendors: (key: string, macs: string[]) => invoke<Record<string, string>>("lookup_vendors", { key, macs }),
  routerPortal: (key: string, name: string, url: string) => invoke<Portal>("router_portal", { key, name, url }),
  openDevice: (ip: string) => invoke<void>("open_device_page", { ip }),
};
