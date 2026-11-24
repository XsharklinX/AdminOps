import { describe, expect, it } from "vitest";
import { createElement } from "react";
import { isFilterKey, matchesFilter, nodeText, normalizeText } from "./typeFilter";

describe("filtro al escribir", () => {
  it("no distingue mayúsculas ni tildes", () => {
    expect(normalizeText("Configuración")).toBe("configuracion");
    expect(matchesFilter("Administrador de Tareas · Configuración", "CONFIGURACION")).toBe(true);
    expect(matchesFilter("Peñón", "penon")).toBe(true);
    expect(matchesFilter("Google Chrome", "crome")).toBe(false);
  });

  it("varias palabras: todas tienen que aparecer, en cualquier orden", () => {
    expect(matchesFilter("Google Chrome 131", "chrome goo")).toBe(true);
    expect(matchesFilter("Google Chrome 131", "chrome edge")).toBe(false);
    expect(matchesFilter("lo que sea", "   ")).toBe(true);
  });

  it("saca el texto de las celdas con elementos dentro", () => {
    const cell = createElement("span", null, "Teams ", createElement("b", null, "2 ventanas"), 42, null, false);
    expect(nodeText(cell)).toBe("Teams 2 ventanas 42");
    expect(nodeText(["a", ["b", 3]])).toBe("a b 3");
  });

  it("solo valen los caracteres sueltos sin Ctrl, Alt ni Meta (Mayús sí)", () => {
    const k = (key: string, o: Partial<KeyboardEvent> = {}) => isFilterKey({ key, ctrlKey: false, altKey: false, metaKey: false, isComposing: false, ...o });
    expect(k("a")).toBe(true);
    expect(k("A")).toBe(true);
    expect(k("ñ")).toBe(true);
    expect(k("a", { ctrlKey: true })).toBe(false);
    expect(k("a", { altKey: true })).toBe(false);
    expect(k("/")).toBe(false);
    expect(k("Enter")).toBe(false);
    expect(k("F5")).toBe(false);
    expect(k("a", { isComposing: true })).toBe(false);
  });
});
