import { describe, expect, it } from "vitest";
import { isPageId } from "../components/Sidebar";
import { RELEASES } from "./changelog";
import { FAQ, GLOSSARY, GUIDE } from "./guide";
import { parseTerms, searchHelp } from "./help";
import pkg from "../../package.json";
import terms from "../../src-tauri/terminos.txt?raw";

describe("guía", () => {
  it("cada «Abrir esta pantalla» lleva a una página que existe", () => {
    for (const c of GUIDE) for (const t of c.topics) if (t.go) expect(isPageId(t.go[0]), `${c.id}/${t.id} → ${t.go[0]}`).toBe(true);
  });

  it("no hay apartados ni términos repetidos, y ninguno vacío", () => {
    const ids = GUIDE.flatMap((c) => c.topics.map((t) => `${c.id}/${t.id}`));
    expect(new Set(ids).size).toBe(ids.length);
    const words = GLOSSARY.map((g) => g.term.toLowerCase());
    expect(new Set(words).size).toBe(words.length);
    for (const c of GUIDE) for (const t of c.topics) expect(t.what.length, t.id).toBeGreaterThan(40);
    expect(FAQ.length).toBeGreaterThan(8);
  });

  it("el glosario va en orden alfabético", () => {
    const terms = GLOSSARY.map((g) => g.term);
    expect([...terms].sort((a, b) => a.localeCompare(b, "es"))).toEqual(terms);
  });

  it("busca sin tildes, en toda la ayuda, con el título primero", () => {
    const r = searchHelp("diagnostico");
    expect(r.topics[0].topic.title).toBe("Diagnóstico");
    expect(r.glossary.some((g) => g.term === "Diagnóstico")).toBe(true);
    expect(searchHelp("olvide pin").faq).toHaveLength(1);
    expect(searchHelp("  ")).toEqual({ topics: [], glossary: [], faq: [] });
    expect(searchHelp("xyzquenoexiste").topics).toEqual([]);
  });
});

describe("novedades", () => {
  it("la primera entrada es la versión actual, sin repetidas y sin fechas", () => {
    expect(RELEASES[0].version).toBe(pkg.version);
    const versions = RELEASES.map((r) => r.version);
    expect(new Set(versions).size).toBe(versions.length);
    for (const r of RELEASES) {
      expect(r.added.length + (r.fixed?.length ?? 0) + (r.removed?.length ?? 0), r.version).toBeGreaterThan(0);
      expect(/\b20\d\d\b/.test(JSON.stringify(r)), `${r.version} lleva un año`).toBe(false);
    }
  });

  it("van de la más nueva a la más antigua", () => {
    const num = (v: string) => v.split(".").map(Number);
    const older = (a: string, b: string) => {
      const [x, y] = [num(a), num(b)];
      for (let i = 0; i < 3; i++) if ((x[i] ?? 0) !== (y[i] ?? 0)) return (x[i] ?? 0) < (y[i] ?? 0);
      return false;
    };
    for (let i = 1; i < RELEASES.length; i++) expect(older(RELEASES[i].version, RELEASES[i - 1].version), RELEASES[i].version).toBe(true);
  });
});

describe("términos de uso", () => {
  it("se parten en título y secciones numeradas", () => {
    const blocks = parseTerms(terms);
    expect(blocks[0]).toMatchObject({ heading: true, text: "TÉRMINOS DE USO DE ADMINOPS" });
    const headings = blocks.filter((b) => b.heading).map((b) => b.text);
    expect(headings).toContain("5. Sin garantía");
    expect(headings).toHaveLength(12);
    expect(blocks.find((b) => b.text.startsWith("Al instalar"))?.heading).toBe(false);
  });
});

describe("apartados de la guía", () => {
  it("cada apartado tiene un id propio (Ctrl+K lleva a él por el id)", () => {
    const ids = GUIDE.flatMap((c) => c.topics.map((t) => t.id));
    expect(new Set(ids).size).toBe(ids.length);
  });
});
