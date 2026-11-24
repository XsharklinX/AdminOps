import { describe, expect, it } from "vitest";
import type { PlanState, PlanWait, RecipeStep } from "./api";
import { checkedMessage, fit, passwordsNeeded, pauseNeeded, requestResume, resumeBanner, shouldResume, takeResumeRequest } from "./planResume";

const wait = (kind: PlanWait["kind"]): PlanWait => ({ kind, expect: "x", name: "" });

describe("planResume", () => {
  it("la petición de retomar se consume una sola vez", () => {
    expect(takeResumeRequest()).toBe(false);
    requestResume();
    expect(takeResumeRequest()).toBe(true);
    expect(takeResumeRequest()).toBe(false);
  });

  it("rellena lo que falta si la plantilla cambió de tamaño", () => {
    expect(fit(["ok", "error"], 3, "pending")).toEqual(["ok", "error", "pending"]);
    expect(fit(["ok", "ok", "ok"], 2, "pending")).toEqual(["ok", "ok"]);
    expect(fit(undefined, 2, "")).toEqual(["", ""]);
  });

  it("reiniciar siempre para; renombrar o unir al dominio solo si quedan pasos", () => {
    expect(pauseNeeded(null, 0, 3)).toBe(false);
    expect(pauseNeeded(wait("rename"), 0, 3)).toBe(true);
    expect(pauseNeeded(wait("domain"), 2, 3)).toBe(false);
    expect(pauseNeeded(wait("reboot"), 2, 3)).toBe(true);
  });

  it("las contraseñas se piden solo para los pasos que quedan", () => {
    const steps: RecipeStep[] = [
      { kind: "user", name: "ana", fullName: "", admin: false },
      { kind: "rename", newName: "PC-01" },
      { kind: "domain", domain: "empresa.local", ou: "" },
    ];
    expect(passwordsNeeded(steps, 0)).toEqual({ user: true, domain: true });
    expect(passwordsNeeded(steps, 2)).toEqual({ user: false, domain: true });
    expect(passwordsNeeded(steps, 3)).toEqual({ user: false, domain: false });
  });

  it("el aviso de arriba cuenta el paso desde 1", () => {
    expect(resumeBanner(2, 5)).toBe("Seguido tras reiniciar · paso 3 de 5");
    expect(checkedMessage("El equipo se reinició.")).toBe("Comprobado tras reiniciar: El equipo se reinició.");
  });

  it("el estado guardado va y vuelve como JSON", () => {
    const s: PlanState = {
      recipe: { id: "k1", name: "Equipo nuevo", steps: [{ kind: "reboot" }] },
      statuses: ["ok", "pending"],
      messages: ["", ""],
      stopOnError: true,
      waitingStep: 0,
      wait: wait("reboot"),
      hostBefore: "PC-A",
      bootTime: 1,
      savedAt: 2,
    };
    expect(JSON.parse(JSON.stringify(s))).toEqual(s);
  });

  it("solo se retoma con --seguir-plan o si el equipo se reinició", () => {
    expect(shouldResume({ fromArgs: true, rebooted: false })).toBe(true);
    expect(shouldResume({ fromArgs: false, rebooted: true })).toBe(true);
    // Un plan guardado sin reinicio no se adueña del arranque.
    expect(shouldResume({ fromArgs: false, rebooted: false })).toBe(false);
  });
});
