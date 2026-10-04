// Acceso remoto, solucionar problemas y línea de tiempo.
import { invoke } from "./core";

// ---------- Acceso remoto (agenda, herramientas) ----------

export interface RdpOptions {
  fullscreen: boolean;
  multimon: boolean;
  clipboard: boolean;
  drives: boolean;
  printers: boolean;
  audio: boolean;
}

export interface Connection {
  id: string;
  name: string;
  kind: string;
  target: string;
  username: string;
  client: string;
  notes: string;
  options: RdpOptions;
  savedPassword: boolean;
  lastUsed: number;
}

export interface Reach {
  resolved: string | null;
  pingMs: number | null;
  rdpOpen: boolean;
  hint: string;
}

export interface RemoteTool {
  id: string;
  name: string;
  installed: boolean;
  thisId: string | null;
}

export interface RdpServer {
  port: number;
  nla: boolean;
  users: string[];
}

export const remoteApi = {
  list: () => invoke<Connection[]>("list_connections"),
  save: (connection: Connection) => invoke<Connection>("save_connection", { connection }),
  remove: (id: string) => invoke<void>("delete_connection", { id }),
  setPassword: (id: string, password: string) => invoke<void>("set_connection_password", { id, password }),
  connect: (id: string) => invoke<void>("connect_saved", { id }),
  connectRdp: (target: string, username: string, options: RdpOptions) => invoke<void>("connect_rdp", { target, username, options }),
  test: (target: string) => invoke<Reach>("test_connection", { target }),
  tools: () => invoke<RemoteTool[]>("remote_tools"),
  connectTool: (tool: string, target: string) => invoke<void>("connect_tool_id", { tool, target }),
  openTool: (tool: string) => invoke<void>("open_remote_tool", { tool }),
  install: (tool: string) => invoke<void>("install_remote_tool", { tool }),
  server: () => invoke<RdpServer>("rdp_server"),
  setUser: (user: string, allow: boolean) => invoke<void>("set_rdp_user", { user, allow }),
};

// ---------- Solucionar problemas, reparación de red y línea de tiempo (1.1.1) ----------

export type Symptom = "internet" | "wifi" | "audio" | "bluetooth" | "display" | "printer" | "slow" | "winupdate";

export interface TroubleFix {
  id: string;
  label: string;
  admin: boolean;
  confirm: string | null;
}

export interface TroubleFinding {
  level: "ok" | "info" | "warn" | "bad";
  title: string;
  detail: string;
  fixes: TroubleFix[];
  page: string | null;
}

export interface NetCheck {
  connected: boolean;
  noDhcpAddress: boolean;
  adapter: string;
  gateway: boolean | null;
  internet: boolean;
  dns: boolean;
  /** IPv4 del adaptador activo. */
  ip: string;
  /** Puerta de enlace (el router). */
  gatewayIp: string;
  dnsServers: string[];
  /** La IP la da el router (DHCP) o está puesta a mano. */
  dhcp: boolean;
  wifi: boolean;
  /** Proxy configurado en Windows (vacío si no hay). */
  proxy: string;
  vpn: string[];
}

/** Qué pasa con la red y qué hacer ahora. */
export interface NetVerdict {
  level: "ok" | "warn" | "bad";
  title: string;
  text: string;
  /** Botones a ofrecer: router · dns · deep · wifi · proxy · speed. */
  next: string[];
}

export interface NetRepair {
  before: NetCheck;
  after: NetCheck;
  steps: { title: string; ok: boolean; detail: string }[];
  reboot: boolean;
  verdict: NetVerdict;
}

export const troubleshootApi = {
  check: (symptom: Symptom) => invoke<{ symptom: Symptom; findings: TroubleFinding[] }>("troubleshoot_check", { symptom }),
  fix: (id: string) => invoke<string>("troubleshoot_fix", { id }),
  repairNetwork: (deep: boolean) => invoke<NetRepair>("repair_network", { deep }),
  netCheck: () => invoke<NetCheck>("quick_net_check"),
};

export interface TimelineEvent {
  time: number;
  kind: "change" | "alert" | "scan" | "windows";
  level: "ok" | "info" | "warn" | "bad";
  title: string;
  detail: string;
  page: string | null;
}

export const timelineApi = {
  list: (days: number) => invoke<TimelineEvent[]>("machine_timeline", { days }),
};

export interface ContactChannel {
  kind: "phone" | "email";
  label: string;
  value: string;
}

export interface Contact {
  id: string;
  name: string;
  role: string;
  company: string;
  extension: string;
  phone: string;
  mobile: string;
  email: string;
  channels: ContactChannel[];
  reason: string;
  availability: string;
  substituteId: string;
  clientId: string;
  tags: string[];
  notes: string;
  favorite: boolean;
  uses: number;
  lastUsed: number;
  created: number;
  updated: number;
  deleted: number | null;
}

export type ContactBulk =
  | { op: "addTag"; tag: string }
  | { op: "removeTag"; tag: string }
  | { op: "favorite"; value: boolean }
  | { op: "delete" }
  | { op: "restore" }
  | { op: "purge" };

export const contactsApi = {
  list: () => invoke<Contact[]>("list_contacts"),
  // Las fechas y contadores van como enteros (u64/u32 en el backend).
  save: (contact: Contact) =>
    invoke<Contact>("save_contact", {
      contact: {
        ...contact,
        uses: Math.floor(contact.uses || 0),
        lastUsed: Math.floor(contact.lastUsed || 0),
        created: Math.floor(contact.created || 0),
        updated: Math.floor(contact.updated || 0),
        deleted: contact.deleted == null ? null : Math.floor(contact.deleted),
      },
    }),
  touch: (id: string) => invoke<void>("touch_contact", { id }),
  bulk: (ids: string[], action: ContactBulk) => invoke<number>("bulk_contacts", { ids, action }),
  merge: (keep: string, others: string[]) => invoke<void>("merge_contacts", { keep, others }),
  tagColors: () => invoke<{ name: string; color: string }[]>("contact_tag_colors"),
  setTagColor: (name: string, color: string) => invoke<void>("set_contact_tag_color", { name, color }),
  renameTag: (from: string, to: string) => invoke<number>("rename_contact_tag", { from, to }),
  deleteTag: (name: string) => invoke<number>("delete_contact_tag", { name }),
  backups: () => invoke<{ id: number; contacts: number }[]>("list_contact_backups"),
  backupNow: () => invoke<void>("backup_contacts_now"),
  restoreBackup: (id: number) => invoke<number>("restore_contact_backup", { id }),
  importFile: () => invoke<{ added: number; updated: number } | null>("import_contacts"),
  email: (email: string) => invoke<void>("write_email", { email }),
  teams: (email: string, call: boolean) => invoke<void>("open_teams", { email, call }),
  call: (number: string) => invoke<void>("call_number", { number }),
  saveVcard: (name: string, content: string) => invoke<string | null>("save_vcard", { name, content }),
};
