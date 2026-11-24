import { describe, expect, it } from "vitest";
import type { RecipeStep } from "./api";
import { riskyTweaks, withTweakIds, type TweakInfo } from "./recipeEdit";

const catalog = [
  { id: "a", name: "Ajuste A" },
  { id: "b", name: "Ajuste B" },
  { id: "c", name: "Ajuste C" },
];

describe("recipeEdit", () => {
  it("al cambiar los ids, los nombres se recalculan", () => {
    const step = { kind: "tweaks" as const, ids: ["a", "b"], names: ["Viejo A", "Viejo B"] };
    // Se quita «a» y se añade «c»: «b» conserva su nombre y «c» sale del catálogo.
    const out = withTweakIds(step, ["b", "c"], catalog.filter((t) => t.id === "c"));
    expect(out.ids).toEqual(["b", "c"]);
    expect(out.names).toEqual(["Viejo B", "Ajuste C"]);
    expect(out.names).toHaveLength(out.ids.length);
  });

  it("sin nombres guardados no se inventan", () => {
    const out = withTweakIds({ kind: "tweaks", ids: ["a"] }, ["a", "b"], catalog);
    expect(out.names).toBeUndefined();
    expect(out.ids).toEqual(["a", "b"]);
  });

  it("un id que no está en ningún sitio se queda con el id", () => {
    const out = withTweakIds({ kind: "tweaks", ids: [], names: [] }, ["zzz"], []);
    expect(out.names).toEqual(["zzz"]);
  });

  const index: TweakInfo[] = [
    { id: "a", name: "A", risk: "low", note: null },
    { id: "b", name: "B", risk: "medium", note: "Cambia algo" },
    { id: "c", name: "C", risk: "high", note: null },
  ];

  it("lista los ajustes de riesgo medio o alto, sin repetir", () => {
    const steps: RecipeStep[] = [{ kind: "restorePoint" }, { kind: "tweaks", ids: ["a", "b"] }, { kind: "tweaks", ids: ["b", "c", "nada"] }];
    expect(riskyTweaks(steps, index).map((t) => t.id)).toEqual(["b", "c"]);
  });

  it("una plantilla solo con ajustes de riesgo bajo no pide confirmar", () => {
    expect(riskyTweaks([{ kind: "tweaks", ids: ["a"] }, { kind: "diagnostics" }], index)).toEqual([]);
    expect(riskyTweaks([], index)).toEqual([]);
  });
});
