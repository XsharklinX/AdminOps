import { describe, expect, it } from "vitest";
import { answer, candidates, culprit, maxQuestions, plan, startBisect, toUndo, type Candidate } from "./bisect";

/** Un técnico simulado: el problema desaparece cuando el culpable está deshecho. */
function search(count: number, bad: number) {
  let s = startBisect(count);
  let guard = 0;
  while (culprit(s) === null && guard++ < 50) {
    s = answer(s, toUndo(s).includes(bad));
  }
  return { s, found: culprit(s), questions: s.asked };
}

describe("bisect", () => {
  it("encuentra al culpable en cualquier posición con ceil(log2 n) preguntas como mucho", () => {
    for (const n of [2, 3, 5, 8, 9, 13, 20]) {
      for (let bad = 0; bad < n; bad++) {
        const r = search(n, bad);
        expect(r.found).toBe(bad);
        expect(r.questions).toBeLessThanOrEqual(maxQuestions(n));
      }
    }
  });

  it("con 8 cambios bastan 3 preguntas", () => {
    expect(maxQuestions(8)).toBe(3);
    expect(search(8, 1).questions).toBe(3);
  });

  it("un solo cambio no necesita preguntas", () => {
    expect(maxQuestions(1)).toBe(0);
    expect(culprit(startBisect(1))).toBe(0);
    expect(culprit(startBisect(0))).toBeNull();
  });

  it("al terminar solo queda deshecho el culpable", () => {
    const { s } = search(8, 5);
    expect(toUndo(s)).toEqual([5]);
  });

  it("answer no se mueve cuando ya hay culpable", () => {
    const s = startBisect(1);
    expect(answer(s, true)).toBe(s);
  });

  it("plan: deshace lo que falta y rehace lo que sobra", () => {
    expect(plan([], [0, 1])).toEqual({ undo: [0, 1], redo: [] });
    expect(plan([0, 1], [2, 3])).toEqual({ undo: [2, 3], redo: [0, 1] });
    expect(plan([0, 1], [1, 2])).toEqual({ undo: [2], redo: [0] });
    expect(plan([4], [4])).toEqual({ undo: [], redo: [] });
  });

  it("el plan paso a paso termina con el culpable deshecho y lo demás aplicado", () => {
    let s = startBisect(6);
    let undone: number[] = [];
    while (culprit(s) === null) {
      const want = toUndo(s);
      const p = plan(undone, want);
      undone = [...undone.filter((i) => !p.redo.includes(i)), ...p.undo];
      s = answer(s, undone.includes(3));
    }
    const p = plan(undone, toUndo(s));
    undone = [...undone.filter((i) => !p.redo.includes(i)), ...p.undo];
    expect(undone).toEqual([3]);
  });
});

describe("candidates", () => {
  const e = (id: number, tweakId: string | null, o: Partial<Candidate> = {}): Candidate => ({
    id,
    timestamp: 1000 + id,
    op: "apply",
    tweakId,
    title: `t${id}`,
    ok: true,
    reverted: false,
    undoable: true,
    ...o,
  });

  it("solo entran los que se pueden deshacer y rehacer, desde la hora dada, del más antiguo al más nuevo", () => {
    const list = [
      e(3, "c"),
      e(2, "b", { reverted: true }),
      e(1, "a"),
      e(4, null),
      e(5, "d", { undoable: false }),
      e(6, "f", { ok: false }),
      e(7, "g", { op: "run" }),
      e(8, "viejo", { timestamp: 10 }),
    ];
    expect(candidates(list, 500).map((x) => x.tweakId)).toEqual(["a", "c"]);
  });

  it("un ajuste aplicado dos veces cuenta una vez, con la última", () => {
    const r = candidates([e(1, "a"), e(5, "a")], 0);
    expect(r).toHaveLength(1);
    expect(r[0].id).toBe(5);
  });
});
