import { describe, expect, it } from "vitest";
import type { Finding } from "./api";
import { partsMap, type PartsInput } from "./partsMap";

const f = (area: string, title: string, severity: Finding["severity"] = "warn"): Finding => ({ area, title, severity, detail: null, actions: [], key: title });
const input = (over: Partial<PartsInput> = {}): PartsInput => ({
  findings: [],
  disks: [
    { name: "ST3500410SV", kind: "HDD", size: 500e9, status: "bad", detail: "" },
    { name: "P3-1TB", kind: "SSD", size: 1e12, status: "ok", detail: "" },
  ],
  gpus: ["GTX 1050"],
  cpu: "Ryzen 5 5500",
  ramBytes: 16 * 1024 ** 3,
  windows: "Windows 11 Pro",
  securityScore: 92,
  online: true,
  hasBattery: false,
  ...over,
});

describe("mapa de piezas", () => {
  it("cada hallazgo va a su pieza y la pieza toma el color de lo peor", () => {
    const parts = partsMap(
      input({
        findings: [
          f("Discos", "ST3500410SV: 517 sectores dañados", "bad"),
          f("Memoria", "La RAM funciona en un solo canal", "info"),
          f("Temperatura", "CPU a 92 °C en reposo", "bad"),
          f("Sistema", "Hay un reinicio pendiente"),
        ],
      }),
    );
    const get = (id: string) => parts.find((p) => p.id === id)!;
    expect(get("disk:ST3500410SV").status).toBe("bad");
    expect(get("disk:P3-1TB").status).toBe("ok");
    expect(get("cpu").status).toBe("bad");
    // Las sugerencias no pintan nada.
    expect(get("ram").status).toBe("ok");
    expect(get("windows").findings).toHaveLength(1);
  });

  it("un hallazgo de disco sin nombre va al disco que peor está", () => {
    const parts = partsMap(input({ findings: [f("Discos", "Windows ha apuntado 1204 errores graves de disco en 30 días", "bad")] }));
    expect(parts.find((p) => p.id === "disk:ST3500410SV")!.findings).toHaveLength(1);
  });

  it("sin Internet la red sale en rojo; sin batería no hay pieza de batería", () => {
    const parts = partsMap(input({ online: false }));
    expect(parts.find((p) => p.id === "net")!.status).toBe("bad");
    expect(parts.some((p) => p.id === "battery")).toBe(false);
    expect(partsMap(input({ hasBattery: true })).some((p) => p.id === "battery")).toBe(true);
  });

  it("la nota de seguridad decide su color si no hay hallazgos", () => {
    expect(partsMap(input({ securityScore: 55 })).find((p) => p.id === "security")!.status).toBe("bad");
    expect(partsMap(input({ securityScore: null })).find((p) => p.id === "security")!.status).toBe("unknown");
  });
});
