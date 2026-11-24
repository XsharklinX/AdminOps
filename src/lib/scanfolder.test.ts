import { describe, expect, it } from "vitest";
import type { ScanDiagnosis, ScanStep, ScanWatch, VendorPrinter } from "./api";
import { arrivedText, collapseSteps, dictationText, headerText, ipPathText, isIpv4, modelName, offersSigning, panelFields, pickPrinters, setupWorked, waitedText, watchLine } from "./scanfolder";

const printer = (vendor: VendorPrinter["vendor"], brand: string, model: string, ip: string | null): VendorPrinter =>
  ({ name: `${brand} ${model}`, vendor, brand, model, connection: { kind: ip ? "red" : "usb", ip, port: "IP_1" } }) as unknown as VendorPrinter;

const step = (id: string, state: ScanStep["state"], label = id): ScanStep => ({ id, label, state, detail: null });

describe("la impresora que se elige", () => {
  it("solo RICOH y EPSON con IP, sin repetir la marca ni la IP", () => {
    const list = pickPrinters([
      printer("ricoh", "ricoh", "RICOH IM C3000", "10.0.0.41"),
      printer("epson", "epson", "WF-C5790", "10.0.0.50"),
      printer("ricoh", "ricoh", "RICOH IM C3000", "10.0.0.41"),
      printer("otra", "HP", "LaserJet", "10.0.0.60"),
      printer("epson", "EPSON", "L3250", null),
    ]);
    expect(list.map((p) => p.label)).toEqual(["RICOH IM C3000 · 10.0.0.41", "EPSON WF-C5790 · 10.0.0.50"]);
    expect(list[1].vendor).toBe("epson");
    expect(pickPrinters([])).toEqual([]);
  });

  it("el nombre del modelo no repite la marca", () => {
    expect(modelName("RICOH", "RICOH IM C3000")).toBe("RICOH IM C3000");
    expect(modelName("RICOH", "IM C3000")).toBe("RICOH IM C3000");
    expect(modelName("", "IM C3000")).toBe("IM C3000");
    expect(modelName("EPSON", "")).toBe("EPSON");
  });

  it("la IP escrita a mano se valida", () => {
    expect(isIpv4("10.0.0.41")).toBe(true);
    expect(isIpv4(" 10.0.0.41 ")).toBe(true);
    expect(isIpv4("10.0.0.256")).toBe(false);
    expect(isIpv4("impresora")).toBe(false);
    expect(isIpv4("10.0.0")).toBe(false);
    expect(isIpv4("")).toBe(false);
  });

  it("la cabecera de la maqueta", () => {
    expect(headerText("RICOH IM C3000", "10.0.0.41")).toBe("Escanear a carpeta · RICOH IM C3000 · 10.0.0.41");
    expect(headerText(null, "10.0.0.41")).toBe("Escanear a carpeta · 10.0.0.41");
    expect(headerText(null, "")).toBe("Escanear a carpeta");
  });
});

describe("el panel de la impresora", () => {
  it("los tres campos y la frase para dictarla", () => {
    const f = panelFields({ path: "\\\\NOMBRE-DEL-EQUIPO\\Escaneos", user: "escaner", password: "Mesa-Verde-42" });
    expect(f.map((x) => [x.id, x.label, x.value])).toEqual([
      ["path", "Ruta", "\\\\NOMBRE-DEL-EQUIPO\\Escaneos"],
      ["user", "Usuario", "escaner"],
      ["password", "Contraseña", "Mesa-Verde-42"],
    ]);
    expect(dictationText("Mesa, guion, Verde, guion, cuatro, dos")).toBe("Para dictarla: «Mesa, guion, Verde, guion, cuatro, dos».");
    expect(ipPathText("\\\\10.0.0.23\\Escaneos")).toBe("o con la IP: \\\\10.0.0.23\\Escaneos");
    expect(ipPathText(null)).toBeNull();
  });
});

describe("los pasos", () => {
  it("si todo salió bien, una sola línea; la impresora llega aparte", () => {
    const out = collapseSteps([step("folder", "ok"), step("share", "ok"), step("firewall", "ok"), step("reach", "ok", "La impresora llega a este equipo")]);
    expect(out.map((s) => s.label)).toEqual(["Carpeta, usuario y cortafuegos", "La impresora llega a este equipo"]);
  });

  it("si alguno avisó, cada paso con su frase", () => {
    const out = collapseSteps([step("folder", "ok"), step("share", "ok"), step("firewall", "warn", "Cortafuegos sin tocar: la red es pública"), step("reach", "warn")]);
    expect(out.map((s) => s.id)).toEqual(["folder", "share", "firewall", "reach"]);
  });

  it("a medias (sin cortafuegos todavía) no se resume", () => {
    expect(collapseSteps([step("folder", "ok"), step("share", "running")]).map((s) => s.id)).toEqual(["folder", "share"]);
    expect(collapseSteps([])).toEqual([]);
  });

  it("funcionó si ningún paso falló ni sigue en marcha", () => {
    expect(setupWorked([step("folder", "ok"), step("reach", "warn")])).toBe(true);
    expect(setupWorked([step("folder", "ok"), step("reach", "error")])).toBe(false);
    expect(setupWorked([step("folder", "running")])).toBe(false);
    expect(setupWorked([])).toBe(false);
  });
});

describe("esperar el escaneo", () => {
  const watch = (p: Partial<ScanWatch>): ScanWatch => ({ running: false, state: "idle", startedMs: 0, file: null, pages: null, arrivedMs: null, diagnosis: null, ...p });

  it("«Llegó escaneo_0012.pdf · 2 páginas · hace 3 s»", () => {
    expect(arrivedText("escaneo_0012.pdf", 2, 10_000, 13_000)).toBe("Llegó escaneo_0012.pdf · 2 páginas · hace 3 s");
    expect(arrivedText("escaneo_0013.pdf", 1, 10_000, 13_000)).toBe("Llegó escaneo_0013.pdf · 1 página · hace 3 s");
    // Un JPG o un TIFF no tienen páginas.
    expect(arrivedText("escaneo_0014.jpg", null, 10_000, 70_000)).toBe("Llegó escaneo_0014.jpg · hace 1 min");
  });

  it("la línea de cada estado", () => {
    expect(watchLine(null, 0)).toBe("Sin esperar ningún escaneo.");
    expect(watchLine(watch({ state: "waiting", running: true }), 0)).toBe("Esperando un escaneo de prueba…");
    expect(watchLine(watch({ state: "arrived", file: "a.pdf", pages: 3, arrivedMs: 1000 }), 4000)).toBe("Llegó a.pdf · 3 páginas · hace 3 s");
    expect(watchLine(watch({ state: "timeout" }), 0)).toContain("10 minutos");
  });

  it("el tiempo que lleva esperando", () => {
    expect(waitedText(0, 85_000)).toBe("1:25");
    expect(waitedText(5000, 1000)).toBe("0:00");
  });

  it("quitar la firma solo se ofrece si es la causa probable", () => {
    const d = (kind: ScanDiagnosis["kind"], signingRequired: boolean): ScanDiagnosis => ({ kind, sentence: "", details: [], signingRequired });
    expect(offersSigning(d("signing", true))).toBe(true);
    expect(offersSigning(d("signing", false))).toBe(false);
    expect(offersSigning(d("nothing", true))).toBe(false);
    expect(offersSigning(d("smb1", true))).toBe(false);
    expect(offersSigning(null)).toBe(false);
  });
});
