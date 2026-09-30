import { BatteryWarning, Loader2, Power, RefreshCw, RotateCcw, Trash2, Wifi, WifiOff } from "lucide-react";
import { useCallback, useEffect, useState } from "react";
import { useToast } from "./feedback";
import { Button, Card } from "./ui";
import { wifiApi, type WifiState } from "../lib/api";

const TONE: Record<string, string> = {
  ok: "border-ok/40 bg-ok/5 text-ok",
  off: "border-warn/40 bg-warn/5 text-warn",
  error: "border-bad/40 bg-bad/5 text-bad",
  none: "border-line bg-panel-2 text-dim",
};

/** Estado real de la Wi-Fi y cómo arreglarla: encender/apagar, reiniciar la tarjeta, limpiar fantasmas. */
export function WifiControl({ isAdmin }: { isAdmin: boolean }) {
  const [state, setState] = useState<WifiState | null>(null);
  const [error, setError] = useState<string | null>(null);
  const [busy, setBusy] = useState<string | null>(null);
  const toast = useToast();

  const load = useCallback(async () => {
    setBusy((b) => b ?? "load");
    try {
      setState(await wifiApi.state());
      setError(null);
    } catch (e) {
      setError(String(e));
    } finally {
      setBusy((b) => (b === "load" ? null : b));
    }
  }, []);

  useEffect(() => {
    void load();
  }, [load]);

  const run = async (key: string, fn: () => Promise<unknown>, ok: string) => {
    setBusy(key);
    try {
      await fn();
      toast("ok", ok);
    } catch (e) {
      toast("error", String(e));
    } finally {
      setBusy(null);
      void load();
    }
  };

  const present = state?.adapters.filter((a) => a.present) ?? [];
  const ghosts = state?.adapters.filter((a) => !a.present) ?? [];
  const broken = present.find((a) => a.problem !== 0);
  const needAdmin = isAdmin ? undefined : "Requiere ejecutar AdminOps como administrador";

  return (
    <Card
      title="Estado de la Wi-Fi"
      icon={<Wifi size={14} />}
      className="col-span-12"
      right={
        <button onClick={load} disabled={!!busy} className="flex items-center gap-1 text-[11px] text-mute hover:text-ink">
          <RefreshCw size={11} className={busy === "load" ? "animate-spin" : ""} /> Volver a leer
        </button>
      }
    >
      {error ? (
        <p className="text-sm text-mute">{error}</p>
      ) : !state ? (
        <p className="flex items-center gap-2 text-sm text-mute">
          <Loader2 size={13} className="animate-spin" /> Comprobando la tarjeta Wi-Fi…
        </p>
      ) : (
        <div className="space-y-3">
          <div className={`flex items-start gap-2 rounded-lg border px-3 py-2 text-sm ${TONE[state.level] ?? TONE.none}`}>
            {state.level === "ok" ? <Wifi size={15} className="mt-0.5 shrink-0" /> : <WifiOff size={15} className="mt-0.5 shrink-0" />}
            <span>{state.summary}</span>
          </div>

          {present.length > 0 && (
            <ul className="space-y-1 text-sm">
              {present.map((a) => (
                <li key={a.instanceId} className="flex flex-wrap items-center gap-x-3 gap-y-0.5">
                  <span className="text-ink">{a.name}</span>
                  <span className="text-xs text-mute">{a.description}</span>
                  {a.status && <span className="text-xs text-dim">· {a.status}</span>}
                  {a.problem !== 0 && <span className="text-xs text-bad">· código {a.problem}</span>}
                  {a.powerSaving && <span className="text-xs text-warn">· Windows puede apagarla para ahorrar energía</span>}
                </li>
              ))}
            </ul>
          )}

          <div className="flex flex-wrap gap-2">
            {state.radioOn !== null && (
              <Button
                kind={state.radioOn ? "ghost" : "primary"}
                disabled={!!busy}
                onClick={() => run("radio", () => wifiApi.setRadio(!state.radioOn), state.radioOn ? "Wi-Fi apagada." : "Wi-Fi encendida.")}
              >
                {busy === "radio" ? <Loader2 size={13} className="animate-spin" /> : <Power size={13} />}
                {state.radioOn ? "Apagar Wi-Fi" : "Encender Wi-Fi"}
              </Button>
            )}
            {present.map((a) => (
              <Button
                key={a.instanceId}
                kind={broken ? "primary" : "ghost"}
                disabled={!!busy || !isAdmin}
                title={needAdmin ?? "Deshabilita y vuelve a habilitar la tarjeta: reinicia su driver sin reiniciar el equipo"}
                onClick={() => run("restart", () => wifiApi.restartAdapter(a.instanceId), "Tarjeta Wi-Fi reiniciada.")}
              >
                {busy === "restart" ? <Loader2 size={13} className="animate-spin" /> : <RotateCcw size={13} />}
                {present.length > 1 ? `Reiniciar ${a.name}` : a.problem === 22 ? "Habilitar la tarjeta" : "Reiniciar la tarjeta"}
              </Button>
            ))}
            {present
              .filter((a) => a.powerSaving)
              .map((a) => (
                <Button
                  key={`pw-${a.instanceId}`}
                  kind="ghost"
                  disabled={!!busy || !isAdmin}
                  title={needAdmin ?? "Evita que Windows apague la tarjeta para ahorrar energía (causa típica de cortes y fallos intermitentes)"}
                  onClick={() => run("power", () => wifiApi.powerSavingOff(a.name), "Windows ya no apagará la tarjeta para ahorrar energía.")}
                >
                  <BatteryWarning size={13} /> No apagar para ahorrar energía
                </Button>
              ))}
            {ghosts.length > 0 && (
              <Button
                kind="ghost"
                disabled={!!busy || !isAdmin}
                title={needAdmin ?? "Adaptadores de instalaciones anteriores del driver que ya no existen"}
                onClick={() => run("ghosts", () => wifiApi.removeGhosts(), `Adaptadores fantasma quitados.`)}
              >
                <Trash2 size={13} /> Quitar {ghosts.length} adaptador{ghosts.length > 1 ? "es" : ""} fantasma
              </Button>
            )}
          </div>

          {broken && broken.problem !== 22 && (
            <p className="text-xs text-mute">
              Si el error vuelve tras reiniciar la tarjeta: descarga el driver más reciente de la web del fabricante del portátil o de la tarjeta, desactiva el ahorro de
              energía y, si sigue, apaga el equipo del todo (no reiniciar) durante un minuto. Un fallo que va y viene también puede ser la tarjeta o su antena.
            </p>
          )}
        </div>
      )}
    </Card>
  );
}
