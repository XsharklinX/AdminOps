import { ShieldAlert } from "lucide-react";
import { useState, type ReactNode } from "react";
import { api } from "../lib/api";

/** Reinicia AdminOps con permisos de administrador (Windows pide confirmación con UAC). */
function useRelaunch() {
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
  return { relaunch, busy, error };
}

const relaunchBtn =
  "h-7 shrink-0 rounded-md border border-line-2 px-3 text-xs font-medium text-ink transition-colors hover:bg-panel-2 disabled:opacity-50";

/** Aviso de una pantalla que necesita administrador para algo, con el botón para reiniciar así. */
export function NeedsAdmin({ children, className = "" }: { children: ReactNode; className?: string }) {
  const { relaunch, busy, error } = useRelaunch();
  return (
    <div className={`flex flex-wrap items-center gap-x-3 gap-y-2 rounded-lg border border-warn/30 bg-warn/5 px-3 py-2 text-xs text-warn ${className}`} role="note">
      <ShieldAlert size={14} className="shrink-0" />
      <p className="min-w-48 flex-1">
        {children}
        {error && <span className="ml-2 text-bad">{error}</span>}
      </p>
      <button onClick={() => void relaunch()} disabled={busy} className={relaunchBtn}>
        {busy ? "Esperando UAC…" : "Reiniciar como administrador"}
      </button>
    </div>
  );
}
