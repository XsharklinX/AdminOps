import { Lock } from "lucide-react";
import { useState } from "react";
import logo from "../assets/logo.svg";
import { lockApi } from "../lib/api";
import { Button, inputClass } from "./ui";

/** Pantalla de bloqueo: PIN o contraseña de AdminOps, o la de Windows si se olvidó. */
export function LockScreen({ kind, onUnlock }: { kind: string; onUnlock: () => void }) {
  const [secret, setSecret] = useState("");
  const [windows, setWindows] = useState(false);
  const [error, setError] = useState("");
  const [busy, setBusy] = useState(false);
  const pin = kind === "pin" && !windows;

  const submit = async (e: React.FormEvent) => {
    e.preventDefault();
    if (!secret || busy) return;
    setBusy(true);
    setError("");
    try {
      const ok = windows ? await lockApi.verifyWindows(secret) : await lockApi.verify(secret);
      if (ok) onUnlock();
      else setError(windows ? "La contraseña de Windows no es correcta." : pin ? "PIN incorrecto." : "Contraseña incorrecta.");
    } catch (err) {
      setError(String(err));
    } finally {
      setBusy(false);
      setSecret("");
    }
  };

  return (
    <div className="fixed inset-0 z-[100] grid place-items-center bg-void">
      <form onSubmit={submit} className="flex w-80 flex-col items-center gap-4">
        <img src={logo} alt="" className="size-14" draggable={false} />
        <div className="text-center">
          <div className="flex items-center justify-center gap-2 text-lg font-semibold text-ink">
            <Lock size={16} /> AdminOps está bloqueado
          </div>
          <p className="mt-1 text-sm text-dim">{windows ? "Escribe la contraseña de Windows de esta cuenta." : pin ? "Escribe tu PIN." : "Escribe tu contraseña."}</p>
        </div>
        <input
          autoFocus
          type="password"
          inputMode={pin ? "numeric" : undefined}
          maxLength={pin ? 8 : 128}
          value={secret}
          onChange={(e) => setSecret(pin ? e.target.value.replace(/\D/g, "") : e.target.value)}
          className={`${inputClass} text-center ${pin ? "font-mono text-xl tracking-[0.5em]" : ""}`}
          aria-label={pin ? "PIN" : "Contraseña"}
        />
        {error && <p className="text-center text-sm text-bad">{error}</p>}
        <Button onClick={() => {}} disabled={busy || !secret}>
          Desbloquear
        </Button>
        <button
          type="button"
          onClick={() => {
            setWindows(!windows);
            setError("");
            setSecret("");
          }}
          className="text-xs text-mute hover:text-ink"
        >
          {windows ? "Usar el PIN o la contraseña de AdminOps" : "¿Lo olvidaste? Desbloquear con la contraseña de Windows"}
        </button>
      </form>
    </div>
  );
}
