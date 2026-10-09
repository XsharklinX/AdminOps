import { describe, expect, it } from "vitest";
import { summarize, summaryText, type UserInput } from "./userHome";

const base: UserInput = { ramPct: 45, cpuPct: 20, securityScore: 92, securityIssues: [], freePct: 55, disks: [], startupCount: 8, analyzed: true };

describe("pantalla del modo usuario", () => {
  it("un equipo sano dice que está bien y no ofrece arreglar nada", () => {
    const s = summarize(base);
    expect(s.level).toBe("ok");
    expect(s.headline).toBe("Tu equipo está bien");
    expect(s.cards.map((c) => c.label)).toEqual(["Rápido", "Protegido", "De sobra"]);
    expect(s.fixable).toBe(false);
  });

  it("poco espacio se nota en la tarjeta y se puede arreglar", () => {
    const s = summarize({ ...base, freePct: 6 });
    expect(s.level).toBe("bad");
    expect(s.cards[2]).toMatchObject({ id: "space", level: "bad", label: "Casi lleno" });
    expect(s.cards[2].why[0]).toContain("muy poco espacio");
    expect(s.fixable).toBe(true);
    expect(s.sub).toBe("Hay una cosa que mejorar.");
  });

  it("memoria llena y muchos programas al inicio hacen el equipo lento", () => {
    const s = summarize({ ...base, ramPct: 93, startupCount: 22 });
    expect(s.cards[0].level).toBe("bad");
    expect(s.cards[0].why.join(" ")).toContain("22 programas");
  });

  it("un disco que falla pasa a la tarjeta de espacio, en lenguaje normal", () => {
    const s = summarize({ ...base, disks: [["Samsung SSD", "bad"]] });
    expect(s.cards[2].level).toBe("bad");
    expect(s.cards[2].why.join(" ")).toContain("hacer una copia");
  });

  it("sin análisis no se afirma que esté protegido", () => {
    const s = summarize({ ...base, securityScore: null, analyzed: false });
    expect(s.cards[1].label).toBe("Sin revisar");
    expect(s.sub).toContain("Revisar");
  });

  it("el texto para el técnico lleva todas las tarjetas", () => {
    const t = summaryText(summarize({ ...base, freePct: 12 }), "PC-CONTA-03");
    expect(t).toContain("PC-CONTA-03");
    expect(t.match(/• /g)).toHaveLength(3);
    expect(t).toContain("¿Puedes echarle un vistazo?");
  });
});
