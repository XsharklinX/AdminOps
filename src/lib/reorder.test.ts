import { describe, expect, it } from "vitest";
import { moveBefore, shiftItem } from "./reorder";

const id = (s: string) => s;

describe("reordenar", () => {
  it("pone un elemento antes de otro", () => {
    expect(moveBefore(["a", "b", "c", "d"], "d", "b", id)).toEqual(["a", "d", "b", "c"]);
    expect(moveBefore(["a", "b", "c", "d"], "a", "d", id)).toEqual(["b", "c", "a", "d"]);
  });
  it("al final si no hay destino", () => {
    expect(moveBefore(["a", "b", "c"], "a", null, id)).toEqual(["b", "c", "a"]);
  });
  it("no cambia nada si el elemento o el destino no existen, o son el mismo", () => {
    const l = ["a", "b"];
    expect(moveBefore(l, "x", "a", id)).toBe(l);
    expect(moveBefore(l, "a", "x", id)).toBe(l);
    expect(moveBefore(l, "a", "a", id)).toBe(l);
  });
  it("sube y baja una posición, sin salirse", () => {
    expect(shiftItem(["a", "b", "c"], "b", -1, id)).toEqual(["b", "a", "c"]);
    expect(shiftItem(["a", "b", "c"], "b", 1, id)).toEqual(["a", "c", "b"]);
    const l = ["a", "b"];
    expect(shiftItem(l, "a", -1, id)).toBe(l);
    expect(shiftItem(l, "b", 1, id)).toBe(l);
  });
});
