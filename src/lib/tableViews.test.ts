// Las vistas guardadas recuerdan columnas, orden y ordenación.
import { afterEach, describe, expect, it, vi } from "vitest";
import { loadViews, upsertView } from "../components/SavedViews";
import type { ColInfo } from "./colset";
import { applyLayout, captureLayout, layoutOf, registerTable, sanitizeLayout, type TableLayout } from "./tableViews";

const infos: ColInfo[] = [{ id: "name", locked: true }, { id: "cpu" }, { id: "ram" }, { id: "path", hidden: true }];
const known = infos.map((c) => c.id);

describe("guardar y restaurar", () => {
  it("guarda el estado actual: columnas que se ven, su orden y la ordenación", () => {
    const saved = { order: ["name", "ram", "cpu", "path"], shown: ["name", "ram", "path"] };
    const layout = layoutOf(infos, saved, { id: "ram", desc: true });
    expect(layout).toEqual({ cols: { order: ["name", "ram", "cpu", "path"], shown: ["name", "ram", "path"] }, sort: { id: "ram", desc: true } });
  });

  it("sin nada tocado guarda las de siempre", () => {
    expect(layoutOf(infos, null, null)).toEqual({ cols: { order: known, shown: ["name", "cpu", "ram"] }, sort: null });
  });

  it("lo guardado vuelve igual tras pasar por el almacenamiento", () => {
    const layout = layoutOf(infos, { order: ["name", "ram", "cpu", "path"], shown: ["name", "ram"] }, { id: "cpu", desc: false });
    expect(sanitizeLayout(JSON.parse(JSON.stringify(layout)), known)).toEqual(layout);
  });

  it("la vista guarda el layout junto al filtro y lo conserva al sustituirla", () => {
    const layout: TableLayout = { cols: { order: known, shown: ["name"] }, sort: { id: "cpu", desc: true } };
    const views = upsertView([], "Pesados", { q: "x" }, layout);
    expect(views).toEqual([{ name: "Pesados", value: { q: "x" }, pinned: false, layout }]);
  });

  it("la tabla registrada entrega su estado y recibe el de la vista", () => {
    let got: TableLayout | null = null;
    const off = registerTable("t1", { get: () => ({ cols: { order: known, shown: ["name"] }, sort: null }), apply: (l) => (got = l) });
    expect(captureLayout("t1")).toEqual({ cols: { order: known, shown: ["name"] }, sort: null });
    applyLayout("t1", { sort: { id: "ram", desc: false } });
    expect(got).toEqual({ sort: { id: "ram", desc: false } });
    off();
    expect(captureLayout("t1")).toBeUndefined();
    expect(captureLayout(undefined)).toBeUndefined();
  });
});

describe("vistas antiguas, sin columnas", () => {
  afterEach(() => vi.unstubAllGlobals());

  it("se cargan tal cual y abrirlas no toca la tabla", () => {
    const mem = new Map<string, string>([["adminops.views.p", JSON.stringify([{ name: "Vieja", value: { q: "a" }, pinned: true }])]]);
    vi.stubGlobal("localStorage", { getItem: (k: string) => mem.get(k) ?? null });
    const [v] = loadViews<{ q: string }>("p");
    expect(v.layout).toBeUndefined();
    let touched = false;
    const off = registerTable("t2", { get: () => ({}), apply: () => (touched = true) });
    applyLayout("t2", v.layout);
    off();
    expect(touched).toBe(false);
  });

  it("un layout sin columnas ni ordenación no cambia nada", () => {
    expect(sanitizeLayout({}, known)).toEqual({});
    expect(sanitizeLayout(undefined, known)).toEqual({});
    expect(sanitizeLayout("basura", known)).toEqual({});
  });
});

describe("columna desconocida", () => {
  it("se ignora sin fallar, en columnas y en ordenación", () => {
    const out = sanitizeLayout({ cols: { order: ["name", "vieja", "cpu"], shown: ["name", "vieja"] }, sort: { id: "vieja", desc: true } }, known);
    expect(out).toEqual({ cols: { order: ["name", "cpu"], shown: ["name"] } });
  });

  it("columnas con forma rara no rompen", () => {
    expect(sanitizeLayout({ cols: { order: "x", shown: null }, sort: 5 }, known)).toEqual({});
    expect(sanitizeLayout({ cols: { order: [1, "cpu"], shown: ["cpu"] }, sort: null }, known)).toEqual({ cols: { order: ["cpu"], shown: ["cpu"] }, sort: null });
  });
});
