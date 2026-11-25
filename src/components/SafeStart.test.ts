import { describe, expect, it } from "vitest";
import { explainDoing } from "./SafeStart";

describe("abrir a salvo: qué estaba haciendo", () => {
  it("dice lo mismo que el aviso de Rust", () => {
    expect(explainDoing(["vigilando la pantalla (Grabar el fallo)", "vigilando la pantalla (Grabar el fallo)"])).toBe("Las dos veces estaba vigilando la pantalla (Grabar el fallo).");
    expect(explainDoing(["grabando la pantalla", "en «Discos»"])).toBe("La primera vez estaba grabando la pantalla y la segunda, en «Discos».");
    expect(explainDoing(["", "en «Discos»"])).toBe("La última vez estaba en «Discos».");
    expect(explainDoing(["", ""])).toBe("");
    expect(explainDoing([])).toBe("");
  });
});
