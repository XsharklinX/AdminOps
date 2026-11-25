// Abrir a salvo (1.5): si AdminOps se cerró de golpe dos veces seguidas, esta vez
// abrió sin nada en segundo plano. La franja lo dice, cuenta qué estaba haciendo y
// deja volver a lo normal o copiar el informe del fallo. Ver src-tauri/src/safestart.rs.
import { ShieldAlert } from "lucide-react";
import { useEffect, useState } from "react";
import { logQuietly, safeStartApi, type SafeStartInfo } from "../lib/api";
import { useToast } from "./feedback";

const times = (n: number) => (n === 2 ? "dos veces" : `${n} veces`);

/** Las dos veces estaba… / La primera vez… y la segunda… (igual que `explain` en Rust). */
export function explainDoing(doing: string[]): string {
  const known = doing.filter((d) => d.trim());
  if (known.length === 0) return "";
  if (doing.length === 2 && known.length === 2) {
    return doing[0] === doing[1] ? `Las dos veces estaba ${doing[0]}.` : `La primera vez estaba ${doing[0]} y la segunda, ${doing[1]}.`;
  }
  return `La última vez estaba ${known[known.length - 1]}.`;
}

export function SafeStartBanner() {
  const [info, setInfo] = useState<SafeStartInfo | null>(null);
  const [hidden, setHidden] = useState(false);
  const [busy, setBusy] = useState(false);
  const toast = useToast();
  useEffect(() => {
    safeStartApi.info().then(setInfo).catch(logQuietly("SafeStart"));
  }, []);
  if (!info?.safe || hidden) return null;
  const detail = explainDoing(info.doing);
  const copy = () =>
    safeStartApi
      .report()
      .then((t) => navigator.clipboard.writeText(t))
      .then(() => toast("ok", "Informe del fallo copiado: pégalo en el aviso o en el caso."))
      .catch((e) => toast("error", `No se pudo copiar el informe: ${String(e)}`));
  const normal = () => {
    setBusy(true);
    safeStartApi.leave().catch((e) => {
      setBusy(false);
      toast("error", `No se pudo reiniciar AdminOps: ${String(e)}`);
    });
  };
  return (
    <div role="alert" className="flex flex-wrap items-center gap-x-4 gap-y-2 border-b border-warn/40 bg-warn/10 px-8 py-2.5 text-sm">
      <ShieldAlert size={16} className="shrink-0 text-warn" />
      <div className="min-w-0 flex-1">
        <p className="font-medium text-warn">AdminOps se cerró de golpe {times(info.badCloses)} seguidas.</p>
        <p className="text-dim">
          {detail && `${detail} `}Se ha abierto a salvo: sin vigilar, sin monitor en vivo y sin tareas nocturnas.
        </p>
      </div>
      <div className="flex shrink-0 flex-wrap gap-2">
        <button type="button" onClick={() => setHidden(true)} className="rounded border border-warn/50 px-2.5 py-1 text-xs text-warn hover:bg-warn/15">
          Seguir a salvo
        </button>
        <button type="button" disabled={busy} onClick={normal} className="rounded border border-line px-2.5 py-1 text-xs text-ink hover:bg-panel-2 disabled:opacity-50">
          Abrir normal
        </button>
        <button type="button" onClick={() => void copy()} className="rounded border border-line px-2.5 py-1 text-xs text-ink hover:bg-panel-2">
          Copiar el informe del fallo
        </button>
      </div>
    </div>
  );
}
