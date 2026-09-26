import { ShieldAlert } from "lucide-react";
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
    <div className="flex items-center gap-3 border-b border-warn/25 bg-warn/[0.06] px-6 py-2.5 text-sm">
      <ShieldAlert size={16} className="shrink-0 text-warn" />
      <p className="flex-1 text-dim">
        <span className="text-warn">Sin permisos de administrador.</span> Puedes ver el sistema, pero no modificar el
        registro ni los servicios.
        {error && <span className="ml-2 text-bad">{error}</span>}
      </p>
      <button
        onClick={relaunch}
        disabled={busy}
        className="rounded-md border border-warn/40 px-3 py-1 text-xs font-medium text-warn transition-colors hover:bg-warn/10 disabled:opacity-50"
      >
        {busy ? "Esperando UAC…" : "Reiniciar como admin"}
      </button>
    </div>
  );
}
