// Ctrl+K lleva a la pantalla nueva con lo que la gente escribe de verdad: «monitor borroso»,
// «la ñ no sale», «excel no se guardó». Las pantallas se encuentran por sus palabras (sections.ts).
import { describe, expect, it } from "vitest";
import { searchEntries } from "../components/CommandPalette";
import { SECTIONS } from "./sections";

const norm = (s: string) => s.normalize("NFD").replace(/[̀-ͯ]/g, "").toLowerCase();

/** Las mismas entradas de sección que arma Ctrl+K (título, y para buscar: nombre + palabras). */
const entries = Object.entries(SECTIONS).flatMap(([page, list]) =>
  (list ?? []).map((s) => ({ key: `section:${page}:${s.id}`, kind: "section", title: s.label, search: norm(`${s.label} ${page} ${s.keywords ?? ""}`), run: () => undefined })),
);

const firstFor = (typed: string) => searchEntries(entries as never, norm(typed))[0]?.key;

describe("Ctrl+K encuentra las pantallas nuevas", () => {
  it("«monitor borroso» lleva a Pantallas", () => {
    expect(firstFor("monitor borroso")).toBe("section:machine:displays");
    expect(firstFor("monitor estirado")).toBe("section:machine:displays");
    expect(firstFor("segundo monitor no detecta")).toBe("section:machine:displays");
    expect(firstFor("pantalla estirada")).toBe("section:machine:displays");
    expect(firstFor("no detecta el segundo monitor")).toBe("section:machine:displays");
  });

  it("«la ñ no sale» lleva a Teclado e idioma", () => {
    expect(firstFor("la ñ no sale")).toBe("section:machine:keyboard");
    expect(firstFor("teclado en ingles")).toBe("section:machine:keyboard");
    expect(firstFor("no sale el arroba")).toBe("section:machine:keyboard");
    expect(firstFor("arroba en otro sitio")).toBe("section:machine:keyboard");
  });

  it("«excel no se guardó» lleva a Recuperar archivos", () => {
    expect(firstFor("excel no se guardó")).toBe("section:data:recover");
    expect(firstFor("word se cerró sin guardar")).toBe("section:data:recover");
    expect(firstFor("autorrecuperación")).toBe("section:data:recover");
  });

  it("«escanear a carpeta» y «configurar escaner ricoh» llevan a Escáneres", () => {
    expect(firstFor("escanear a carpeta")).toBe("section:printers:scanners");
    expect(firstFor("configurar escaner ricoh")).toBe("section:printers:scanners");
    expect(firstFor("configurar escáner epson")).toBe("section:printers:scanners");
  });

  it("«internet va lento», «quién usa internet» y «onedrive se come la subida» llevan a Uso de la red", () => {
    expect(firstFor("internet va lento")).toBe("section:router:usage");
    expect(firstFor("quién usa internet")).toBe("section:router:usage");
    expect(firstFor("onedrive se come la subida")).toBe("section:router:usage");
    expect(firstFor("onedrive sincronizando")).toBe("section:router:usage");
  });

  it("con una errata también", () => {
    expect(firstFor("monitor borrosso")).toBe("section:machine:displays");
  });
});
