import { describe, expect, it } from "vitest";
import type { Station } from "./api";
import { failed, severityOf, sortBySeverity } from "./stationSeverity";

const remote = (o: Partial<NonNullable<Station["remote"]>> = {}): NonNullable<Station["remote"]> => ({
  os: "Windows 11",
  user: "",
  bootDays: 2,
  freeGb: 100,
  totalGb: 240,
  updateDays: 5,
  via: "WinRM",
  diskHealth: "ok",
  antivirus: "",
  avOk: true,
  bsods: 0,
  pendingReboot: false,
  ...o,
});
const st = (host: string, o: Partial<Station> = {}): Station => ({ host, ip: "", online: true, ms: 1, ports: [], remote: null, remoteError: "", warnings: [], ...o });

describe("gravedad de los puestos", () => {
  it("clasifica cada caso", () => {
    expect(severityOf(st("a", { online: false })).severity).toBe("offline");
    expect(severityOf(st("a", { remote: remote({ diskHealth: "bad" }) })).severity).toBe("disk");
    expect(severityOf(st("a", { remote: remote({ avOk: false }) })).severity).toBe("security");
    expect(severityOf(st("a", { remote: remote({ pendingReboot: true }) })).severity).toBe("reboot");
    expect(severityOf(st("a", { remote: remote({ bootDays: 30 }) })).severity).toBe("reboot");
    expect(severityOf(st("a", { remote: remote({ freeGb: 5 }) })).severity).toBe("space");
    expect(severityOf(st("a", { remote: remote() })).severity).toBe("ok");
    expect(severityOf(st("a")).severity).toBe("other");
  });

  it("ordena: no responde, disco, seguridad, reinicio, espacio, otros avisos, bien", () => {
    const list = [
      st("OK-1", { remote: remote() }),
      st("SIN", {}),
      st("ESP", { remote: remote({ freeGb: 3 }) }),
      st("REI", { remote: remote({ pendingReboot: true }) }),
      st("SEG", { remote: remote({ avOk: false }) }),
      st("DIS", { remote: remote({ diskHealth: "warn" }) }),
      st("OFF", { online: false }),
    ];
    expect(sortBySeverity(list).map((s) => s.host)).toEqual(["OFF", "DIS", "SEG", "REI", "ESP", "SIN", "OK-1"]);
  });

  it("a igual gravedad, por nombre y sin tocar la lista original", () => {
    const list = [st("PC-10", { online: false }), st("PC-2", { online: false })];
    expect(sortBySeverity(list).map((s) => s.host)).toEqual(["PC-2", "PC-10"]);
    expect(list[0].host).toBe("PC-10");
  });

  it("«los que fallaron» son todos menos los que están bien", () => {
    const list = [st("a", { online: false }), st("b", { remote: remote() }), st("c", { warnings: ["algo"] }), st("d", {})];
    expect(failed(list).map((s) => s.host)).toEqual(["a", "c", "d"]);
  });
});
