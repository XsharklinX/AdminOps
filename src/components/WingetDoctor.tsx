import { listen } from "@tauri-apps/api/event";
import { CheckCircle2, CircleSlash, Loader2, Wrench, XCircle } from "lucide-react";
import { useEffect, useState } from "react";
import { installQueueApi, type InstallItem } from "../lib/api";
import { wingetApi, WINGET_STEP_EVENT, type WingetHealth, type WingetStep } from "../lib/api/wingetfix";
import { Button } from "./ui";

/** Fallos que apuntan a winget y no al programa: no está, origen roto, no responde. */
const WINGET_CODES = new Set(["0x8A15000F", "0x8A150044", "0x8A15003B", "0x8A150046", "0x8A150014", "0x8A150017"]);
export function wingetSuspect(items: InstallItem[]): boolean {
  const failed = items.filter((i) => i.state === "failed");
  if (!failed.length) return false;
  if (failed.some((i) => i.message.startsWith("winget no está disponible"))) return true;
  // Varios fallos con códigos de origen o de «no encuentra»: es winget, no los programas.
  return failed.filter((i) => i.code && WINGET_CODES.has(i.code.toUpperCase().replace("0X", "0x"))).length >= Math.min(2, failed.length);
}

/** Ya se vio en esta sesión que winget funciona: al volver a Instalar no se mira otra vez. */
let healthyThisSession = false;

const STEP_ICON = { run: Loader2, ok: CheckCircle2, fail: XCircle, skip: CircleSlash } as const;

/**
 * «winget falla en este equipo»: por qué (no está, versión vieja, origen roto, sin red), lo repara
 * paso a paso y vuelve a intentar lo que había fallado.
 */
export function WingetDoctor({ items, isAdmin, quiet = false }: { items: InstallItem[]; isAdmin: boolean; /** Comprobación previa al entrar en Instalar: sin «Mirando…» y nada si todo va bien. */ quiet?: boolean }) {
  const [health, setHealth] = useState<WingetHealth | null>(null);
  const [steps, setSteps] = useState<WingetStep[]>([]);
  const [state, setState] = useState<"checking" | "idle" | "repairing" | "retrying" | "done">("checking");
  const [error, setError] = useState("");

  useEffect(() => {
    if (quiet && healthyThisSession) {
      setHealth({ state: "ok", version: null, detail: "" });
      setState("idle");
      return;
    }
    wingetApi
      .check()
      .then((h) => {
        if (h.state === "ok") healthyThisSession = true;
        setHealth(h);
        setState("idle");
      })
      .catch((e) => {
        setError(String(e));
        setState("idle");
      });
    const off = listen<WingetStep>(WINGET_STEP_EVENT, (e) => setSteps((s) => [...s.filter((x) => x.id !== e.payload.id), e.payload]));
    return () => {
      void off.then((f) => f());
    };
  }, [quiet]);

  const failed = items.filter((i) => i.state === "failed");
  const repair = async () => {
    setSteps([]);
    setError("");
    setState("repairing");
    try {
      const h = await wingetApi.repair();
      setHealth(h);
      if (h.state === "ok" && failed.length) {
        setState("retrying");
        for (const i of failed) await installQueueApi.retry(i.id);
      }
      setState("done");
    } catch (e) {
      setError(String(e));
      setState("idle");
    }
  };

  if (state === "checking" && quiet) return null;
  if (state === "checking")
    return (
      <div className="mb-3 flex items-center gap-2 rounded-lg border border-line bg-panel-2/40 px-3 py-2 text-sm text-mute">
        <Loader2 size={14} className="animate-spin" /> Mirando si el problema es winget…
      </div>
    );
  if (health?.state === "ok" && state !== "done") return null;

  const fixed = state === "done" && health?.state === "ok";
  return (
    <div className={`mb-3 rounded-lg border p-3 text-sm ${fixed ? "border-ok/40 bg-ok/5" : "border-warn/40 bg-warn/5"}`}>
      <p className="font-medium text-ink">{fixed ? `winget reparado${failed.length ? `: vuelve a instalar ${failed.length === 1 ? "el programa que falló" : `los ${failed.length} programas que fallaron`}` : ": ya puedes instalar"}.` : quiet ? "winget no está listo en este equipo" : "winget falla en este equipo"}</p>
      {!fixed && health && <p className="mt-0.5 text-dim">{health.detail}</p>}
      {steps.length > 0 && (
        <ul className="mt-2 space-y-1">
          {["register", "update", "sources"].map((id) => {
            const s = steps.find((x) => x.id === id);
            if (!s) return null;
            const Icon = STEP_ICON[s.state];
            return (
              <li key={id} className={`flex items-start gap-2 text-[12.5px] ${s.state === "fail" ? "text-bad" : s.state === "skip" ? "text-mute" : "text-dim"}`}>
                <Icon size={13} className={`mt-0.5 shrink-0 ${s.state === "run" ? "animate-spin text-neon" : s.state === "ok" ? "text-ok" : ""}`} />
                {s.text}
              </li>
            );
          })}
          {state === "retrying" && (
            <li className="flex items-center gap-2 text-[12.5px] text-dim">
              <Loader2 size={13} className="animate-spin text-neon" /> Volver a instalar lo que falló
            </li>
          )}
        </ul>
      )}
      {error && <p className="mt-2 text-bad">{error}</p>}
      {!fixed && health?.state !== "network" && (
        <div className="mt-2 flex items-center gap-2">
          <Button size="sm" disabled={!isAdmin || state === "repairing" || state === "retrying"} onClick={repair}>
            <Wrench size={13} /> {state === "repairing" ? "Reparando winget…" : failed.length ? "Reparar winget y volver a intentar" : "Reparar winget"}
          </Button>
          {!isAdmin && <span className="text-xs text-warn">Hace falta abrir AdminOps como administrador.</span>}
        </div>
      )}
    </div>
  );
}
