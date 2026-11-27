import { describe, expect, it } from "vitest";
import { lidSentence, sideSentence } from "./power";

const st = (acLid: number, dcLid: number, battery = true) => ({ ac: { screenMin: 2, sleepMin: 10, lid: acLid }, dc: { screenMin: 5, sleepMin: 15, lid: dcLid }, lidPresent: true, battery, profile: null, admin: true });

describe("energía en una frase", () => {
  it("minutos, horas y nunca", () => {
    expect(sideSentence({ screenMin: 2, sleepMin: 10, lid: 1 })).toBe("la pantalla se apaga a los 2 min y se suspende a los 10 min");
    expect(sideSentence({ screenMin: 0, sleepMin: 0, lid: 0 })).toBe("la pantalla no se apaga nunca y no se suspende nunca");
    expect(sideSentence({ screenMin: 60, sleepMin: 180, lid: 0 })).toBe("la pantalla se apaga a la hora y se suspende a las 3 horas");
  });

  it("la tapa", () => {
    expect(lidSentence(st(1, 1))).toBe("suspende, también en la base");
    expect(lidSentence(st(0, 1))).toBe("enchufado no hace nada; con batería suspende");
    expect(lidSentence(st(3, 3))).toBe("se apaga");
  });
});
