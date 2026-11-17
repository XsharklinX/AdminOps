import { describe, expect, it } from "vitest";
import type { DriverCandidate, ScanSoftware, VendorDeviceRef, VendorDownloadEvent, VendorPrinter, VendorStepEvent } from "../../lib/api";
import { applyStep, batchTargets, bestCandidate, bytesText, chipText, connectionText, coveredBy, installerFor, isAdminError, isAuditError, offersBatch, shortVersion, ticketSummary } from "./logic";

const dev: VendorDeviceRef = { kind: "printer", name: "x", vendor: "ricoh", brand: "ricoh", model: "MP 2555", hardwareIds: [], driverName: "", driverVersion: "", driverManufacturer: "" };

function printer(name: string, vendor: "ricoh" | "epson" | "otra", status: VendorPrinter["state"]["status"], kind = "red"): VendorPrinter {
  return {
    name,
    vendor,
    brand: vendor,
    model: name,
    connection: { kind, ip: kind === "red" ? "10.0.0.5" : null, port: "P" },
    driver: { name: "d", version: "1.0", date: "", manufacturer: "", inf: "", kind: "fabricante" },
    default: false,
    shared: false,
    hardwareIds: [],
    snmp: null,
    state: { status, label: status, reason: "", newVersion: null, source: null },
    device: { ...dev, name },
  };
}

const cand = (id: string, matchedBy: string, newer: boolean | null): DriverCandidate => ({ id, source: "biblioteca", title: id, version: "1", date: "", model: id, matchedBy, comparable: newer !== null, newer, package: "p" });

describe("conexión y chip", () => {
  it("dice cómo está conectada", () => {
    expect(connectionText({ kind: "usb", ip: null, port: "USB001" })).toBe("USB");
    expect(connectionText({ kind: "red", ip: "10.20.4.31", port: "IP_10.20.4.31" })).toBe("Red · 10.20.4.31");
    expect(connectionText({ kind: "otra", ip: null, port: "" })).toBe("Sin puerto");
  });
  it("«Viejo» lleva la versión nueva", () => {
    expect(chipText("viejo", "Viejo", "4.42.0.0")).toBe("Viejo · hay 4.42");
    expect(chipText("alDia", "Al día", null)).toBe("Al día");
    expect(shortVersion("3.07.1.0")).toBe("3.07.1");
  });
});

describe("candidato", () => {
  it("elige el que encaja por ID y más nuevo; nunca uno más viejo", () => {
    expect(bestCandidate([cand("a", "universal", true), cand("b", "hardwareId", true), cand("c", "hardwareId", null)])?.id).toBe("b");
    expect(bestCandidate([cand("a", "modelo", false)])).toBeNull();
    expect(bestCandidate([])).toBeNull();
  });
});

describe("lote", () => {
  const all = [printer("A", "ricoh", "viejo"), printer("B", "ricoh", "generico"), printer("C", "ricoh", "alDia"), printer("D", "epson", "viejo"), printer("E", "ricoh", "viejo", "servidor")];
  it("solo la marca, con driver viejo o genérico y no de servidor", () => {
    expect(batchTargets(all, "ricoh").map((p) => p.name)).toEqual(["A", "B"]);
  });
  it("se ofrece solo si hay más de una", () => {
    expect(offersBatch(all, "ricoh")).toBe(true);
    expect(offersBatch(all, "epson")).toBe(false);
  });
  it("el resumen cuenta hechas y fallidas", () => {
    const t = ticketSummary("ricoh", [
      { printer: "A", ok: true, from: "x 1", to: "x 2", message: "" },
      { printer: "B", ok: false, from: "x 1", to: "x 1", message: "no responde" },
    ]);
    expect(t).toContain("1 de 2");
    expect(t).toContain("- A: x 1 → x 2");
    expect(t).toContain("- B: sin cambios. no responde");
  });
});

describe("pasos", () => {
  const ev = (printerName: string, step: string, index: number, state: VendorStepEvent["state"]): VendorStepEvent => ({ printer: printerName, step, index, total: 5, state, message: "m" });
  it("sustituye el paso y ordena; ignora otra impresora", () => {
    let s = applyStep([], ev("P", "install", 2, "running"), "P");
    s = applyStep(s, ev("P", "backup", 1, "ok"), "P");
    s = applyStep(s, ev("P", "install", 2, "ok"), "P");
    s = applyStep(s, ev("Q", "driver", 3, "ok"), "P");
    expect(s.map((x) => [x.step, x.state])).toEqual([
      ["backup", "ok"],
      ["install", "ok"],
    ]);
  });
});

describe("mensajes y tamaños", () => {
  it("distingue administrador de auditoría", () => {
    expect(isAdminError("Requiere ejecutar AdminOps como administrador.")).toBe(true);
    expect(isAuditError("Modo auditoría activo: AdminOps no hace cambios en este equipo.")).toBe(true);
    expect(isAdminError("Modo auditoría activo: no hace cambios (administrador)")).toBe(false);
  });
  it("tamaños", () => {
    expect(bytesText(2048)).toBe("2 KB");
    expect(bytesText(5 * 1024 ** 2)).toBe("5 MB");
  });
});

describe("descargas y programas", () => {
  const ev: VendorDownloadEvent = { id: "dl:1", name: "x.exe", size: 1, kind: "exe", readable: true, version: "3.01", provider: "EPSON", vendor: "epson", models: ["EPSON L3250 Series"] };
  it("cubre la impresora del mismo modelo y marca", () => {
    const l3250 = printer("EPSON L3250", "epson", "falta");
    const other = printer("EPSON WF-C5790", "epson", "viejo");
    expect(coveredBy(ev, [l3250, other], []).printers.map((p) => p.name)).toEqual(["EPSON L3250"]);
    expect(coveredBy({ ...ev, readable: false }, [l3250], []).printers).toEqual([]);
  });
  it("encuentra el instalador del programa", () => {
    const soft: ScanSoftware = { id: "epsonscan2", name: "Epson Scan 2", vendor: "epson", installed: false, version: "" };
    expect(installerFor(soft, [{ id: "i1", name: "ES2_Setup.exe", relPath: "", size: 1 }])?.id).toBe("i1");
    expect(installerFor(soft, [{ id: "i2", name: "otro.exe", relPath: "", size: 1 }])).toBeNull();
  });
  it("no mezcla los instaladores de PaperStream Capture y PaperStream IP", () => {
    const capture: ScanSoftware = { id: "paperstream-capture", name: "PaperStream Capture", vendor: "ricoh", installed: false, version: "" };
    const ip: ScanSoftware = { id: "paperstream-ip", name: "PaperStream IP", vendor: "ricoh", installed: false, version: "" };
    const installers = [
      { id: "psc", name: "PaperStreamCapture_Setup.exe", relPath: "", size: 1 },
      { id: "psip", name: "PaperStreamIP_Setup.exe", relPath: "", size: 1 },
    ];
    expect(installerFor(capture, installers)?.id).toBe("psc");
    expect(installerFor(ip, installers)?.id).toBe("psip");
  });
});
