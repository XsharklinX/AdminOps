import { listen } from "@tauri-apps/api/event";
import { GripVertical, Mic } from "lucide-react";
import { useEffect, useState } from "react";
import { faultRecApi, FAULTREC_TICK_EVENT } from "../lib/api";
import { logQuietly } from "../lib/api/core";
import { pillText } from "../lib/faultrec";

/**
 * El contador de «Grabar el fallo»: una ventanita sin marco y siempre encima (faultrec.rs) con el tiempo
 * y el botón «Parar». Windows la deja fuera de la grabación. Se cierra sola al parar.
 */
export function RecPill() {
  const [secs, setSecs] = useState(0);
  const [stopping, setStopping] = useState(false);
  const [mic, setMic] = useState(false);

  useEffect(() => {
    // Por si la ventana se abrió después del primer aviso.
    void faultRecApi
      .status()
      .then((s) => {
        setSecs(s.secs);
        setMic(!!s.mic);
      })
      .catch(logQuietly("RecPill.status"));
    const off = listen<number>(FAULTREC_TICK_EVENT, (e) => setSecs(e.payload));
    return () => {
      void off.then((f) => f());
    };
  }, []);

  const stop = () => {
    if (stopping) return;
    setStopping(true);
    faultRecApi.stop().catch((e) => {
      setStopping(false);
      logQuietly("RecPill.stop")(e);
    });
  };

  return (
    // Todo menos «Parar» sirve de asa: se arrastra a otro sitio si tapa lo que hay que ver.
    <div data-tauri-drag-region className="flex h-screen cursor-grab items-center justify-between gap-3 overflow-hidden border border-line-2 bg-void px-3 text-ink select-none active:cursor-grabbing" title="Arrastra para moverla">
      <span data-tauri-drag-region className="flex min-w-0 items-center gap-2 text-[13px]" role="status" aria-live="off">
        <GripVertical size={14} className="pointer-events-none -ml-1 shrink-0 text-mute" aria-hidden />
        <span data-tauri-drag-region className="h-2.5 w-2.5 shrink-0 animate-pulse rounded-full bg-ok" aria-hidden />
        <span data-tauri-drag-region className="truncate tabular">{pillText(secs)}</span>
        {mic && <Mic size={13} className="pointer-events-none shrink-0 text-ok" aria-label="Micrófono encendido" />}
      </span>
      <button type="button" onClick={stop} disabled={stopping} className="shrink-0 rounded-md border border-bad/50 px-2.5 py-1 text-xs font-medium text-bad transition-colors hover:bg-bad/10 disabled:opacity-50">
        Parar
      </button>
    </div>
  );
}
