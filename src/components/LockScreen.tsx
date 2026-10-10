import { Lock } from "lucide-react";
import { useEffect, useState } from "react";
import logo from "../assets/logo.svg";
import { dataCryptApi, lockApi, workApi, type Settings } from "../lib/api";
import { Button, inputClass } from "./ui";

/** Pantalla de bloqueo: PIN o contraseña de AdminOps, o la de Windows si se olvidó. */
export function LockScreen({ kind, onUnlock }: { kind: string; onUnlock: () => void }) {
  const [secret, setSecret] = useState("");
  const [windows, setWindows] = useState(false);
  const [error, setError] = useState("");
  const [busy, setBusy] = useState(false);
  const [needKey, setNeedKey] = useState(false);
  const pin = kind === "pin" && !windows;
  // La marca del técnico: si alguien se acerca al equipo desatendido, ve su nombre y un teléfono.
  const [brand, setBrand] = useState<Settings | null>(null);
  useEffect(() => {
    let alive = true;
    workApi.settings().then((s) => alive && setBrand(s), () => {});
    return () => {
      alive = false;
    };
  }, []);
  const color = brand?.brandColor && /^#[0-9a-f]{6}$/i.test(brand.brandColor) ? brand.brandColor : null;
  const name = brand?.company?.trim() || "";

  const submit = async (e: React.FormEvent) => {
    e.preventDefault();
    if (!secret || busy) return;
    setBusy(true);
    setError("");
    try {
      const ok = windows ? await lockApi.verifyWindows(secret) : await lockApi.verify(secret);
      if (ok) {
        // Con la contraseña de Windows no se puede abrir la clave de los datos cifrados: hace falta la de rescate.
        if (windows && (await dataCryptApi.status().then((d) => d.enabled && !d.unlocked, () => false))) setNeedKey(true);
        else onUnlock();
      } else setError(windows ? "La contraseña de Windows no es correcta." : pin ? "PIN incorrecto." : "Contraseña incorrecta.");
    } catch (err) {
      setError(String(err));
    } finally {
      setBusy(false);
      setSecret("");
    }
  };

  const openWithKey = async (e: React.FormEvent) => {
    e.preventDefault();
    if (!secret || busy) return;
    setBusy(true);
    setError("");
    try {
      if (await dataCryptApi.unlockRecovery(secret)) onUnlock();
      else setError("Esa clave de rescate no es correcta.");
    } catch (err) {
      setError(String(err));
    } finally {
      setBusy(false);
    }
  };

  if (needKey)
    return (
      <div className="fixed inset-0 z-[100] grid place-items-center bg-void">
        <form onSubmit={openWithKey} className="flex w-96 flex-col items-center gap-4">
          <Lock size={28} className="text-mute" />
          <div className="text-center">
            <div className="text-lg font-semibold text-ink">Tus datos están cifrados</div>
            <p className="mt-1 text-sm text-dim">Entraste con la contraseña de Windows: para abrir los datos escribe la clave de rescate.</p>
          </div>
          <input autoFocus value={secret} onChange={(e) => setSecret(e.target.value)} placeholder="XXXXX-XXXXX-XXXXX-XXXXX-XXXXX" className={`${inputClass} text-center font-mono`} aria-label="Clave de rescate" />
          {error && <p className="text-center text-sm text-bad">{error}</p>}
          <Button onClick={() => {}} disabled={busy || secret.trim().length < 10}>
            Abrir los datos
          </Button>
          <button type="button" onClick={onUnlock} className="text-xs text-mute hover:text-ink">
            Entrar sin abrir los datos (se verán vacíos y no se podrá guardar nada)
          </button>
        </form>
      </div>
    );

  return (
    <div
      className="fixed inset-0 z-[100] grid place-items-center bg-void"
      style={color ? { background: `radial-gradient(ellipse 70% 55% at 50% 0%, color-mix(in srgb, ${color} 28%, transparent), transparent 70%), var(--color-void)` } : undefined}
    >
      <form onSubmit={submit} className="flex w-80 flex-col items-center gap-4">
        <img src={brand?.logo ?? logo} alt="" className="size-16 object-contain" draggable={false} />
        {name && (
          <div className="text-center">
            <div className="text-xl font-semibold tracking-tight text-ink">{name}</div>
            {brand?.tagline?.trim() && <div className="mt-0.5 text-sm text-dim">{brand.tagline.trim()}</div>}
          </div>
        )}
        <div className="text-center">
          <div className="flex items-center justify-center gap-2 text-lg font-semibold text-ink" style={color ? { color } : undefined}>
            <Lock size={16} /> {name ? "Equipo bloqueado" : "AdminOps está bloqueado"}
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
        {name && brand?.phone?.trim() && <p className="text-xs text-mute">Soporte: {brand.phone.trim()}</p>}
      </form>
    </div>
  );
}
