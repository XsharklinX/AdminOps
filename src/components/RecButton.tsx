// «Grabar el fallo» a mano en la barra de arriba, junto a Teams y Correo: un clic empieza a grabar la
// pantalla principal y otro para. Mientras graba, el botón se enciende (punto verde que late y el tiempo)
// para que se vea desde cualquier pantalla. Al terminar lleva a la grabación con sus marcas.
// Para elegir otro monitor, una ventana o una zona: clic derecho, que abre la pantalla completa de Grabar el fallo.
// Con «Vigilar» encendido lleva un puntito verde, y un clic es «¡Acaba de pasar!»: guarda el último minuto.
import { listen } from "@tauri-apps/api/event";
import { Loader2 } from "lucide-react";
import { useEffect, useState } from "react";
import { faultRecApi, FAULTREC_DONE_EVENT, FAULTREC_OPEN_EVENT, FAULTREC_TICK_EVENT, type FaultRecDone } from "../lib/api";
import { logQuietly } from "../lib/api/core";
import { clock, getRecOptions } from "../lib/faultrec";
import { RecDot } from "./BrandIcons";
import { useToast } from "./feedback";
import type { PageId } from "./Sidebar";

export function RecButton({ onNavigate }: { onNavigate: (page: PageId, section?: string | null) => void }) {
  const toast = useToast();
  const [recording, setRecording] = useState(false);
  const [secs, setSecs] = useState(0);
  // Empezando o parando: el botón espera y no admite otro clic.
  const [busy, setBusy] = useState(false);
  // Vigilar encendido (el último minuto en memoria). Se mira cada 5 s: se enciende y apaga desde Grabar el fallo.
  const [watching, setWatching] = useState(false);
  useEffect(() => {
    const check = () =>
      faultRecApi.watchStatus().then((w) => {
        setWatching(!!w?.on && !w.pausedForRecording);
        // Vigilar se apagó solo (p. ej. se desconectó el monitor): se avisa desde cualquier pantalla.
        if (w?.error) toast("error", w.error);
      }, logQuietly("RecButton.watch"));
    void check();
    const t = window.setInterval(() => void check(), 5000);
    return () => window.clearInterval(t);
  }, [toast]);

  useEffect(() => {
    // Si ya se estaba grabando al abrir esta ventana (p. ej. tras recargar), se enciende igual.
    faultRecApi
      .status()
      .then((s) => {
        setRecording(s.recording);
        setSecs(s.secs);
      })
      .catch(logQuietly("RecButton.status"));
    const offs = [
      listen<number>(FAULTREC_TICK_EVENT, (e) => {
        setRecording(true);
        setSecs(e.payload);
      }),
      // Se pulsó el aviso de Windows de una grabación guardada sola o con Ctrl+Alt+G.
      listen(FAULTREC_OPEN_EVENT, () => onNavigate("machine", "recorder")),
      listen<FaultRecDone>(FAULTREC_DONE_EVENT, (e) => {
        setRecording(false);
        setBusy(false);
        setSecs(0);
        // Con o sin error, la pantalla de la grabación es donde se ve qué pasó (allí sale el aviso). Lo guardado con
        // Ctrl+Alt+G no cambia de pantalla: para eso está el aviso de Windows con «ver grabación».
        if ((e.payload.id || e.payload.error) && !e.payload.replay) onNavigate("machine", "recorder");
      }),
    ];
    return () => {
      for (const off of offs) void off.then((f) => f());
    };
  }, [onNavigate]);

  const click = async () => {
    if (busy) return;
    setBusy(true);
    try {
      if (recording) {
        await faultRecApi.stop();
        setRecording(false);
        onNavigate("machine", "recorder");
      } else if (watching) {
        await faultRecApi.justHappened();
        toast("ok", "Guardado el último minuto.");
        onNavigate("machine", "recorder");
      } else {
        // Siempre la pantalla entera; clics, teclas y voz como se dejaron en Grabar el fallo.
        const o = getRecOptions();
        const s = await faultRecApi.start(null, { input: o.input, mic: o.mic });
        setRecording(s.recording);
        setSecs(s.secs);
      }
    } catch (e) {
      const msg = String(e);
      toast(msg.includes("antes de empezar a grabar") ? "info" : "error", msg);
    } finally {
      setBusy(false);
    }
  };

  const openPage = (e: React.MouseEvent) => {
    e.preventDefault();
    onNavigate("machine", "recorder");
  };

  if (recording) {
    return (
      <button
        onClick={() => void click()}
        onContextMenu={openPage}
        disabled={busy}
        className="flex h-8 shrink-0 items-center gap-1.5 rounded-md border border-ok/40 bg-ok/10 px-2 text-xs font-medium text-ink tabular transition-colors hover:border-ok"
        title="Grabando la pantalla · clic para parar y ver la grabación"
        aria-label={`Grabando ${clock(secs)}. Parar`}
      >
        {busy ? <Loader2 size={13} className="animate-spin" /> : <RecDot size={14} recording />}
        {clock(secs)}
      </button>
    );
  }
  return (
    <button
      onClick={() => void click()}
      onContextMenu={openPage}
      disabled={busy}
      className="relative grid size-8 place-items-center rounded-md text-mute transition-colors hover:bg-panel-2 hover:text-ink disabled:opacity-60"
      title={
        watching
          ? "Vigilando: el último minuto está en memoria · clic: ¡Acaba de pasar! (guardarlo) · clic derecho: Grabar el fallo"
          : "Grabar la pantalla (hasta 2 minutos, con los errores y cuelgues marcados) · clic derecho: elegir qué grabar y ver grabaciones"
      }
      aria-label={watching ? "Vigilando. Guardar el último minuto" : "Grabar la pantalla"}
    >
      {busy ? <Loader2 size={16} className="animate-spin" /> : <RecDot size={19} />}
      {watching && !busy && <span className="absolute top-1 right-1 size-2 animate-pulse rounded-full bg-ok ring-2 ring-void" aria-hidden />}
    </button>
  );
}
