import { describe, expect, it } from "vitest";
import type { TemplateDraft } from "./api";
import { caseSource, checkedByDefault, stepsFromDraft } from "./caseTemplate";

const draft: TemplateDraft = {
  items: [
    { title: "Limpiar temporales del usuario", step: "tweak", tweakId: "cleanup.user-temp", repeatable: true, reason: null, risk: "low", note: null },
    { title: "Dominio: desbloquear la cuenta de ana", step: "", tweakId: null, repeatable: false, reason: "Solo valía para este equipo", risk: null, note: null },
    { title: "Punto de restauración", step: "restorePoint", tweakId: null, repeatable: true, reason: null, risk: null, note: null },
    { title: "Plan de energía Alto rendimiento", step: "tweak", tweakId: "performance.power-plan", repeatable: true, reason: null, risk: "medium", note: "Cambia el plan de energía" },
  ],
  removed: ["Reparar Office"],
};

describe("caseTemplate", () => {
  it("convierte lo marcado en pasos, en orden", () => {
    const steps = stepsFromDraft(draft, new Set([0, 2, 3]));
    expect(steps).toEqual([
      { kind: "tweaks", ids: ["cleanup.user-temp"], names: ["Limpiar temporales del usuario"] },
      { kind: "restorePoint" },
      { kind: "tweaks", ids: ["performance.power-plan"], names: ["Plan de energía Alto rendimiento"] },
    ]);
  });

  it("lo desmarcado y lo que no se puede repetir no entran, aunque estén marcados", () => {
    expect(stepsFromDraft(draft, new Set([1]))).toEqual([]);
    expect(stepsFromDraft(draft, new Set([0]))).toHaveLength(1);
    expect(stepsFromDraft(draft, new Set())).toEqual([]);
  });

  it("dice de dónde sale: número de caso o, sin él, la fecha (nunca persona ni equipo)", () => {
    expect(caseSource({ ticket: "#4521", started: 1_790_000_000 })).toBe("Sacada del caso #4521");
    expect(caseSource({ ticket: "4521", started: 0 })).toBe("Sacada del caso #4521");
    const sin = caseSource({ ticket: "", started: 1_790_000_000 });
    expect(sin).toMatch(/^Sacada de un caso del \d{1,2} de \p{L}+ de 2026$/u);
    expect(caseSource({ ticket: " ", started: 0 })).toBe("Sacada de un caso");
    // Aunque el caso tenga persona y equipo, no salen.
    const conPersona = caseSource({ ticket: "", started: 1_790_000_000, person: "Ana Pérez", machine: "PC-1" } as Parameters<typeof caseSource>[0]);
    expect(conPersona).not.toMatch(/Ana|PC-1/);
  });

  it("lo de riesgo medio o alto sale desmarcado; lo de riesgo bajo y el punto de restauración, marcados", () => {
    const marcados = draft.items.map((i) => checkedByDefault(i));
    expect(marcados).toEqual([true, false, true, false]);
    expect(checkedByDefault({ repeatable: true, risk: "high" })).toBe(false);
    expect(checkedByDefault({ repeatable: true, risk: null })).toBe(true);
  });
});
