import { describe, expect, it } from "vitest";
import { matchesMachine } from "./inventoryFilter";
import type { Client, Machine } from "./api";

const row = (os: string, tpm: boolean | null, disks: string) =>
  ({ client: { name: "Asesoría Pérez" } as Client, machine: { host: "PC-01", inventory: { os, tpm, disks, manufacturer: "HP", model: "ProDesk", cpu: "i5", gpu: "", ip: "", serial: "", reasons: [] } } as unknown as Machine });

describe("filtro del inventario", () => {
  it("busca por palabras en la ficha", () => {
    expect(matchesMachine(row("Windows 10 Pro", true, "SSD 256 GB"), "windows 10")).toBe(true);
    expect(matchesMachine(row("Windows 11 Pro", true, "SSD"), "windows 10")).toBe(false);
    expect(matchesMachine(row("Windows 10 Pro", true, "SSD"), "pérez")).toBe(true);
  });
  it("«sin tpm» y «sin ssd» buscan lo que falta", () => {
    expect(matchesMachine(row("Windows 10", false, "HDD 1 TB"), "windows 10 sin tpm")).toBe(true);
    expect(matchesMachine(row("Windows 10", true, "HDD 1 TB"), "sin tpm")).toBe(false);
    expect(matchesMachine(row("Windows 10", true, "HDD 1 TB"), "sin ssd")).toBe(true);
    expect(matchesMachine(row("Windows 10", true, "NVMe 512 GB"), "sin ssd")).toBe(false);
  });
});
