import { describe, expect, it } from "vitest";
import type { UpdateCause, UpdateProgress } from "./api";
import { advance, markTried, nextStep, progressText, showsBar, startLadder, tryLabel } from "./wuRow";

const cause = (fix: string | null): UpdateCause => ({ kind: "esp", sentence: "x", fix });
const prog = (phase: UpdateProgress["phase"], extra: Partial<UpdateProgress> = {}): UpdateProgress => ({
  id: "0a1b2c3d-0000-4000-8000-0123456789ab",
  phase,
  percent: null,
  bytes: null,
  total: null,
  ...extra,
});

describe("wuRow: el escalón que toca según la causa", () => {
  it("empieza por el arreglo de la causa medida", () => {
    expect(startLadder(cause("repair.esp-fonts")).step).toEqual({ kind: "fix", id: "repair.esp-fonts" });
    expect(tryLabel("repair.esp-fonts")).toBe("Probar: liberar la partición reservada");
    expect(tryLabel("repair.wu-services")).toBe("Probar: reiniciar los servicios de Windows Update");
    expect(tryLabel("repair.dism")).toBe("Probar: reparar componentes (DISM)");
    expect(tryLabel("repair.windows-update")).toBe("Probar: restablecer Windows Update");
  });

  it("sin causa o sin arreglo automático no hay botón", () => {
    expect(startLadder(null).step).toBeNull();
    expect(startLadder(cause(null)).step).toBeNull();
  });

  it("«Siguiente paso» recorre la escalera y no repite lo ya probado", () => {
    let l = startLadder(cause("repair.esp-fonts"));
    const seen: string[] = [];
    for (let i = 0; i < 6; i++) {
      const step = nextStep(l);
      if (!step) break;
      seen.push(step.kind === "fix" ? step.id : "manual");
      l = advance(l);
    }
    expect(seen).toEqual(["repair.wu-services", "repair.windows-update", "repair.dism", "repair.sfc", "manual"]);
    // Al llegar al paso manual no hay más.
    expect(l.step?.kind).toBe("manual");
    expect(nextStep(l)).toBeNull();
  });

  it("lo que ya se probó se salta", () => {
    let l = startLadder(cause("repair.esp-fonts"));
    l = markTried(l, "repair.wu-services");
    expect(nextStep(l)).toEqual({ kind: "fix", id: "repair.windows-update" });
  });

  it("una causa de componentes sigue con SFC, no vuelve a DISM", () => {
    const l = startLadder({ kind: "components", sentence: "x", fix: "repair.dism" });
    expect(nextStep(l)).toEqual({ kind: "fix", id: "repair.sfc" });
  });
});

describe("wuRow: el avance de cada actualización", () => {
  it("dice lo que hace con el dato", () => {
    expect(progressText(prog("downloading", { percent: 81, bytes: 54525952, total: 67108864 }))).toBe("Descargando 81 % · 52 MB");
    expect(progressText(prog("downloading", { percent: 0 }))).toBe("Descargando 0 %");
    expect(progressText(prog("installing", { percent: 48 }))).toBe("Instalando 48 %");
    expect(progressText(prog("done"))).toBe("Instalada");
    expect(progressText(prog("reboot"))).toBe("Pide reiniciar");
  });

  it("solo las que están en curso llevan barra", () => {
    expect(showsBar(prog("downloading", { percent: 10 }))).toBe(true);
    expect(showsBar(prog("installing", { percent: 10 }))).toBe(true);
    expect(showsBar(prog("done"))).toBe(false);
    expect(showsBar(prog("failed"))).toBe(false);
  });
});
