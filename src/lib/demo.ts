// Modo demostración: la app se llena con un parque de equipos, clientes y
// contactos de ejemplo, sin tocar los reales. Para enseñar AdminOps a un
// cliente o a un compañero nuevo, o grabar un vídeo, sin enseñar datos de nadie.
// Lo que se guardaría (clientes, contactos, casos) no se guarda mientras dura.
import type { Client, Contact, DisplayList, FaultAutoSettings, FaultRecMonitor, FaultRecStatus, FaultRecWindow, FaultWatchStatus, FaultRecording, JournalEntry, KbdState, Machine, MachineInventory, ProgramUsage, RecentDoc, ScanDiagnosis, ScanSetup, ScanStatus, ScanWatch, SpeedResult, TargetUser, UnsavedFile, UsageSnapshot } from "./api";

const DAY = 86400;
const now = () => Math.floor(Date.now() / 1000);

function inv(p: Partial<MachineInventory>): MachineInventory {
  return {
    manufacturer: "HP",
    model: "ProDesk 400 G7",
    serial: "CZC0000000",
    cpu: "Intel Core i5-10500",
    cores: 6,
    ramGb: 16,
    disks: "SSD 512 GB",
    gpu: "Intel UHD 630",
    os: "Windows 11 Pro 23H2",
    biosYear: 2021,
    installed: "2022-03-14",
    tpm: true,
    secureBoot: true,
    security: 86,
    battery: null,
    ip: "192.168.1.20",
    mac: "00-11-22-33-44-55",
    verdict: "ok",
    reasons: [],
    updated: now() - 3 * DAY,
    ...p,
  };
}

function machine(host: string, p: Partial<MachineInventory>): Machine {
  const i = inv(p);
  return { host, os: i.os, firstSeen: now() - 400 * DAY, lastSeen: now() - 3 * DAY, hardware: `${i.manufacturer} ${i.model}`, inventory: i };
}

function client(id: string, name: string, contact: string, machines: Machine[]): Client {
  return { id, name, contact, phone: "600 000 000", email: `info@${id}.example`, address: "Calle de Ejemplo, 1", notes: "", created: now() - 500 * DAY, machines, sessions: [], network: null };
}

export const DEMO_CLIENTS: Client[] = [
  client("asesoria-norte", "Asesoría Norte (ejemplo)", "Laura Gómez", [
    machine("PC-CONTA-01", { ip: "192.168.1.21" }),
    machine("PC-CONTA-02", { os: "Windows 10 Pro 22H2", tpm: false, secureBoot: false, disks: "HDD 1 TB", verdict: "replace", reasons: ["Sin TPM 2.0: no puede pasar a Windows 11", "Disco mecánico lento"], biosYear: 2015, model: "Compaq Elite 8300" }),
    machine("PC-RECEP", { verdict: "upgrade", ramGb: 4, reasons: ["Solo 4 GB de memoria"], model: "ProDesk 400 G4" }),
    machine("LAP-GERENCIA", { manufacturer: "Lenovo", model: "ThinkPad T14 Gen 3", battery: 71, cpu: "Intel Core i7-1255U" }),
  ]),
  client("clinica-sol", "Clínica Sol (ejemplo)", "Dr. Martín Ruiz", [
    machine("CONSULTA-1", { manufacturer: "Dell", model: "OptiPlex 7090" }),
    machine("CONSULTA-2", { manufacturer: "Dell", model: "OptiPlex 3020", os: "Windows 10 Pro 22H2", tpm: false, verdict: "replace", reasons: ["Equipo de 2014", "Sin TPM 2.0"], biosYear: 2014 }),
    machine("RECEPCION", { manufacturer: "Dell", model: "OptiPlex 7090", disks: "HDD 500 GB", verdict: "upgrade", reasons: ["Cambiar el disco por un SSD"] }),
  ]),
  client("taller-rio", "Talleres Río (ejemplo)", "Pedro Sanz", [machine("TALLER-PC", { manufacturer: "Acer", model: "Veriton X2680G" })]),
];

function contact(id: string, name: string, role: string, company: string, extension: string, tags: string[]): Contact {
  return {
    id,
    name,
    role,
    company,
    extension,
    phone: "910 000 000",
    mobile: "600 000 000",
    email: `${id}@ejemplo.example`,
    channels: [],
    reason: "",
    availability: "",
    substituteId: "",
    clientId: "",
    tags,
    notes: "",
    favorite: false,
    uses: 0,
    lastUsed: 0,
    created: now() - 200 * DAY,
    updated: now() - 20 * DAY,
    deleted: null,
  };
}

export const DEMO_CONTACTS: Contact[] = [
  contact("laura", "Laura Gómez", "Gerente", "Asesoría Norte", "201", ["cliente"]),
  contact("martin", "Martín Ruiz", "Director médico", "Clínica Sol", "100", ["cliente"]),
  contact("soporte-isp", "Soporte del proveedor de fibra", "Averías", "Operador", "", ["proveedor", "internet"]),
  contact("impresoras", "Servicio técnico de impresoras", "Renting", "Copiadoras SA", "", ["proveedor", "impresoras"]),
  contact("pedro", "Pedro Sanz", "Encargado", "Talleres Río", "", ["cliente"]),
];

const DEMO_JOURNAL = (): JournalEntry[] => [
  { id: 9001, timestamp: now() - 3600, op: "apply", tweakId: "privacy.telemetry", title: "Desactivar la telemetría", ok: true, message: null, reverted: false, undoable: true },
  { id: 9002, timestamp: now() - 7200, op: "run", tweakId: "cleanup.user-temp", title: "Limpiar los temporales del usuario", ok: true, message: "1,2 GB liberados", reverted: false, undoable: false },
  { id: 9003, timestamp: now() - DAY, op: "restorePoint", tweakId: null, title: "Punto de restauración", ok: true, message: null, reverted: false, undoable: false },
];

const DEMO_USER: TargetUser = { name: "usuario.demo", sid: "S-1-5-21-0-0-0-1001", redirected: false };

const DEMO_DISPLAYS = (): DisplayList => ({
  monitors: [
    { id: "demo-1", name: "LG 24MK430", maker: "LG", model: "24MK430", number: 1, width: 1280, height: 1024, x: 0, y: 0, hz: 60, maxHz: 75, nativeWidth: 1920, nativeHeight: 1080, connection: "HDMI", scale: 100, primary: true, rotation: 0 },
    { id: "demo-2", name: "Dell P2419H", maker: "Dell", model: "P2419H", number: 2, width: 1920, height: 1080, x: 1280, y: 0, hz: 60, maxHz: 60, nativeWidth: 1920, nativeHeight: 1080, connection: "DisplayPort", scale: 100, primary: false, rotation: 0 },
  ],
  duplicated: false,
  pending: null,
});

// ---------- Uso de la red y escanear a carpeta (los datos de la maqueta) ----------

const MB = 1024 * 1024;
let demoPause: { untilMs: number; entryId: number } | null = null;

// «Grabar el fallo»: en la demo no hay vídeo. Empezar a grabar simula 5 s y deja una grabación de ejemplo
// de 1:12 con las tres marcas de siempre (disco con OneDrive, Excel que no responde y el error 1002).
const DEMO_FAULT_SECS = 5;
let demoFault: FaultRecording | null = null;
let demoFaultStart = 0;

const DEMO_FAULT_MONITORS = (): FaultRecMonitor[] => [{ id: "demo-monitor-1", name: "DELL U2419H", number: 1, primary: true, width: 1920, height: 1080 }];

const DEMO_FAULT_REC = (): FaultRecording => ({
  id: "fr-demo",
  started: now() - 20 * 60,
  secs: 72,
  monitor: "Monitor principal",
  width: 1920,
  height: 1080,
  caseId: null,
  bytes: 21_700_000,
  note: null,
  marks: [
    { t: 16, kind: "disk", title: "Disco al 100 %", detail: "OneDrive leyendo y escribiendo en el disco", probable: true },
    { t: 24, kind: "hang", title: "Excel no responde", detail: "Excel no respondió 14 s", app: true },
    { t: 43, kind: "error", title: "Error 1002", detail: "Un programa dejó de responder", app: true },
  ],
  target: "Pantalla entera",
  inputOn: true,
  // Lo que hizo el usuario justo antes de que Excel dejara de responder.
  input: [
    { t: 9_200, kind: "click", x: 0.31, y: 0.42, label: "Doble clic" },
    { t: 12_000, kind: "keys", x: 0, y: 0, label: "•••" },
    { t: 17_400, kind: "click", x: 0.03, y: 0.05, label: "Clic" },
    { t: 18_600, kind: "keys", x: 0, y: 0, label: "Ctrl + P" },
    { t: 21_100, kind: "keys", x: 0, y: 0, label: "Intro" },
    { t: 38_000, kind: "click", x: 0.97, y: 0.02, label: "Clic" },
  ],
  // Las pistas: el disco se pone al 100 % con OneDrive justo antes de que Excel deje de responder.
  samples: Array.from({ length: 72 }, (_, t) => ({
    t,
    cpu: t >= 22 && t <= 28 ? 85 + (t % 3) * 4 : 12 + ((t * 7) % 15),
    disk: t >= 14 && t <= 26 ? 95 + (t % 2) * 5 : 4 + ((t * 5) % 9),
    mem: 58 + Math.min(10, t / 8),
    net: t >= 14 && t <= 26 ? 2_400_000 : 40_000 + ((t * 13) % 7) * 9_000,
  })),
  mic: true,
  // La voz: habla al principio («ahora le doy a imprimir…») y al final.
  voice: Array.from({ length: 720 }, (_, i) => (i > 150 && i < 260) || (i > 400 && i < 470) ? 30 + ((i * 37) % 50) : (i * 13) % 7),
});

// Vigilar en la demo: el contador del último minuto se llena con el reloj de verdad.
let demoWatchSince = 0;
let demoAuto: FaultAutoSettings = { enabled: false, hang: true, appError: true, busy: true, ignored: [] };
const demoFaultWatch = (): FaultWatchStatus => ({
  on: demoWatchSince !== 0,
  since: demoWatchSince || null,
  bufferedSecs: demoWatchSince ? Math.min(75, now() - demoWatchSince) : 0,
  monitor: "Monitor principal",
  auto: structuredClone(demoAuto),
  pausedForRecording: false,
  hotkey: true,
  skippedToday: 0,
});

const DEMO_FAULT_WINDOWS = (): FaultRecWindow[] => [
  { id: 101, title: "Facturación nov.xlsx - Excel", app: "Microsoft Excel", minimized: false },
  { id: 102, title: "Contabilidad · Asientos", app: "Contabilidad", minimized: false },
  { id: 103, title: "Portal del banco - Microsoft Edge", app: "Microsoft Edge", minimized: true },
];

/** Si la grabación simulada ya acabó, queda guardada. */
function demoFaultTick(): FaultRecStatus {
  if (demoFaultStart && Date.now() - demoFaultStart >= DEMO_FAULT_SECS * 1000) {
    demoFault = DEMO_FAULT_REC();
    demoFaultStart = 0;
  }
  const secs = demoFaultStart ? Math.min(DEMO_FAULT_SECS, Math.floor((Date.now() - demoFaultStart) / 1000)) : 0;
  return { recording: demoFaultStart !== 0, finishing: false, secs, maxSecs: 120, id: demoFaultStart ? "fr-demo" : null, error: null };
}

const demoProgram = (key: string, name: string, p: Partial<ProgramUsage>): ProgramUsage => ({
  key,
  name,
  path: `C:\\Demo\\${key}`,
  upMbps: 0,
  downMbps: 0,
  totalUp: 0,
  totalDown: 0,
  todayUp: 0,
  todayDown: 0,
  watchedSecs: 240,
  destinations: [],
  active: true,
  ...p,
});

const DEMO_NETUSAGE = (): UsageSnapshot => {
  const now = Date.now();
  const paused = demoPause && demoPause.untilMs > now ? demoPause : null;
  const programs: ProgramUsage[] = [
    demoProgram("onedrive.exe", "OneDrive", {
      upMbps: paused ? 0 : 18.4,
      totalUp: 552 * MB,
      todayUp: 1.4 * 1024 * MB,
      todayDown: 90 * MB,
      active: !paused,
      destinations: [
        { host: "onedrive.live.com", ip: "13.107.42.12", bytes: 400 * MB },
        { host: "storage.live.com", ip: "52.108.8.254", bytes: 120 * MB },
        { host: "13.107.6.171", ip: "13.107.6.171", bytes: 32 * MB },
      ],
    }),
    demoProgram("chrome.exe", "Chrome", {
      downMbps: 2.1,
      upMbps: 0.05,
      totalDown: 80 * MB,
      todayUp: 120 * MB,
      todayDown: 2.6 * 1024 * MB,
      destinations: [
        { host: "www.youtube.com", ip: "142.250.78.14", bytes: 60 * MB },
        { host: "fonts.gstatic.com", ip: "142.250.200.67", bytes: 4 * MB },
      ],
    }),
    demoProgram("ms-teams.exe", "Teams", {
      upMbps: 0.6,
      downMbps: 0.6,
      totalUp: 18 * MB,
      totalDown: 18 * MB,
      todayUp: 210 * MB,
      todayDown: 340 * MB,
      destinations: [{ host: "worldaz.tr.teams.microsoft.com", ip: "52.112.100.10", bytes: 30 * MB }],
    }),
    demoProgram("svchost:wu", "Windows Update", { downMbps: 0.3, totalDown: 9 * MB, todayDown: 410 * MB, destinations: [{ host: "tlu.dl.delivery.mp.microsoft.com", ip: "152.199.21.175", bytes: 9 * MB }] }),
    demoProgram("slack.exe", "Slack", { active: false, todayUp: 12 * MB, todayDown: 40 * MB }),
  ];
  return {
    running: true,
    method: "etw",
    tcpOnly: false,
    error: null,
    programs,
    totalUpMbps: programs.reduce((a, p) => a + p.upMbps, 0),
    totalDownMbps: programs.reduce((a, p) => a + p.downMbps, 0),
    startedMs: now - 240_000,
    nowMs: now,
    onedrivePause: paused,
    lostEvents: 0,
  };
};

/** Un icono de ejemplo: la inicial del programa en un cuadro de color. */
const demoIcon = (path: string): string => {
  const name = path.split("\\").pop() ?? "?";
  const hue = [...name].reduce((a, c) => (a * 31 + c.charCodeAt(0)) % 360, 7);
  const svg = `<svg xmlns="http://www.w3.org/2000/svg" width="32" height="32"><rect width="32" height="32" rx="6" fill="hsl(${hue} 55% 45%)"/><text x="16" y="22" font-family="sans-serif" font-size="18" font-weight="700" text-anchor="middle" fill="white">${name.charAt(0).toUpperCase()}</text></svg>`;
  return `data:image/svg+xml;base64,${btoa(svg)}`;
};

const DEMO_SPEED = (): SpeedResult[] => [
  { timestamp: now() - 3600, server: "Ejemplo", isp: "Proveedor de ejemplo", ip: null, latencyMs: 14, jitterMs: 2, downloadMbps: 200, uploadMbps: 20, downloadLatencyMs: 20, uploadLatencyMs: 25, transferredMb: 180 },
];

const DEMO_SCAN_STATUS: ScanStatus = {
  isAdmin: true,
  computer: "NOMBRE-DEL-EQUIPO",
  folder: "C:\\Escaneos",
  folderExists: false,
  userExists: false,
  shareExists: false,
  shareOk: false,
  networkCategory: "Private",
  networkName: "Red de ejemplo",
  localIp: "10.0.0.23",
  sameSubnet: true,
  requireSigning: false,
  smb1Enabled: false,
  passwordMin: 8,
  passwordMax: 18,
  passwordComplexity: true,
  firewallOff: 1,
};

const DEMO_SCAN_SETUP: ScanSetup = {
  path: "\\\\NOMBRE-DEL-EQUIPO\\Escaneos",
  ipPath: "\\\\10.0.0.23\\Escaneos",
  user: "escaner",
  password: "Mesa-Verde-42",
  speech: "Mesa, guion, Verde, guion, cuatro, dos",
  entryId: 9100,
  publicNetwork: false,
  folderHadFiles: false,
  steps: [
    { id: "folder", label: "Carpeta Escaneos creada", state: "ok", detail: null },
    { id: "share", label: "Compartida solo con el usuario «escaner»", state: "ok", detail: null },
    { id: "firewall", label: "Cortafuegos de este equipo abierto para compartir", state: "ok", detail: null },
    { id: "reach", label: "La impresora llega a este equipo", state: "ok", detail: "Responde al ping · misma red · el servidor de archivos escucha en el 445" },
  ],
};

let demoWatchAt = 0;
const demoWatch = (): ScanWatch => {
  const waited = Date.now() - demoWatchAt;
  // A los 6 s del «escaneo de prueba» llega el archivo de la maqueta.
  if (waited < 6000) return { running: true, state: "waiting", startedMs: demoWatchAt, file: null, pages: null, arrivedMs: null, diagnosis: null };
  return { running: false, state: "arrived", startedMs: demoWatchAt, file: "escaneo_0012.pdf", pages: 2, arrivedMs: demoWatchAt + 6000, diagnosis: null };
};

const DEMO_DIAGNOSIS: ScanDiagnosis = { kind: "nothing", sentence: "La impresora no ha intentado conectar: revisa la ruta y que esté en la misma red.", details: [], signingRequired: false };

const DEMO_KBD: KbdState = {
  rows: [
    { tag: "es-DO", language: "Español (República Dominicana)", layout: "Latinoamericano", klid: "0000080A", langId: 0x1c0a, active: true },
    { tag: "en-US", language: "Inglés (Estados Unidos)", layout: "Inglés (EE. UU.)", klid: "00000409", langId: 0x0409, active: false },
  ],
  languageCount: 2,
  region: "DO",
  shortcut: { code: "1", label: "Alt+Mayús" },
  hasSpanish: true,
  otherUser: false,
  kept: null,
  warning: null,
  note: null,
};

const DEMO_UNSAVED = (): { files: UnsavedFile[]; recent: RecentDoc[] } => ({
  files: [
    { path: "C:\\Users\\usuario.demo\\AppData\\Roaming\\Microsoft\\Excel\\Presupuesto 2027.xlsb", name: "Presupuesto 2027.xlsx", app: "Excel", origin: "Autorrecuperación", size: 48_210, modified: now() - 18 * 60, ext: "xlsb", copyExt: "xlsb" },
    { path: "C:\\Users\\usuario.demo\\AppData\\Local\\Microsoft\\Office\\UnsavedFiles\\Acta de la reunión.asd", name: "Acta de la reunión.asd", app: "Word", origin: "Sin guardar", size: 22_540, modified: now() - 2 * 3600, ext: "asd", copyExt: "asd" },
    { path: "C:\\Users\\usuario.demo\\AppData\\Roaming\\Microsoft\\Word\\Informe trimestral.asd", name: "Informe trimestral.asd", app: "Word", origin: "Autorrecuperación", size: 31_880, modified: now() - DAY - 3600, ext: "asd", copyExt: "asd" },
  ],
  recent: [
    { name: "Cuentas 2026.xlsx", path: "C:\\Users\\usuario.demo\\Documents\\Cuentas 2026.xlsx", app: "Excel", opened: now() - 3 * 3600 },
    { name: "Contrato de servicio.docx", path: "C:\\Users\\usuario.demo\\Documents\\Contrato de servicio.docx", app: "Word", opened: now() - DAY },
  ],
});

/** Respuesta de ejemplo para una orden, o undefined si se deja pasar la real. */
export function demoAnswer(command: string, args?: Record<string, unknown>): unknown {
  switch (command) {
    // El cuaderno de Notas: unas de ejemplo, nunca las reales del técnico.
    case "notes_list":
      return [
        { id: "n-demo-1", title: "Impresora de recepción", text: "Si sale «sin conexión»: apagarla 10 s y volver a encenderla.\nIP fija 192.168.1.40.", created: (now() - 3 * DAY) * 1000, updated: (now() - DAY) * 1000, pinned: true, images: [], remindAt: null },
        { id: "n-demo-2", title: "Llamar a contabilidad", text: "Pedir el número de serie del portátil nuevo.", created: (now() - 2 * 3600) * 1000, updated: (now() - 2 * 3600) * 1000, pinned: false, images: [], remindAt: null },
      ];
    case "list_clients":
      return structuredClone(DEMO_CLIENTS);
    case "list_contacts":
      return structuredClone(DEMO_CONTACTS);
    case "get_journal":
      return DEMO_JOURNAL();
    case "get_target_user":
      return DEMO_USER;
    case "case_current":
    case "get_session":
      return null;
    case "cases_for_person":
      return [];
    case "displays_list":
      return DEMO_DISPLAYS();
    case "kbd_state":
      return structuredClone(DEMO_KBD);
    case "netusage_start":
    case "netusage_snapshot":
      return DEMO_NETUSAGE();
    case "netusage_stop":
      return null;
    case "netusage_icon":
      return typeof args?.path === "string" ? demoIcon(args.path) : null;
    case "faultrec_monitors":
      return DEMO_FAULT_MONITORS();
    case "faultrec_windows":
      return DEMO_FAULT_WINDOWS();
    case "faultrec_thumb":
      return null;
    case "faultrec_to_report":
    case "faultrec_report_remove":
      return {};
    case "faultrec_export":
      return null;
    case "faultrec_export_small":
    case "faultrec_export_gif":
      return { path: "demo", ms: 0 };
    case "faultrec_watch_start":
      demoWatchSince = now();
      return demoFaultWatch();
    case "faultrec_watch_stop":
      demoWatchSince = 0;
      return demoFaultWatch();
    case "faultrec_watch_status":
      return demoFaultWatch();
    case "faultrec_just_happened":
      demoFault = { ...DEMO_FAULT_REC(), id: "fr-demo", secs: 60, origin: "replay" };
      return structuredClone(demoFault);
    case "faultrec_auto_get":
      return structuredClone(demoAuto);
    case "faultrec_auto_set":
      demoAuto = structuredClone(args?.settings as FaultAutoSettings);
      return structuredClone(demoAuto);
    case "faultrec_auto_ignore":
      if (typeof args?.key === "string" && !demoAuto.ignored.some((i) => i.key === args.key)) demoAuto.ignored.push({ key: args.key, label: String(args.label ?? args.key) });
      return null;
    case "faultrec_auto_unignore":
      demoAuto.ignored = demoAuto.ignored.filter((i) => i.key !== args?.key);
      return null;
    case "faultrec_pick_region":
      throw new Error("En la demo no se puede dibujar una zona: prueba con «Pantalla entera» o «Una ventana».");
    case "faultrec_region_done":
      return null;
    case "faultrec_start":
      demoFault = null;
      demoFaultStart = Date.now();
      return demoFaultTick();
    case "faultrec_status":
    case "faultrec_pause":
    case "faultrec_resume":
      return demoFaultTick();
    case "faultrec_mark":
      return demoFaultTick().secs;
    case "faultrec_draw":
      return false;
    case "faultrec_stop":
      // Parar antes de tiempo en la demo deja igualmente la grabación de ejemplo.
      demoFaultStart = 0;
      demoFault = DEMO_FAULT_REC();
      return structuredClone(demoFault);
    case "faultrec_list":
      demoFaultTick();
      return demoFault ? [structuredClone(demoFault)] : [];
    case "faultrec_get":
      demoFaultTick();
      return demoFault ? { meta: structuredClone(demoFault), path: "" } : null;
    case "faultrec_delete":
      demoFault = null;
      return null;
    case "list_speedtests":
      return DEMO_SPEED();
    case "onedrive_pause": {
      demoPause = { untilMs: Date.now() + 2 * 3_600_000, entryId: 9200 };
      return { ...demoPause, message: "OneDrive en pausa: se vuelve a abrir solo dentro de 2 horas." };
    }
    case "scanfolder_status":
      return structuredClone(DEMO_SCAN_STATUS);
    case "scanfolder_setup":
      return structuredClone(DEMO_SCAN_SETUP);
    case "scanfolder_watch_start":
      demoWatchAt = Date.now();
      return demoWatch();
    case "scanfolder_watch_status":
      return demoWatch();
    case "scanfolder_watch_stop":
    case "scanfolder_open_folder":
      return null;
    case "scanfolder_diagnose":
      return structuredClone(DEMO_DIAGNOSIS);
    case "scanfolder_desktop_shortcut":
      return "Acceso «Escaneos» creado en el Escritorio.";
    case "unsaved_list":
      return DEMO_UNSAVED();
    case "unsaved_preview":
      return { lines: ["Presupuesto 2027", "Concepto · Importe", "Alquiler · 1.200", "Suministros · 340"] };
    case "unsaved_versions":
      return { versions: [{ id: "demo-v1", when: now() - 2 * DAY, size: 51_200, modified: now() - 2 * DAY }], snapshots: 3 };
    case "unsaved_pick":
      return null;
    default:
      return undefined;
  }
}

/** ¿Guardaría algo en los datos del técnico? En demostración no se guarda. */
export function demoBlocks(command: string): boolean {
  return /^(save_|delete_|purge_|import_|update_client|contacts_bulk|revert_entry|display_(apply|confirm|undo|restart_driver)|kbd_only_spanish|unsaved_copy|scanfolder_(new_password|private_network|disable_signing)|plan_(state_save|reboot)|measure_record|case_(open|update|close|discard)|inventory_(add|remove)|finish_session|start_session|notes_(save|delete))/.test(command);
}

export const DEMO_BLOCKED = "Modo demostración: no se guarda nada. Desactívalo en Ajustes → General para trabajar con tus datos.";
