import { describe, expect, it } from "vitest";
import type { JournalEntry } from "./api";
import { programKey } from "./programs";
import { fill, questions } from "./templates";
import { autoDone } from "./visits";

describe("plantillas", () => {
  it("encuentra las preguntas propias sin repetirlas", () => {
    expect(questions("Hola {?Nombre}, {?Nombre} tu equipo {equipo} está listo. {?Día de recogida}")).toEqual(["Nombre", "Día de recogida"]);
  });

  it("rellena variables y respuestas, y deja intactas las desconocidas", () => {
    const text = fill("Hola {?Nombre}: {equipo} listo el {fecha}. {Desconocida}", { equipo: "PC-01", fecha: "hoy" }, { Nombre: "Ana" });
    expect(text).toBe("Hola Ana: PC-01 listo el hoy. {Desconocida}");
    expect(fill("{?Sin respuesta}.", {}, {})).toBe(".");
  });
});

const entry = (patch: Partial<JournalEntry>): JournalEntry =>
  ({ id: 1, timestamp: 200, op: "run", tweakId: null, title: "", ok: true, message: null, backups: [], reverted: false, undoable: false, ...patch }) as JournalEntry;

describe("checklist que se marca sola", () => {
  const started = 100;
  it("reconoce cada tarea por el diario", () => {
    expect(autoDone("cleanup", [entry({ tweakId: "cleanup.user-temp" })], [], started, 0)).toBe(true);
    expect(autoDone("updates", [entry({ title: "Actualizar 7-Zip (25 → 26)" })], [], started, 0)).toBe(true);
    expect(autoDone("startup", [entry({ title: "Inicio: desactivar Spotify" })], [], started, 0)).toBe(true);
    expect(autoDone("startup", [entry({ title: "Inicio: activar Spotify" })], [], started, 0)).toBe(false);
    expect(autoDone("privacy", [entry({ tweakId: "privacy.telemetry", op: "apply" })], [], started, 0)).toBe(true);
    expect(autoDone("restorepoint", [entry({ op: "restorePoint" })], [], started, 0)).toBe(true);
  });

  it("no cuenta lo fallido, lo anterior a la sesión ni el diagnóstico inicial", () => {
    expect(autoDone("cleanup", [entry({ tweakId: "cleanup.x", ok: false })], [], started, 0)).toBe(false);
    expect(autoDone("cleanup", [entry({ tweakId: "cleanup.x", timestamp: 50 })], [], started, 0)).toBe(false);
    expect(autoDone("diagnostic", [], [150], started, 150)).toBe(false);
    expect(autoDone("diagnostic", [], [150, 300], started, 150)).toBe(true);
    expect(autoDone("", [entry({})], [], started, 0)).toBe(false);
  });
});

describe("emparejar programas con winget", () => {
  it("quita versión, arquitectura e idioma", () => {
    expect(programKey("7-Zip 25.01 (x64)")).toBe("7-zip");
    expect(programKey("Mozilla Firefox (x64 es-ES)")).toBe("mozilla firefox");
    expect(programKey("Notepad++ v8.6.2")).toBe("notepad++");
    expect(programKey("VLC media player")).toBe("vlc media player");
    expect(programKey("Programa Ñandú 64-bit")).toBe("programa nandu");
  });
});
