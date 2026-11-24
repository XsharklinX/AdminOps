import { describe, expect, it } from "vitest";
import type { InstallItem } from "./api";
import { canCloseAndGo, counts, initial, mergeItem, statusText, summaryText } from "./installQueue";

const item = (id: string, state: InstallItem["state"], extra: Partial<InstallItem> = {}): InstallItem => ({
  id,
  name: id,
  state,
  percent: null,
  size: null,
  message: "",
  kind: "",
  reboot: false,
  code: null,
  ...extra,
});

describe("installQueue", () => {
  it("dice en qué va cada programa", () => {
    expect(statusText(item("a", "queued"))).toBe("En cola");
    expect(statusText(item("a", "downloading", { percent: 64 }))).toBe("Descargando 64 %");
    expect(statusText(item("a", "downloading"))).toBe("Descargando…");
    expect(statusText(item("a", "installing"))).toBe("Instalando…");
    expect(statusText(item("a", "done"))).toBe("Listo");
    expect(statusText(item("a", "done", { reboot: true }))).toBe("Listo · pide reiniciar");
    expect(statusText(item("a", "skipped"))).toBe("Saltado");
    expect(statusText(item("a", "failed"))).toBe("Falló");
  });

  it("el resumen sale solo cuando no queda nada en curso", () => {
    const running = [item("a", "done"), item("b", "installing")];
    expect(summaryText(running)).toBeNull();
    expect(summaryText([])).toBeNull();
    const end = [item("a", "done"), item("b", "done"), item("c", "done"), item("d", "skipped"), item("e", "failed"), item("f", "failed")];
    expect(summaryText(end)).toBe("Instalación terminada · 3 instalados · 1 saltado · 2 fallaron");
    expect(summaryText([item("a", "done"), item("b", "failed")])).toBe("Instalación terminada · 1 instalado · 0 saltados · 1 falló");
  });

  it("una fila que espera decisión no cierra la instalación, pero un fallo sí cuenta", () => {
    expect(counts([item("a", "failed"), item("b", "queued")])).toEqual({ done: 0, skipped: 0, failed: 1, waiting: 1 });
  });

  it("«Cerrar y seguir» solo si el motivo es cerrar el programa", () => {
    expect(canCloseAndGo(item("a", "failed", { kind: "close-app" }))).toBe(true);
    expect(canCloseAndGo(item("a", "failed", { kind: "network" }))).toBe(false);
    expect(canCloseAndGo(item("a", "done", { kind: "close-app" }))).toBe(false);
  });

  it("mezcla los cambios de fila sin perder el orden", () => {
    const list = [item("a", "queued"), item("b", "queued")];
    const next = mergeItem(list, item("a", "downloading", { percent: 10 }));
    expect(next.map((i) => i.id)).toEqual(["a", "b"]);
    expect(next[0].percent).toBe(10);
    expect(mergeItem(next, item("c", "queued")).map((i) => i.id)).toEqual(["a", "b", "c"]);
  });

  it("la inicial sirve de icono", () => {
    expect(initial("Google Chrome")).toBe("G");
    expect(initial("  7-Zip")).toBe("7");
    expect(initial("???")).toBe("?");
  });
});
