import { describe, expect, it } from "vitest";
import { busyPages, isBusy, markBusy, MAX_ALIVE, touchAlive, trimAlive } from "./alive";

describe("touchAlive", () => {
  it("la última visitada va primera y no se repite", () => {
    expect(touchAlive(["a", "b", "c"], "c", 3)).toEqual(["c", "a", "b"]);
  });
  it("descarta la más antigua al pasar del límite", () => {
    expect(touchAlive(["a", "b", "c"], "d", 3)).toEqual(["d", "a", "b"]);
  });
  it("no descarta la pantalla de la derecha de la vista dividida", () => {
    expect(touchAlive(["a", "b", "c"], "d", 3, "c")).toEqual(["d", "a", "c"]);
  });
  it("conserva las páginas ocupadas aunque sobrepasen el límite", () => {
    expect(touchAlive(["a", "b", "c"], "d", 3, null, ["b"])).toEqual(["d", "a", "b"]);
  });
  it("conserva keep y busy a la vez", () => {
    expect(touchAlive(["a", "b", "c"], "d", 2, "c", ["b"])).toEqual(["d", "b", "c"]);
  });
  it("el límite es de 3", () => {
    expect(MAX_ALIVE).toBe(3);
  });
});

describe("trimAlive", () => {
  it("deja la lista como está si cabe", () => {
    expect(trimAlive(["a", "b"], 3)).toEqual(["a", "b"]);
  });
  it("nunca quita la primera", () => {
    expect(trimAlive(["a", "b", "c"], 1, "b")).toEqual(["a", "b"]);
  });
});

describe("markBusy", () => {
  it("devuelve las páginas ocupadas", () => {
    markBusy("x", true);
    expect(isBusy("x")).toBe(true);
    expect(busyPages()).toContain("x");
    markBusy("x", false);
    expect(isBusy("x")).toBe(false);
    expect(busyPages()).not.toContain("x");
  });
  it("usa un contador para varias tareas en la misma página", () => {
    markBusy("y", true);
    markBusy("y", true);
    markBusy("y", false);
    expect(isBusy("y")).toBe(true);
    markBusy("y", false);
    expect(isBusy("y")).toBe(false);
  });
});
