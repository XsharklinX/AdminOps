import { describe, expect, it } from "vitest";
import { fillTemplate, recipients, reportNumber } from "./reportText";

describe("textos del informe", () => {
  it("rellena los campos conocidos y deja el resto", () => {
    const t = "Hola {contacto}, informe Nº {numero} de {cliente} ({fecha}). {otro}";
    expect(fillTemplate(t, { contacto: "Ana", numero: "2026-0012", cliente: "Farmacia Sol", fecha: "3 de octubre" })).toBe(
      "Hola Ana, informe Nº 2026-0012 de Farmacia Sol (3 de octubre). {otro}",
    );
  });

  it("sin valor, el campo desaparece sin dejar espacios raros", () => {
    expect(fillTemplate("Hola {contacto}, te envío el informe.", {})).toBe("Hola, te envío el informe.");
  });

  it("saca el número del nombre del archivo", () => {
    expect(reportNumber("C:/x/Informe_2026-0012_Farmacia.pdf")).toBe("2026-0012");
    expect(reportNumber("otro.pdf")).toBe("");
  });

  it("une destinatarios sin repetir", () => {
    expect(recipients("ana@sol.com", "Jefe@Sol.com, ana@SOL.com; x")).toBe("ana@sol.com; Jefe@Sol.com");
    expect(recipients(undefined, "")).toBe("");
  });
});
