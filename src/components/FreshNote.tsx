// Nota discreta junto a lo que se enseña de memoria: «actualizado hace 3 min ·
// actualizando…». Cuando llega lo nuevo, desaparece el «actualizando».
import { Loader2 } from "lucide-react";
import { useEffect, useState } from "react";
import { ago } from "./ResumeRibbon";

export function FreshNote({ at, refreshing }: { at: number | null; refreshing: boolean }) {
  const [, tick] = useState(0);
  useEffect(() => {
    const t = window.setInterval(() => tick((n) => n + 1), 30_000);
    return () => window.clearInterval(t);
  }, []);
  if (!at) return null;
  return (
    <span className="inline-flex items-center gap-1 text-[11px] text-mute" aria-live="polite">
      {refreshing && <Loader2 size={10} className="animate-spin" />}
      {refreshing ? `Datos de ${ago(Date.now() - at)} · actualizando…` : `Actualizado ${ago(Date.now() - at)}`}
    </span>
  );
}
