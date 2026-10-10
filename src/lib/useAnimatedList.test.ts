import { describe, expect, it } from "vitest";
import { mergeLeaving, type AnimatedRow } from "./useAnimatedList";

const row = (key: string): AnimatedRow<string> => ({ item: key, key, state: "idle" });

describe("lista animada", () => {
  it("el que se va sigue en su sitio mientras sale", () => {
    const out = mergeLeaving([row("a"), row("c")], [{ item: "b", key: "b", at: 1 }]);
    expect(out.map((r) => `${r.key}:${r.state}`)).toEqual(["a:idle", "b:leave", "c:idle"]);
  });
  it("varios que se van, cada uno donde estaba", () => {
    const out = mergeLeaving([row("b")], [
      { item: "a", key: "a", at: 0 },
      { item: "c", key: "c", at: 2 },
    ]);
    expect(out.map((r) => r.key)).toEqual(["a", "b", "c"]);
  });
  it("si vuelve a aparecer, no se duplica", () => {
    const out = mergeLeaving([row("a")], [{ item: "a", key: "a", at: 0 }]);
    expect(out).toHaveLength(1);
    expect(out[0].state).toBe("idle");
  });
  it("una posición pasada del final no rompe nada", () => {
    expect(mergeLeaving([row("a")], [{ item: "z", key: "z", at: 9 }]).map((r) => r.key)).toEqual(["a", "z"]);
  });
});
