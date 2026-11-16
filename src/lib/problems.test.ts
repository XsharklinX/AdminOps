import { describe, expect, it } from "vitest";
import type { JournalEntry, Problem } from "./api";
import { problemColumns, problemStory } from "./problems";

const prob = (key: string, over: Partial<Problem> = {}): Problem => ({
  key,
  title: key,
  area: "Sistema",
  severity: "warn",
  firstSeen: 1000,
  lastSeen: 2000,
  resolvedAt: null,
  fixes: ["repair.sfc"],
  events: [{ at: 1000, kind: "detected" }],
  ...over,
});
const j = (tweakId: string, timestamp: number, ok = true): JournalEntry => ({ id: timestamp, timestamp, op: "run", tweakId, title: `Arreglo ${tweakId}`, ok, message: null, reverted: false, undoable: false });

describe("problemas de este equipo", () => {
  it("abierto si no se probó nada; en curso si sí; resuelto si dejó de salir", () => {
    const list = [prob("a"), prob("b"), prob("c", { resolvedAt: 3000 }), prob("d", { severity: "bad" })];
    const cols = problemColumns(list, [j("repair.sfc", 1500)].map((x) => ({ ...x })), 4000);
    // «b» y «a» tienen el mismo arreglo: los dos cuentan como en curso.
    expect(cols.working.map((p) => p.key).sort()).toEqual(["a", "b", "d"]);
    expect(cols.solved.map((p) => p.key)).toEqual(["c"]);
    expect(problemColumns([prob("x", { fixes: [] })], [j("repair.sfc", 1500)], 4000).open.map((p) => p.key)).toEqual(["x"]);
  });

  it("no cuenta arreglos de antes de que apareciera el problema", () => {
    expect(problemColumns([prob("a")], [j("repair.sfc", 10)], 4000).open).toHaveLength(1);
  });

  it("cuenta la historia en orden, con lo que se probó", () => {
    const p = prob("a", { resolvedAt: 3000, events: [{ at: 1000, kind: "detected" }, { at: 3000, kind: "resolved" }] });
    const story = problemStory(p, [j("repair.sfc", 1500, false), j("repair.sfc", 2500)]);
    expect(story.map((s) => s.text)).toEqual(["Apareció en el diagnóstico", "Probado: Arreglo repair.sfc (falló)", "Probado: Arreglo repair.sfc", "Dejó de salir: resuelto"]);
    expect(story[3].tone).toBe("ok");
  });
});
