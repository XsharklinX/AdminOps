import { describe, expect, it } from "vitest";
import type { StartupItem } from "./api";
import { forHost, gains, measured, secsText } from "./startupGain";

const item = (name: string, command: string, enabled = true): StartupItem => ({ id: name, name, command, location: "", source: "registry", enabled, needsAdmin: false, publisher: null, description: null, target: command.split(" ")[0] });

describe("lo que se gana quitando programas de inicio", () => {
  const items = [item("Teams", '"C:\\Users\\x\\AppData\\Local\\Microsoft\\Teams\\Update.exe" --processStart Teams.exe'), item("Spotify", "C:\\Spotify\\Spotify.exe /minimized"), item("Off", "C:\\x\\off.exe", false)];
  const culprits = [
    { kind: "app" as const, name: "Spotify.exe", times: 4, avgDelayMs: 6000 },
    { kind: "app" as const, name: "Update.exe", times: 6, avgDelayMs: 18000 },
    { kind: "driver" as const, name: "spotify.sys", times: 1, avgDelayMs: 99000 },
    { kind: "app" as const, name: "off.exe", times: 3, avgDelayMs: 5000 },
  ];

  it("cruza los retrasos de Windows con los programas que arrancan, de más a menos", () => {
    const g = gains(items, culprits);
    expect(g.map((x) => [x.item.name, x.delayMs])).toEqual([
      ["Teams", 18000],
      ["Spotify", 6000],
    ]);
  });

  it("mide el arranque siguiente frente a la media de antes", () => {
    const boots = [
      { time: "2026-10-16T08:00:00Z", totalMs: 89000, mainMs: 0, postMs: 0 },
      { time: "2026-10-10T08:00:00Z", totalMs: 130000, mainMs: 0, postMs: 0 },
    ];
    const at = Date.parse("2026-10-12T00:00:00Z") / 1000;
    expect(measured(boots, { at, avgMs: 130000, estimateMs: 92000, names: ["Teams"] })).toEqual({ beforeMs: 130000, afterMs: 89000, estimateMs: 92000 });
    // Sin arranques después, aún no se puede medir.
    expect(measured(boots, { at: Date.parse("2026-10-17T00:00:00Z") / 1000, avgMs: 1, estimateMs: 1, names: [] })).toBeNull();
    expect(secsText(130000)).toBe("2 min 10 s");
    expect(secsText(41000)).toBe("41 s");
  });

  it("el apunte de otro equipo (viajó en el USB) no vale", () => {
    const b = { at: 1, avgMs: 1, estimateMs: 1, names: ["Teams"], host: "PC-A" };
    expect(forHost(b, "PC-A")).toBe(b);
    expect(forHost(b, "pc-a")).toBe(b);
    expect(forHost(b, "PC-B")).toBeNull();
    // Sin nombre de equipo no se puede decidir: se acepta.
    expect(forHost(b, "")).toBe(b);
    // Los apuntes viejos no llevan equipo.
    const viejo = { at: 1, avgMs: 1, estimateMs: 1, names: [] };
    expect(forHost(viejo, "PC-B")).toBe(viejo);
    expect(forHost(null, "PC-B")).toBeNull();
  });
});
