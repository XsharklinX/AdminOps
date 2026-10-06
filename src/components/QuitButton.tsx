import { Power } from "lucide-react";
import { useEffect, useState } from "react";
import { logQuietly, SETTINGS_SAVED, workApi } from "../lib/api";
import { analyzeQuick } from "../lib/diagRun";
import { useConfirm, useToast } from "./feedback";

/**
 * «Salir», en la barra de arriba. Solo aparece con el ajuste «Al cerrar la
 * ventana, minimizar»: entonces la X ya no cierra y hace falta otra salida.
 */
export function QuitButton() {
  const [show, setShow] = useState(false);
  const { confirm, dialog } = useConfirm();

  useEffect(() => {
    const read = () =>
      workApi
        .settings()
        .then((s) => setShow(s.closeMinimizes))
        .catch(logQuietly("QuitButton"));
    void read();
    window.addEventListener(SETTINGS_SAVED, read);
    return () => window.removeEventListener(SETTINGS_SAVED, read);
  }, []);

  // El «Análisis rápido» del icono junto al reloj se hace aquí, que siempre está montado.
  const toast = useToast();
  useEffect(() => {
    const run = () => {
      toast("info", "Análisis rápido en marcha…");
      analyzeQuick().then(
        (d) => {
          const n = d.findings.filter((f) => f.severity !== "info").length;
          toast(n ? "info" : "ok", n ? `Análisis rápido: ${n} ${n === 1 ? "cosa que atender" : "cosas que atender"}. Está en Diagnóstico.` : "Análisis rápido: nada urgente.");
        },
        (e) => toast("error", String(e)),
      );
    };
    window.addEventListener("adminops-tray-quick-scan", run);
    return () => window.removeEventListener("adminops-tray-quick-scan", run);
  }, [toast]);

  if (!show) return null;
  return (
    <>
      <button
        onClick={async () => {
          const ok = await confirm({
            title: "¿Salir de AdminOps?",
            body: <p>Se cierra del todo. Lo que esté a medias (un análisis, una copia) se detiene.</p>,
            confirmLabel: "Salir",
          });
          if (ok) void workApi.quit().catch(logQuietly("QuitButton"));
        }}
        className="grid size-8 place-items-center rounded-md text-mute transition-colors hover:bg-panel-2 hover:text-bad"
        title="Salir de AdminOps (la X solo minimiza)"
        aria-label="Salir de AdminOps"
      >
        <Power size={16} strokeWidth={1.6} />
      </button>
      {dialog}
    </>
  );
}
