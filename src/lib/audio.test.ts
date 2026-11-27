import { describe, expect, it } from "vitest";
import { advise, gadget, privacyProblems } from "./audio";

const dev = (id: string, name: string, extra: Partial<{ default: boolean; defaultComm: boolean; volume: number; muted: boolean }> = {}) => ({ id, name, default: false, defaultComm: false, volume: 80, muted: false, ...extra });
const okPrivacy = { device: true, apps: true, desktop: true, teamsNew: true };

describe("audio", () => {
  it("empareja micro y cascos por el aparato", () => {
    expect(gadget("Micrófono (Jabra Evolve 20)")).toBe("Jabra Evolve 20");
    expect(gadget("Altavoces")).toBe("Altavoces");
    // Los nombres reales de Realtek llevan paréntesis dentro.
    expect(gadget("Micrófono (Realtek(R) Audio)")).toBe("Realtek(R) Audio");
    expect(gadget("Altavoces (Realtek(R) Audio)")).toBe(gadget("Micrófono (Realtek(R) Audio)"));
  });

  it("si el predeterminado no oye y otro sí, propone el otro con sus cascos", () => {
    const view = {
      capture: [dev("w", "Micrófono (Webcam C270)", { default: true, defaultComm: true, volume: 4 }), dev("j", "Micrófono (Jabra Evolve 20)")],
      render: [dev("r", "Altavoces (Realtek)", { default: true }), dev("jr", "Altavoces (Jabra Evolve 20)")],
      privacy: okPrivacy,
    };
    const a = advise(view, { w: 0.001, j: 0.3 });
    expect(a.problem).toBe("El predeterminado es «Webcam C270» y no te oye; «Jabra Evolve 20» sí.");
    expect(a.mic?.id).toBe("j");
    expect(a.speaker?.id).toBe("jr");
  });

  it("el predeterminado bajo se sube", () => {
    const view = { capture: [dev("w", "Micrófono (Realtek)", { default: true, defaultComm: true, volume: 4 })], render: [], privacy: okPrivacy };
    expect(advise(view, { w: 0.2 }).problem).toBe("«Realtek» está al 4 %.");
  });

  it("todo bien", () => {
    const view = { capture: [dev("w", "Micrófono (Realtek)", { default: true, defaultComm: true })], render: [], privacy: okPrivacy };
    expect(advise(view, { w: 0.2 }).problem).toBe("");
  });

  it("permisos", () => {
    expect(privacyProblems({ ...okPrivacy, teamsNew: false })).toEqual(["Teams no tiene permiso de micrófono."]);
    expect(privacyProblems({ ...okPrivacy, device: false, teamsNew: false })).toEqual(["El micrófono está apagado para todo el equipo."]);
    expect(privacyProblems(okPrivacy)).toEqual([]);
  });
});
