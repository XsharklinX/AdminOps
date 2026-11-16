// El equipo como un mapa de piezas: cada pieza con el color de lo peor que
// tiene, a partir del último diagnóstico. Lógica pura (se prueba sin equipo).
import type { Finding } from "./api";

export type PartId = "cpu" | "ram" | "gpu" | "board" | "net" | "battery" | "windows" | "security" | `disk:${string}`;

export interface Part {
  id: PartId;
  label: string;
  /** Una línea: el modelo, o lo que más importa de la pieza. */
  sub: string;
  status: "ok" | "warn" | "bad" | "unknown";
  findings: Finding[];
}

/** A qué pieza va cada hallazgo, por su área (y por el disco, si lo nombra). */
function partOf(f: Finding, diskNames: string[]): PartId {
  const area = f.area.toLowerCase();
  if (area === "discos" || area === "almacenamiento") {
    const name = diskNames.find((n) => n && f.title.startsWith(n));
    return name ? `disk:${name}` : "disk:*";
  }
  if (area === "memoria") return "ram";
  if (area === "temperatura") return /gpu|gr[aá]fica|nvidia|radeon/i.test(f.title) ? "gpu" : "cpu";
  if (area === "batería" || area === "bateria") return "battery";
  if (area === "seguridad") return "security";
  if (area === "hardware") return /bios|placa/i.test(f.title) ? "board" : /driver de/i.test(f.title) ? "gpu" : "board";
  if (area === "red") return "net";
  return "windows";
}

const worst = (fs: Finding[]): Part["status"] => (fs.some((f) => f.severity === "bad") ? "bad" : fs.some((f) => f.severity === "warn") ? "warn" : "ok");

export interface PartsInput {
  findings: Finding[];
  disks: { name: string; kind: string; size: number; status: "ok" | "warn" | "bad"; detail: string }[];
  gpus: string[];
  cpu: string | null;
  ramBytes: number | null;
  windows: string | null;
  securityScore: number | null;
  online: boolean | null;
  hasBattery: boolean;
}

export function partsMap(x: PartsInput): Part[] {
  const diskNames = x.disks.map((d) => d.name);
  const by = new Map<PartId, Finding[]>();
  for (const f of x.findings.filter((f) => f.severity !== "info")) {
    const id = partOf(f, diskNames);
    by.set(id, [...(by.get(id) ?? []), f]);
  }
  const fs = (id: PartId) => by.get(id) ?? [];
  const gb = (b: number) => `${Math.round(b / 1024 ** 3)} GB`;

  const parts: Part[] = [
    { id: "cpu", label: "Procesador", sub: x.cpu ?? "—", status: worst(fs("cpu")), findings: fs("cpu") },
    { id: "ram", label: "Memoria", sub: x.ramBytes ? gb(x.ramBytes) : "—", status: worst(fs("ram")), findings: fs("ram") },
    { id: "gpu", label: "Gráfica", sub: x.gpus.join(" + ") || "—", status: worst(fs("gpu")), findings: fs("gpu") },
    { id: "board", label: "Placa y BIOS", sub: "", status: worst(fs("board")), findings: fs("board") },
  ];
  // Los hallazgos de disco que no nombran disco van al que peor está (o al primero).
  const loose = fs("disk:*");
  const ranked = [...x.disks].sort((a, b) => ({ bad: 0, warn: 1, ok: 2 })[a.status] - ({ bad: 0, warn: 1, ok: 2 })[b.status]);
  for (const d of x.disks) {
    const own = [...fs(`disk:${d.name}`), ...(d === ranked[0] ? loose : [])];
    const status = d.status === "bad" ? "bad" : worst(own) === "bad" ? "bad" : d.status === "warn" || worst(own) === "warn" ? "warn" : "ok";
    parts.push({ id: `disk:${d.name}`, label: d.kind ? `Disco ${d.kind}` : "Disco", sub: d.name, status, findings: own });
  }
  if (!x.disks.length && loose.length) parts.push({ id: "disk:*", label: "Discos", sub: "", status: worst(loose), findings: loose });
  parts.push({ id: "net", label: "Red", sub: x.online === null ? "" : x.online ? "Con Internet" : "Sin Internet", status: x.online === false ? "bad" : worst(fs("net")), findings: fs("net") });
  if (x.hasBattery || fs("battery").length) parts.push({ id: "battery", label: "Batería", sub: "", status: worst(fs("battery")), findings: fs("battery") });
  parts.push({ id: "windows", label: "Windows", sub: x.windows ?? "", status: worst(fs("windows")), findings: fs("windows") });
  parts.push({
    id: "security",
    label: "Seguridad",
    sub: x.securityScore !== null ? `${x.securityScore}/100` : "",
    status: fs("security").length ? worst(fs("security")) : x.securityScore === null ? "unknown" : x.securityScore < 60 ? "bad" : x.securityScore < 80 ? "warn" : "ok",
    findings: fs("security"),
  });
  return parts;
}
