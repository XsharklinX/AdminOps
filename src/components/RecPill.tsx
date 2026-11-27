import { listen } from "@tauri-apps/api/event";
import { GripVertical, Mic, PenLine, Video, Volume2 } from "lucide-react";
import { useEffect, useState } from "react";
import { faultRecApi, FAULTREC_CAMERA_EVENT, FAULTREC_DRAW_EVENT, FAULTREC_PAUSED_EVENT, FAULTREC_TICK_EVENT } from "../lib/api";
import { logQuietly } from "../lib/api/core";
import { clock } from "../lib/faultrec";

/**
 * El contador de «Grabar el fallo»: una ventanita sin marco y siempre encima (faultrec.rs) con el tiempo
 * y el botón «Parar». Windows la deja fuera de la grabación. Se cierra sola al parar.
 */
export function RecPill() {
  const [secs, setSecs] = useState(0);
  const [stopping, setStopping] = useState(false);
  const [mic, setMic] = useState(false);
  const [system, setSystem] = useState(false);
  const [paused, setPaused] = useState(false);
  const [drawing, setDrawing] = useState(false);
  const [camera, setCamera] = useState(false);
  // Lo que no se pudo hacer (dibujar o la cámara grabando una ventana…): sale unos segundos en lugar del tiempo.
  const [err, setErr] = useState<string | null>(null);
  useEffect(() => {
    if (!err) return;
    const t = window.setTimeout(() => setErr(null), 5000);
    return () => window.clearTimeout(t);
  }, [err]);
  const fail = (where: string) => (e: unknown) => {
    logQuietly(where)(e);
    setErr(String(e));
  };

  useEffect(() => {
    // Por si la ventana se abrió después del primer aviso.
    void faultRecApi
      .status()
      .then((s) => {
        setSecs(s.secs);
        setMic(!!s.mic);
        setSystem(!!s.systemAudio);
        setPaused(!!s.paused);
        setDrawing(!!s.drawing);
        setCamera(!!s.camera);
      })
      .catch(logQuietly("RecPill.status"));
    const off = listen<number>(FAULTREC_TICK_EVENT, (e) => setSecs(e.payload));
    const offPause = listen<boolean>(FAULTREC_PAUSED_EVENT, (e) => setPaused(e.payload));
    const offDraw = listen<boolean>(FAULTREC_DRAW_EVENT, (e) => setDrawing(e.payload));
    const offCam = listen<boolean>(FAULTREC_CAMERA_EVENT, (e) => setCamera(e.payload));
    return () => {
      void off.then((f) => f());
      void offPause.then((f) => f());
      void offDraw.then((f) => f());
      void offCam.then((f) => f());
    };
  }, []);

  const toggleDraw = () => {
    faultRecApi.draw(!drawing).then(setDrawing, fail("RecPill.draw"));
  };

  const toggleCamera = () => {
    faultRecApi.camera(!camera).then(setCamera, fail("RecPill.camera"));
  };

  const togglePause = () => {
    (paused ? faultRecApi.resume() : faultRecApi.pause()).then((s) => setPaused(!!s.paused), fail("RecPill.pause"));
  };

  const stop = () => {
    if (stopping) return;
    setStopping(true);
    faultRecApi.stop().catch((e) => {
      setStopping(false);
      fail("RecPill.stop")(e);
    });
  };

  return (
    // Todo menos «Parar» sirve de asa: se arrastra a otro sitio si tapa lo que hay que ver.
    <div data-tauri-drag-region className="flex h-screen cursor-grab items-center justify-between gap-2 overflow-hidden border border-line-2 bg-void px-3 text-ink select-none active:cursor-grabbing" title="Arrastra para moverla">
      <span data-tauri-drag-region className="flex min-w-0 items-center gap-2 text-[13px]" role="status" aria-live="off">
        <GripVertical size={14} className="pointer-events-none -ml-1 shrink-0 text-mute" aria-hidden />
        <span data-tauri-drag-region className={`h-2.5 w-2.5 shrink-0 rounded-full ${paused ? "bg-mute" : "animate-pulse bg-ok"}`} aria-hidden />
        {err ? (
          <span className="truncate text-[12px] text-bad" title={err}>
            {err}
          </span>
        ) : (
          <span data-tauri-drag-region className="truncate tabular">{/* Solo el tiempo: el punto que late ya dice que graba (con cuatro botones no cabe «Grabando»). */}
            {paused ? `En pausa ${clock(secs)}` : clock(secs)}
          </span>
        )}
        {mic && <Mic size={13} className="pointer-events-none shrink-0 text-ok" aria-label="Micrófono encendido" />}
        {system && <Volume2 size={13} className="pointer-events-none shrink-0 text-ok" aria-label="Sonido del equipo encendido" />}
      </span>
      <button type="button" onClick={togglePause} className="shrink-0 rounded-md border border-line-2 px-2.5 py-1 text-xs font-medium text-ink transition-colors hover:bg-panel">
        {paused ? "Seguir" : "Pausar"}
      </button>
      <button type="button" onClick={toggleDraw} aria-pressed={drawing} aria-label={drawing ? "Quitar el dibujo" : "Dibujar en la pantalla"} title={drawing ? "Quitar el dibujo" : "Dibujar en la pantalla (Esc lo quita)"} className={`shrink-0 rounded-md border px-2 py-1 transition-colors hover:bg-panel ${drawing ? "border-ok/60 text-ok" : "border-line-2 text-ink"}`}>
        <PenLine size={14} />
      </button>
      <button type="button" onClick={toggleCamera} aria-pressed={camera} aria-label={camera ? "Quitar la cámara" : "Cámara en una esquina"} title={camera ? "Quitar la cámara" : "Tu cámara en una esquina"} className={`shrink-0 rounded-md border px-2 py-1 transition-colors hover:bg-panel ${camera ? "border-ok/60 text-ok" : "border-line-2 text-ink"}`}>
        <Video size={14} />
      </button>
      <button type="button" onClick={stop} disabled={stopping} className="shrink-0 rounded-md border border-bad/50 px-2.5 py-1 text-xs font-medium text-bad transition-colors hover:bg-bad/10 disabled:opacity-50">
        Parar
      </button>
    </div>
  );
}
