import { describe, expect, it } from "vitest";
import { completePhrase, parseAction, parseSteps, recognize, splitPhrase } from "./recognize";

describe("reconocer lo que se escribe en Ctrl+K", () => {
  it("equipos", () => {
    expect(recognize("PC-CONTA-03")).toEqual({ kind: "computer", value: "PC-CONTA-03", sure: true });
    expect(recognize("pc-conta-03")?.value).toBe("PC-CONTA-03");
    expect(recognize("LAP_RRHH2")?.kind).toBe("computer");
  });

  it("personas por su usuario: seguro", () => {
    expect(recognize("maria.perez")).toEqual({ kind: "person", value: "maria.perez", sure: true });
    expect(recognize("EMPRESA\\maria.perez")).toEqual({ kind: "person", value: "maria.perez", sure: true });
  });

  it("extensiones, IP y tickets", () => {
    expect(recognize("4512")).toEqual({ kind: "extension", value: "4512", sure: true });
    expect(recognize("192.168.10.45")?.kind).toBe("ip");
    expect(recognize("#4521")?.kind).toBe("ticket");
  });

  it("impresoras", () => {
    expect(recognize("impresora recepción")?.kind).toBe("printer");
    expect(recognize("HP LaserJet")?.kind).toBe("printer");
    expect(recognize("ricoh recepción")?.kind).toBe("printer");
  });

  it("hablar de escanear con una marca no es «una impresora»: lleva a Escáneres", () => {
    expect(recognize("configurar escaner ricoh")).toBeNull();
    expect(recognize("escaner epson")).toBeNull();
    expect(recognize("configurar escáner RICOH")).toBeNull();
  });

  /**
   * Dos palabras pueden ser un nombre o una búsqueda normal: se reconoce como
   * persona, pero «dudoso», para ofrecerlo al final y no tapar «Windows Update».
   */
  it("dos palabras sueltas son dudosas", () => {
    expect(recognize("María Pérez")).toEqual({ kind: "person", value: "María Pérez", sure: false });
    expect(recognize("windows update")?.sure).toBe(false);
  });

  it("un problema escrito con palabras no es el nombre de una persona", () => {
    for (const frase of ["monitor borroso", "pantalla estirada", "no detecta el segundo monitor", "la ñ no sale", "arroba en otro sitio", "excel no se guardó", "word se cerró sin guardar", "internet va lento", "quien usa internet", "onedrive se come la subida", "escanear a carpeta", "configurar escaner ricoh"]) {
      expect(recognize(frase), frase).toBeNull();
    }
    expect(recognize("María Pérez")?.kind).toBe("person");
  });

  it("no adivina de más", () => {
    expect(recognize("red")).toBeNull();
    expect(recognize("espacio")).toBeNull();
    expect(recognize("999.1.1.1")).toBeNull();
    expect(recognize("123456789")).toBeNull();
    expect(recognize("x")).toBeNull();
  });
});

describe("frases que hacen algo en Ctrl+K", () => {
  it("las cuatro del ejemplo", () => {
    expect(parseAction("desbloquear maria")).toEqual({ action: "unlock", arg: "maria" });
    expect(parseAction("reiniciar PC-CONTA-03")).toEqual({ action: "restart", arg: "PC-CONTA-03" });
    expect(parseAction("cola recepción")).toEqual({ action: "queue", arg: "recepción" });
    expect(parseAction("laps legal-04")).toEqual({ action: "laps", arg: "legal-04" });
    expect(parseAction("liberar espacio")).toEqual({ action: "space", arg: "" });
  });

  it("perdona erratas y palabras de enlace", () => {
    expect(parseAction("desblokear maria perez")?.action).toBe("unlock");
    expect(parseAction("desbloquear a maria perez")?.arg).toBe("maria perez");
    expect(parseAction("reinicar pc-conta-03")?.action).toBe("restart");
    expect(parseAction("vaciar la cola de recepción")).toEqual({ action: "queue", arg: "recepción" });
    expect(parseAction("libera espacio")?.action).toBe("space");
  });

  it("no inventa acciones", () => {
    expect(parseAction("cola")).toBeNull();
    expect(parseAction("desbloquear")).toBeNull();
    expect(parseAction("reiniciar el router de la oficina")).toBeNull();
    expect(parseAction("vaciar papelera")).toBeNull();
    expect(parseAction("liberar la cuenta de maria")).toBeNull();
    expect(parseAction("discos")).toBeNull();
    expect(parseAction("windows update")).toBeNull();
    expect(parseAction("")).toBeNull();
  });

  it("explorador y colas sin nombre", () => {
    expect(parseAction("reiniciar explorador")).toEqual({ action: "explorer", arg: "" });
    expect(parseAction("reiniciar el explorador")).toEqual({ action: "explorer", arg: "" });
    expect(parseAction("vaciar cola")).toEqual({ action: "queue", arg: "" });
    expect(parseAction("vaciar la cola de impresión")).toEqual({ action: "queue", arg: "" });
    expect(parseAction("vaciar colas")).toEqual({ action: "queue", arg: "" });
    // Con nombre sigue siendo esa impresora; y «cola» a secas, una búsqueda.
    expect(parseAction("vaciar cola recepción")).toEqual({ action: "queue", arg: "recepción" });
    expect(parseAction("cola")).toBeNull();
  });
});

describe("varias acciones en una frase", () => {
  it("parte por «y», comas y punto y coma", () => {
    const s = parseSteps("liberar espacio y vaciar cola y reiniciar explorador");
    expect(s?.map((x) => x.intent.action)).toEqual(["space", "queue", "explorer"]);
    expect(parseSteps("liberar espacio, vaciar cola; reiniciar el explorador")).toHaveLength(3);
  });

  it("no parte nombres ni mezclas de acción y búsqueda", () => {
    expect(splitPhrase("maria y juan")).toEqual(["maria", "juan"]);
    expect(parseSteps("maria y juan")).toBeNull();
    expect(parseSteps("desbloquear maria y juan")).toBeNull();
    expect(parseSteps("liberar espacio y windows update")).toBeNull();
    expect(parseSteps("liberar espacio")).toBeNull();
    expect(parseSteps("yolanda y yago")).toBeNull();
  });

  it("Tab completa la orden a medias, la de después del último «y»", () => {
    expect(completePhrase("liberar espacio y vaciar co")).toBe("liberar espacio y vaciar cola de impresión");
    expect(completePhrase("reiniciar e")).toBe("reiniciar explorador");
    expect(completePhrase("lib")).toBe("liberar espacio");
    expect(completePhrase("liberar espacio y reiniciar ex")).toBe("liberar espacio y reiniciar explorador");
    expect(completePhrase("liberar espacio")).toBeNull();
    expect(completePhrase("reiniciar PC-CONTA-03")).toBeNull();
    expect(completePhrase("")).toBeNull();
    expect(completePhrase("l")).toBeNull();
  });
});
