// Lo que no depende de React en las plantillas de «Preparar equipos»: los nombres de los
// ajustes de un paso y qué ajustes de riesgo hay que confirmar antes de ejecutar.
import type { Risk, RecipeStep } from "./api";

export interface TweakInfo {
  id: string;
  name: string;
  risk: Risk;
  note: string | null;
}

type TweaksStep = Extract<RecipeStep, { kind: "tweaks" }>;

/**
 * El paso de ajustes con otra lista de ids. Si el paso traía nombres (vienen de un caso),
 * se recalculan para la lista nueva: con el catálogo, o con el nombre que ya tenía ese id.
 */
export function withTweakIds(step: TweaksStep, ids: string[], catalog: readonly { id: string; name: string }[]): TweaksStep {
  if (!step.names) return { ...step, ids };
  const old = step.names;
  const names = ids.map((id) => catalog.find((t) => t.id === id)?.name ?? old[step.ids.indexOf(id)] ?? id);
  return { ...step, ids, names };
}

/** Los ajustes de riesgo medio o alto que lleva la plantilla, sin repetir, en el orden de los pasos. */
export function riskyTweaks(steps: readonly RecipeStep[], index: readonly TweakInfo[]): TweakInfo[] {
  const out: TweakInfo[] = [];
  for (const s of steps) {
    if (s.kind !== "tweaks") continue;
    for (const id of s.ids) {
      const t = index.find((x) => x.id === id);
      if (t && t.risk !== "low" && !out.some((o) => o.id === id)) out.push(t);
    }
  }
  return out;
}
