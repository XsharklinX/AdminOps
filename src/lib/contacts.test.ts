import { describe, expect, it } from "vitest";
import type { Contact } from "./api";
import {
  applyFilters,
  cardText,
  EMPTY_CONTACT,
  EMPTY_FILTERS,
  findDuplicates,
  groupContacts,
  isIncomplete,
  parseQuery,
  responsibleFor,
  sortContacts,
  toCsv,
  toVcard,
} from "./contacts";

const c = (patch: Partial<Contact>): Contact => ({ ...EMPTY_CONTACT, id: patch.name ?? "x", created: 1, ...patch });

const ana = c({ id: "a", name: "Ana López", role: "Sistemas", company: "Empresa SA", extension: "2104", email: "ana@empresa.com", tags: ["TI", "Urgencias"], reason: "Altas de usuarios", favorite: true, uses: 9 });
const bea = c({ id: "b", name: "Beatriz Núñez", role: "Compras", company: "Empresa SA", extension: "2205", phone: "910 000 000", tags: ["Compras"] });
const luis = c({ id: "l", name: "Luis Pérez", company: "Proveedor Fibra", mobile: "600 111 222", tags: ["Proveedores", "TI"], reason: "La fibra de la oficina" });
const all = [ana, bea, luis];
const filter = (patch: Partial<typeof EMPTY_FILTERS>) => applyFilters(all, { ...EMPTY_FILTERS, ...patch }).map((x) => x.id);

describe("búsqueda con prefijos", () => {
  it("entiende prefijos, etiquetas, exclusiones y frases", () => {
    expect(parseQuery('ext:21 #ti -compras "empresa sa"')).toEqual([
      { field: "ext", value: "21", not: false },
      { field: "tag", value: "ti", not: false },
      { field: "any", value: "compras", not: true },
      { field: "any", value: "empresa sa", not: false },
    ]);
    expect(parseQuery("es:favorito")[0]).toMatchObject({ field: "fav" });
  });

  it("filtra sin tildes ni mayúsculas", () => {
    expect(filter({ query: "nunez" })).toEqual(["b"]);
    expect(filter({ query: "ext:21" })).toEqual(["a"]);
    expect(filter({ query: "#ti" })).toEqual(["a", "l"]);
    expect(filter({ query: "#ti -fibra" })).toEqual(["a"]);
    expect(filter({ query: "empresa:proveedor" })).toEqual(["l"]);
    expect(filter({ query: "es:favorito" })).toEqual(["a"]);
    // Teléfono con espacios: se busca por los dígitos.
    expect(filter({ query: "600111" })).toEqual(["l"]);
  });

  it("combina etiquetas en modo todas o alguna, empresa y filtros rápidos", () => {
    expect(filter({ tags: ["TI", "Urgencias"], tagMode: "all" })).toEqual(["a"]);
    expect(filter({ tags: ["Compras", "Proveedores"], tagMode: "any" })).toEqual(["b", "l"]);
    expect(filter({ company: "empresa sa" })).toEqual(["a", "b"]);
    expect(filter({ quick: ["withExt"] })).toEqual(["a", "b"]);
    expect(filter({ quick: ["favorites"] })).toEqual(["a"]);
    expect(filter({ quick: ["incomplete"] })).toEqual(["b", "l"]);
  });
});

describe("orden y grupos", () => {
  it("favoritos primero y después el orden elegido", () => {
    expect(sortContacts(all, "name").map((x) => x.id)).toEqual(["a", "b", "l"]);
    expect(sortContacts([bea, luis, ana], "extension").map((x) => x.id)).toEqual(["a", "b", "l"]);
  });

  it("agrupa por etiqueta (un contacto en cada una) y deja «Sin…» al final", () => {
    const groups = groupContacts([...all, c({ id: "z", name: "Zoe" })], "tag");
    expect(groups.map((g) => g.label)).toEqual(["Compras", "Proveedores", "TI", "Urgencias", "Sin etiqueta"]);
    expect(groups.find((g) => g.label === "TI")!.items.map((x) => x.id)).toEqual(["a", "l"]);
    expect(groupContacts(all, "letter").map((g) => g.label)).toEqual(["A", "B", "L"]);
  });
});

describe("duplicados", () => {
  it("agrupa por nombre, correo o teléfono", () => {
    const dupe = c({ id: "a2", name: "ana lopez", email: "otra@x.com" });
    const byPhone = c({ id: "p", name: "Otro", phone: "+34 910 000 000" });
    const groups = findDuplicates([...all, dupe, byPhone]).map((g) => g.map((x) => x.id).sort());
    expect(groups).toContainEqual(["a", "a2"]);
    expect(groups).toContainEqual(["b", "p"]);
    expect(groups).toHaveLength(2);
  });
});

describe("texto para copiar y exportar", () => {
  it("tarjeta con sustituto", () => {
    const t = cardText({ ...ana, availability: "Mañanas" }, bea);
    expect(t).toContain("Ana López — Sistemas, Empresa SA");
    expect(t).toContain("Ext. 2104");
    expect(t).toContain("Si no está: Beatriz Núñez (ext. 2205)");
  });

  it("vCard y CSV escapan lo que hay que escapar", () => {
    const v = toVcard([c({ name: "Pérez; Juan", company: "A, B", notes: "línea1\nlínea2" })]);
    expect(v).toContain("FN:Pérez\\; Juan");
    expect(v).toContain("ORG:A\\, B");
    expect(v).toContain("NOTE:línea1\\nlínea2");
    expect(toCsv([c({ name: 'Con "comillas"; y punto y coma' })]).split("\r\n")[1]).toMatch(/^"Con ""comillas""; y punto y coma"/);
  });

  it("detecta fichas sin completar", () => {
    expect(isIncomplete(ana)).toBe(false);
    expect(isIncomplete(bea)).toBe(true);
  });
});

describe("responsable de un tema", () => {
  it("prioriza etiquetas sobre motivo y cargo", () => {
    const red = c({ id: "r", name: "Red", role: "Técnico de redes" });
    expect(responsibleFor("internet", [...all, red]).map((x) => x.id)).toEqual(["l", "r"]);
    expect(responsibleFor("printer", all)).toEqual([]);
    expect(responsibleFor(undefined, all)).toEqual([]);
  });
});
