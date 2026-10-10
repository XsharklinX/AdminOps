import { describe, expect, it } from "vitest";
import { splitCodes } from "./CodeLinks";
import { looksLikeCode } from "./CommandPalette";

describe("códigos de error enlazados", () => {
  it("separa los códigos del texto", () => {
    expect(splitCodes("KB5044284 falla con 0x800F081F (2 intentos)")).toEqual([
      { text: "KB5044284 falla con ", code: false },
      { text: "0x800F081F", code: true },
      { text: " (2 intentos)", code: false },
    ]);
    expect(splitCodes("sin códigos")).toEqual([{ text: "sin códigos", code: false }]);
  });
  it("reconoce un código pegado en Ctrl+K", () => {
    expect(looksLikeCode("0x80070005")).toBe(true);
    expect(looksLikeCode("80070005")).toBe(true);
    expect(looksLikeCode("-2147024891")).toBe(true);
    expect(looksLikeCode("stop 0x133")).toBe(true);
    expect(looksLikeCode("impresora")).toBe(false);
  });
});
