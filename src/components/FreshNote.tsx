// Nota discreta junto a lo que se enseña de memoria: «actualizado hace 3 min ·
// actualizando…». Cuando llega lo nuevo, desaparece el «actualizando».
import { Loader2 } from "lucide-react";
import { useState } from "react";
import { usePolling } from "../hooks/usePolling";
import { ago } from "./ResumeRibbon";

export function FreshNote({ at, refreshing, error }: { at: number | null; refreshing: boolean; error?: string | null }) {
  const [, tick] = useState(0);
  usePolling(() => tick((n) => n + 1), 30_000);
  if (!at) return null;
  return (
    <span className="inline-flex items-center gap-1 text-[11px] text-mute" aria-live="polite">
      {refreshing && !error && <Loader2 size={10} className="animate-spin" />}
      {error ? (
        <>
          <span className="text-bad" title={error}>No se pudo actualizar</span>
          <span>· lectura de {ago(Date.now() - at)}</span>
        </>
      ) : refreshing ? (
        `De ${ago(Date.now() - at)} · actualizando…`
      ) : (
        `Al día · leído ${ago(Date.now() - at)}`
      )}
    </span>
  );
}
