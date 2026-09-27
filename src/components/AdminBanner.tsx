import { useState } from "react";
import { api } from "../lib/api";

export function AdminBanner() {
  const [error, setError] = useState<string | null>(null);
  const [busy, setBusy] = useState(false);

  const relaunch = async () => {
    setBusy(true);
    setError(null);
    try {
      await api.relaunchAsAdmin();
    } catch (e) {
      setError(String(e));
      setBusy(false);
    }
  };

  return (
    <div className="flex items-center gap-3 border-b border-line bg-panel px-8 py-2 text-[13px]">
      <span className="size-2 shrink-0 rounded-full bg-warn" />
      <p className="flex-1 text-dim">
        <span className="font-medium text-ink">Modo de solo lectura.</span> Sin permisos de administrador puedes ver el equipo, pero no
        cambiar el registro ni los servicios.
        {error && <span className="ml-2 text-bad">{error}</span>}
      </p>
      <button
        onClick={relaunch}
        disabled={busy}
        className="h-7 rounded-md border border-line-2 px-3 text-xs font-medium text-ink transition-colors hover:bg-panel-2 disabled:opacity-50"
      >
        {busy ? "Esperando UAC…" : "Reiniciar como administrador"}
      </button>
    </div>
  );
}
