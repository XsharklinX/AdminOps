import { Eye, Loader2, RotateCw } from "lucide-react";
import { useEffect, useState } from "react";
import { diagApi, type ReportOptions } from "../lib/api";
import { iconBtn } from "./ui";

/** Tras dejar de escribir, cuánto se espera para volver a pintar la vista previa. */
const WAIT_MS = 700;

/**
 * El informe tal como saldrá, al lado del formulario. Es el mismo HTML que va
 * al PDF, con «BORRADOR» como número: no se crea ningún archivo ni se gasta un
 * número. Se pinta sin scripts (sandbox) y con el último análisis guardado.
 */
export function ReportPreview({ options }: { options: ReportOptions }) {
  const [html, setHtml] = useState<string | null>(null);
  const [error, setError] = useState<string | null>(null);
  const [busy, setBusy] = useState(false);
  const [again, setAgain] = useState(0);
  const key = JSON.stringify(options);

  useEffect(() => {
    let stale = false;
    const t = window.setTimeout(() => {
      setBusy(true);
      diagApi
        .previewReport(JSON.parse(key) as ReportOptions)
        .then(
          (h) => {
            if (stale) return;
            setHtml(h);
            setError(null);
          },
          (e) => !stale && setError(String(e)),
        )
        .finally(() => !stale && setBusy(false));
    }, WAIT_MS);
    return () => {
      stale = true;
      window.clearTimeout(t);
    };
  }, [key, again]);

  return (
    <section className="flex min-w-0 flex-col overflow-hidden rounded-xl border border-line bg-panel xl:sticky xl:top-4" aria-label="Vista previa del informe">
      <header className="flex items-center gap-2 border-b border-line px-4 py-2.5">
        <Eye size={14} className="text-mute" />
        <h2 className="text-[13px] font-semibold text-ink">Vista previa</h2>
        {busy && <Loader2 size={13} className="animate-spin text-neon" />}
        <span className="ml-auto text-[11px] text-mute">Con el último análisis · número «BORRADOR»</span>
        <button onClick={() => setAgain((n) => n + 1)} className={iconBtn} title="Volver a pintarla" aria-label="Volver a pintar la vista previa">
          <RotateCw size={13} />
        </button>
      </header>
      {error ? (
        <p className="p-4 text-sm text-mute">{error}</p>
      ) : html === null ? (
        <p className="flex items-center gap-2 p-4 text-sm text-mute">
          <Loader2 size={13} className="animate-spin" /> Preparando la vista previa…
        </p>
      ) : (
        // Fondo blanco como el papel: el informe está pensado para imprimirse.
        <iframe title="Vista previa del informe" srcDoc={html} sandbox="" className="h-[75vh] w-full bg-white" />
      )}
    </section>
  );
}
