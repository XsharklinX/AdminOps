import { describe, expect, it } from "vitest";
import type { ProcessView } from "./api";
import { groupProcesses, isSystemProcess, killable } from "./processGroups";

const proc = (over: Partial<ProcessView>): ProcessView => ({
  pid: 1,
  parent: null,
  name: "app.exe",
  exe: null,
  user: "ana",
  cpu: 0,
  memory: 0,
  diskReadPerSec: 0,
  diskWritePerSec: 0,
  started: 0,
  protection: "none",
  note: null,
  ...over,
});

describe("groupProcesses", () => {
  it("suma lo de todos los procesos del mismo programa", () => {
    const groups = groupProcesses([
      proc({ pid: 1, name: "chrome.exe", cpu: 2, memory: 100, diskReadPerSec: 5 }),
      proc({ pid: 2, name: "Chrome.exe", cpu: 3, memory: 300, diskWritePerSec: 7 }),
      proc({ pid: 3, name: "notepad.exe", cpu: 1, memory: 50 }),
    ]);
    expect(groups).toHaveLength(2);
    const chrome = groups.find((g) => g.key === "chrome.exe")!;
    expect(chrome).toMatchObject({ cpu: 5, memory: 400, disk: 12, user: "ana" });
    // Dentro, el que más memoria usa va primero.
    expect(chrome.procs.map((p) => p.pid)).toEqual([2, 1]);
  });

  it("la protección del grupo es la más fuerte, y esos no se finalizan", () => {
    const [g] = groupProcesses([proc({ pid: 1, name: "svchost.exe", protection: "sensitive" }), proc({ pid: 2, name: "svchost.exe", protection: "critical", user: "SYSTEM" })]);
    expect(g.protection).toBe("critical");
    expect(g.user).toBe("varios");
    expect(killable(g).map((p) => p.pid)).toEqual([1]);
  });

  it("distingue lo de Windows de lo de las personas", () => {
    expect(isSystemProcess({ user: "NT AUTHORITY\\SYSTEM" })).toBe(true);
    expect(isSystemProcess({ user: "LOCAL SERVICE" })).toBe(true);
    expect(isSystemProcess({ user: "DWM-1" })).toBe(true);
    expect(isSystemProcess({ user: null })).toBe(true);
    expect(isSystemProcess({ user: "EQUIPO\\ana" })).toBe(false);
  });
});
