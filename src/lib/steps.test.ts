import { describe, expect, it } from "vitest";
import { checkFor, parseSteps } from "./steps";

describe("soluciones paso a paso", () => {
  it("parte el texto en pasos", () => {
    const s = parseSteps("Antes de nada\n1. Reinicia\n   el router\n2) Vacía la caché\n\n3. Prueba");
    expect(s.map((x) => x.n)).toEqual([0, 1, 2, 3]);
    expect(s[1].text).toBe("Reinicia\n   el router");
    expect(s[2].text).toBe("Vacía la caché");
  });
  it("elige la comprobación por el tema", () => {
    expect(checkFor({ title: "La impresora no imprime", tags: [] })).toBe("printer");
    expect(checkFor({ title: "Conectado a la Wi-Fi pero sin Internet", tags: [] })).toBe("wifi");
    expect(checkFor({ title: "Outlook pide la contraseña", tags: ["correo"] })).toBe("outlook");
    expect(checkFor({ title: "Algo raro", tags: [] })).toBeNull();
  });
});
