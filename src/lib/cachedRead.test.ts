import { describe, expect, it } from "vitest";
import { cachedValue, forget, readCached } from "./cachedRead";

describe("memoria de lecturas lentas", () => {
  it("la primera vez lee; la segunda enseña lo de antes y luego lo nuevo", async () => {
    forget("t");
    const seen: [number, boolean][] = [];
    await readCached("t", async () => 1, (d, fresh) => seen.push([d, fresh]));
    await readCached("t", async () => 2, (d, fresh) => seen.push([d, fresh]));
    expect(seen).toEqual([
      [1, true],
      [1, false],
      [2, true],
    ]);
    expect(cachedValue<number>("t")?.data).toBe(2);
  });

  it("si la lectura falla, el error llega y lo guardado no cambia", async () => {
    forget("u");
    await readCached("u", async () => "bien", () => {});
    await expect(readCached("u", async () => Promise.reject(new Error("no")), () => {})).rejects.toThrow("no");
    expect(cachedValue<string>("u")?.data).toBe("bien");
    forget("u");
    expect(cachedValue("u")).toBeNull();
  });
});
