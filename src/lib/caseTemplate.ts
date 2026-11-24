// «Guardar como plantilla»: de lo que se hizo en el caso a los pasos de una plantilla.
// Lo que no depende de React, para poder probarlo.
import type { Case, RecipeStep, TemplateDraft, TemplateItem } from "./api";

/**
 * «Sacada del caso #4521» o, sin número de caso, «Sacada de un caso del 20 de noviembre de 2026».
 * Nunca lleva el nombre de la persona ni del equipo: la plantilla se comparte y se reutiliza.
 */
export function caseSource(c: Pick<Case, "ticket" | "started">): string {
  const ticket = c.ticket.trim();
  if (ticket) return `Sacada del caso ${ticket.startsWith("#") ? ticket : `#${ticket}`}`;
  if (!(c.started > 0)) return "Sacada de un caso";
  const date = new Date(c.started * 1000).toLocaleDateString("es", { day: "numeric", month: "long", year: "numeric" });
  return `Sacada de un caso del ${date}`;
}

/** Los ajustes con riesgo medio o alto salen desmarcados: no se repiten sin que alguien los elija. */
export function checkedByDefault(item: Pick<TemplateItem, "repeatable" | "risk">): boolean {
  return item.repeatable && (!item.risk || item.risk === "low");
}

/** Los pasos de la plantilla a partir de lo que se marcó, en el orden en que se hizo. */
export function stepsFromDraft(draft: TemplateDraft, picked: ReadonlySet<number>): RecipeStep[] {
  const steps: RecipeStep[] = [];
  draft.items.forEach((item, i) => {
    if (!item.repeatable || !picked.has(i)) return;
    if (item.step === "restorePoint") steps.push({ kind: "restorePoint" });
    else if (item.step === "tweak" && item.tweakId) steps.push({ kind: "tweaks", ids: [item.tweakId], names: [item.title] });
  });
  return steps;
}
