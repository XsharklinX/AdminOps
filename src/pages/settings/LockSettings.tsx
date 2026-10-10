import { KeyRound, Lock, LockOpen } from "lucide-react";
import { useCallback, useEffect, useState } from "react";
import { useToast } from "../../components/feedback";
import { Button, Card, inputClass, Loading } from "../../components/ui";
import { logQuietly, appApi, lockApi, type LockStatus } from "../../lib/api";

const IDLE = [
  [0, "Solo al abrir AdminOps"],
  [1, "Tras 1 minuto sin usarla"],
  [5, "Tras 5 minutos"],
  [10, "Tras 10 minutos"],
  [15, "Tras 15 minutos"],
  [30, "Tras 30 minutos"],
  [60, "Tras 1 hora"],
] as const;

const notify = () => window.dispatchEvent(new Event("adminops-lock"));

/** Poner, cambiar o quitar el PIN / contraseña de AdminOps. */
export function LockSettings() {
  const [portable, setPortable] = useState(false);
  useEffect(() => {
    appApi
      .info()
      .then((i) => setPortable(i.portable))
      .catch(logQuietly("LockSettings"));
  }, []);
  const [status, setStatus] = useState<LockStatus | null>(null);
  const [kind, setKind] = useState<"pin" | "password">("pin");
  const [secret, setSecret] = useState("");
  const [repeat, setRepeat] = useState("");
  const [current, setCurrent] = useState("");
  const [idle, setIdle] = useState(5);
  const [changing, setChanging] = useState(false);
  const [busy, setBusy] = useState(false);
  const toast = useToast();

  const load = useCallback(() => {
    lockApi.status().then((s) => {
      setStatus(s);
      if (s.enabled) {
        setIdle(s.idleMinutes);
        setKind(s.kind === "password" ? "password" : "pin");
      }
    }).catch(logQuietly("LockSettings"));
  }, []);
  useEffect(load, [load]);

  const valid = kind === "pin" ? /^\d{4,8}$/.test(secret) : secret.length >= 6;
  const reset = () => {
    setSecret("");
    setRepeat("");
    setCurrent("");
    setChanging(false);
  };

  const apply = async () => {
    setBusy(true);
    try {
      await lockApi.set(kind, secret, idle, current);
      toast("ok", status?.enabled ? "Bloqueo actualizado." : "AdminOps pedirá el " + (kind === "pin" ? "PIN" : "contraseña") + " al abrirse.");
      reset();
      load();
      notify();
    } catch (e) {
      toast("error", String(e));
    } finally {
      setBusy(false);
    }
  };

  const disable = async () => {
    setBusy(true);
    try {
      await lockApi.disable(current);
      toast("ok", "Bloqueo quitado.");
      reset();
      load();
      notify();
    } catch (e) {
      toast("error", String(e));
    } finally {
      setBusy(false);
    }
  };

  const changeIdle = async (minutes: number) => {
    setIdle(minutes);
    if (!status?.enabled) return;
    try {
      await lockApi.setIdle(minutes, current);
      load();
      notify();
    } catch (e) {
      toast("error", String(e));
      setIdle(status.idleMinutes);
    }
  };

  if (!status) return <Loading />;

  const secretFields = (
    <div className="space-y-3">
      <div className="inline-flex rounded-lg border border-line bg-void p-0.5">
        {(
          [
            ["pin", "PIN (4 a 8 números)"],
            ["password", "Contraseña"],
          ] as const
        ).map(([k, label]) => (
          <button
            key={k}
            onClick={() => {
              setKind(k);
              setSecret("");
              setRepeat("");
            }}
            className={`rounded-md px-3 py-1.5 text-[13px] ${kind === k ? "bg-panel-2 font-medium text-ink" : "text-dim hover:text-ink"}`}
          >
            {label}
          </button>
        ))}
      </div>
      <div className="grid grid-cols-2 gap-3">
        <input
          type="password"
          inputMode={kind === "pin" ? "numeric" : undefined}
          value={secret}
          onChange={(e) => setSecret(kind === "pin" ? e.target.value.replace(/\D/g, "").slice(0, 8) : e.target.value)}
          placeholder={kind === "pin" ? "Nuevo PIN" : "Nueva contraseña (mínimo 6)"}
          className={inputClass}
        />
        <input
          type="password"
          inputMode={kind === "pin" ? "numeric" : undefined}
          value={repeat}
          onChange={(e) => setRepeat(kind === "pin" ? e.target.value.replace(/\D/g, "").slice(0, 8) : e.target.value)}
          placeholder="Repítelo"
          className={inputClass}
        />
      </div>
      {repeat && repeat !== secret && <p className="text-xs text-bad">No coinciden.</p>}
    </div>
  );

  return (
    <div className="space-y-4">
      <Card title="Bloqueo de AdminOps" icon={status.enabled ? <Lock size={14} /> : <LockOpen size={14} />}>
        <p className="mb-4 text-sm text-dim">
          Pide un PIN o una contraseña al abrir AdminOps y, si quieres, tras un tiempo sin usarla. Útil si el equipo lo usan otras personas o lo dejas en
          casa de un cliente. Bloquea la aplicación; para cifrar también los datos usa «Cifrar mis datos», más abajo (y la Caja fuerte para carpetas enteras). Si lo olvidas, se desbloquea con la contraseña de
          Windows de esta cuenta.
        </p>
        {portable && (
          <p className="mb-4 rounded-lg border border-neon/30 bg-neon/5 px-3 py-2 text-xs text-dim">
            <b className="text-ink">En el pendrive, el PIN también protege tus contraseñas guardadas</b> (portales y routers): si pierdes el pendrive,
            nadie puede leerlas sin él. Por eso, si olvidas el PIN, esas contraseñas no se pueden recuperar (con la contraseña de Windows se abre AdminOps, pero no
            ellas): tendrías que volver a guardarlas.
          </p>
        )}

        {status.enabled ? (
          <div className="space-y-4">
            <div className="flex items-center gap-2 text-sm">
              <span className="size-2 rounded-full bg-ok" />
              <span className="text-ink">Activado con {status.kind === "pin" ? "PIN" : "contraseña"}.</span>
              <span className="text-dim">Ctrl+L o el candado de la barra lateral lo bloquean al momento.</span>
            </div>
            <label className="block">
              <span className="mb-1 block text-xs text-dim">{status.kind === "pin" ? "PIN" : "Contraseña"} actual (para cambiarlo o quitarlo)</span>
              <input type="password" value={current} onChange={(e) => setCurrent(e.target.value)} className={`${inputClass} max-w-xs`} />
            </label>
            {changing ? (
              <>
                {secretFields}
                <div className="flex gap-2">
                  <Button onClick={apply} disabled={busy || !current || !valid || secret !== repeat}>
                    <KeyRound size={14} /> Guardar
                  </Button>
                  <Button kind="ghost" onClick={() => setChanging(false)}>
                    Cancelar
                  </Button>
                </div>
              </>
            ) : (
              <div className="flex gap-2">
                <Button kind="ghost" onClick={() => setChanging(true)}>
                  <KeyRound size={14} /> Cambiar {status.kind === "pin" ? "PIN" : "contraseña"}
                </Button>
                <Button kind="danger" onClick={disable} disabled={busy || !current}>
                  Quitar el bloqueo
                </Button>
              </div>
            )}
          </div>
        ) : (
          <div className="space-y-4">
            {secretFields}
            <Button onClick={apply} disabled={busy || !valid || secret !== repeat}>
              <Lock size={14} /> Activar el bloqueo
            </Button>
          </div>
        )}
      </Card>

      <Card title="Bloquear por inactividad">
        <select value={idle} onChange={(e) => changeIdle(Number(e.target.value))} className="rounded-md border border-line bg-void/60 px-3 py-1.5 text-sm text-ink outline-none">
          {IDLE.map(([v, label]) => (
            <option key={v} value={v}>
              {label}
            </option>
          ))}
        </select>
        <p className="mt-2 text-[11px] text-mute">
          {status.enabled ? "Para pasar a «Solo al abrir» escribe arriba el PIN o la contraseña actual." : "Se aplica al activar el bloqueo."}
        </p>
      </Card>
    </div>
  );
}
