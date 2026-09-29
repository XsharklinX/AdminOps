import { describe, expect, it } from "vitest";
import { lastPortalKey, toRetry, unreadFromTitle } from "./portalState";

describe("portales", () => {
  it("lee los mensajes sin leer del título", () => {
    expect(unreadFromTitle("(3) Correo - Ana - Outlook")).toBe(3);
    expect(unreadFromTitle("  (12) Bandeja de entrada")).toBe(12);
    expect(unreadFromTitle("Correo - Ana - Outlook")).toBeNull();
    expect(unreadFromTitle("Reunión (3) de hoy")).toBeNull();
  });

  it("al volver la conexión recarga solo las vistas que fallaron", () => {
    const base = { url: null, title: "", loading: false, slow: false, canBack: false, canForward: false, downloads: [] };
    const list: [string, typeof base & { error: string | null }][] = [
      ["correo", { ...base, error: "Este equipo no tiene conexión a Internet ni a la red." }],
      ["tickets", { ...base, error: null }],
    ];
    expect(toRetry(list)).toEqual(["correo"]);
    expect(toRetry([])).toEqual([]);
  });

  it("recuerda el último portal de cada tipo por separado", () => {
    expect(lastPortalKey("")).toBe("adminops.lastPortal");
    expect(lastPortalKey("inventory")).toBe("adminops.lastPortal.inventory");
    expect(lastPortalKey("mail")).toBe("adminops.lastPortal.mail");
  });
});
