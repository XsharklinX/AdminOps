import { describe, expect, it } from "vitest";
import type { LocalUser } from "./api";
import { userIssues, worstIssue } from "./userIssues";

const NOW = new Date("2026-10-01T12:00:00Z").getTime();
const iso = (days: number) => new Date(NOW + days * 86_400_000).toISOString();

const user = (over: Partial<LocalUser> = {}): LocalUser => ({
  name: "ana",
  fullName: "",
  description: "",
  sid: "S-1-5-21-1",
  enabled: true,
  admin: false,
  microsoft: false,
  lastLogon: iso(-1),
  passwordLastSet: iso(-30),
  passwordExpires: null,
  hasProfile: true,
  signedIn: false,
  builtin: null,
  isTarget: false,
  isSelf: false,
  ...over,
});

describe("userIssues", () => {
  it("una cuenta normal no tiene nada que revisar", () => {
    expect(userIssues(user(), NOW)).toEqual([]);
    expect(worstIssue([])).toBeNull();
  });

  it("contraseña caducada o a punto de caducar", () => {
    const expired = userIssues(user({ passwordExpires: iso(-2) }), NOW);
    expect(expired).toHaveLength(1);
    expect(expired[0]).toMatchObject({ level: "bad", fix: "password" });
    const soon = userIssues(user({ passwordExpires: iso(3) }), NOW);
    expect(soon[0].level).toBe("warn");
    expect(soon[0].text).toContain("3 días");
    expect(userIssues(user({ passwordExpires: iso(40) }), NOW)).toEqual([]);
  });

  it("sin contraseña: grave si es administrador", () => {
    expect(worstIssue(userIssues(user({ passwordLastSet: null }), NOW))).toBe("warn");
    expect(worstIssue(userIssues(user({ passwordLastSet: null, admin: true }), NOW))).toBe("bad");
    // Las cuentas de Microsoft no guardan esa fecha: no se marcan.
    expect(userIssues(user({ passwordLastSet: null, microsoft: true }), NOW)).toEqual([]);
  });

  it("cuentas integradas activas y cuentas sin usar", () => {
    expect(userIssues(user({ builtin: "administrator", admin: true }), NOW)[0].fix).toBe("disable");
    expect(userIssues(user({ builtin: "guest" }), NOW)[0].fix).toBe("disable");
    expect(userIssues(user({ builtin: "administrator", enabled: false }), NOW)).toEqual([]);
    expect(userIssues(user({ builtin: "wdag" }), NOW)).toEqual([]);
    const stale = userIssues(user({ lastLogon: iso(-400) }), NOW);
    expect(stale[0].text).toContain("13 meses");
    // Con la sesión abierta, o si nunca entró, no se dice que sobra.
    expect(userIssues(user({ lastLogon: iso(-400), signedIn: true }), NOW)).toEqual([]);
    expect(userIssues(user({ lastLogon: null }), NOW)).toEqual([]);
    // Desactivada: ya no hay nada que avisar.
    expect(userIssues(user({ enabled: false, passwordExpires: iso(-2) }), NOW)).toEqual([]);
  });
});
