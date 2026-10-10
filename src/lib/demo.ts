// Modo demostración: la app se llena con un parque de equipos, clientes y
// contactos de ejemplo, sin tocar los reales. Para enseñar AdminOps a un
// cliente o a un compañero nuevo, o grabar un vídeo, sin enseñar datos de nadie.
// Lo que se guardaría (clientes, contactos, casos) no se guarda mientras dura.
import type { Client, Contact, JournalEntry, Machine, MachineInventory, TargetUser } from "./api";

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

/** Respuesta de ejemplo para una orden, o undefined si se deja pasar la real. */
export function demoAnswer(command: string): unknown {
  switch (command) {
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
    default:
      return undefined;
  }
}

/** ¿Guardaría algo en los datos del técnico? En demostración no se guarda. */
export function demoBlocks(command: string): boolean {
  return /^(save_|delete_|purge_|import_|update_client|contacts_bulk|revert_entry|case_(open|update|close|discard)|inventory_(add|remove)|finish_session|start_session)/.test(command);
}

export const DEMO_BLOCKED = "Modo demostración: no se guarda nada. Desactívalo en Ajustes → General para trabajar con tus datos.";
