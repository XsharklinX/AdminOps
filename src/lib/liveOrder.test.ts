import { describe, expect, it } from "vitest";
import { applyFrozenOrder } from "./liveOrder";

const key = (r: { id: string }) => r.id;
const rows = (...ids: string[]) => ids.map((id) => ({ id }));

describe("congelar el orden de una lista en vivo", () => {
  it("mantiene el orden de antes aunque lleguen datos en otro orden", () => {
    const now = rows("chrome", "teams", "outlook");
    expect(applyFrozenOrder(now, ["teams", "chrome", "outlook"], key).map(key)).toEqual(["teams", "chrome", "outlook"]);
  });

  it("los datos nuevos de cada fila se ven (se congela el orden, no los datos)", () => {
    const a = { id: "teams", cpu: 31 };
    const b = { id: "chrome", cpu: 17 };
    const out = applyFrozenOrder([a, b], ["chrome", "teams"], key);
    expect(out).toEqual([b, a]);
    expect(out[1].cpu).toBe(31);
  });

  it("las filas nuevas van al final y las que desaparecen se quitan", () => {
    const now = rows("b", "nueva1", "a", "nueva2");
    expect(applyFrozenOrder(now, ["a", "viejo", "b"], key).map(key)).toEqual(["a", "b", "nueva1", "nueva2"]);
  });

  it("sin orden previo, la lista sale como llega", () => {
    const now = rows("x", "y");
    expect(applyFrozenOrder(now, [], key).map(key)).toEqual(["x", "y"]);
  });
});
