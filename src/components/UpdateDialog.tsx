// La ventana de «Versión nueva»: enseña las novedades y, si el técnico acepta, descarga la
// versión, comprueba su huella y la instala sin salir de AdminOps. El instalador trabaja solo
// (sin preguntar nada) y AdminOps vuelve a abrirse al terminar.
import { CheckCircle2, Download, Loader2, RotateCw } from "lucide-react";
import { useState } from "react";
import { appApi, appcareApi, type UpdateInfo } from "../lib/api";
import { bytes } from "../lib/format";
import { useTaskMessage } from "../hooks/useTask";
import { Button, Modal, Notice } from "./ui";

type Phase = "ask" | "working" | "restarting" | "manual" | "error";

/** Las notas de la versión (Markdown de GitHub) en líneas legibles: títulos, puntos y párrafos. */
export function noteLines(md: string): { kind: "h" | "li" | "p"; text: string }[] {
  const clean = (t: string) => t.replace(/\*\*|__|`/g, "").replace(/\[([^\]]+)\]\([^)]*\)/g, "$1").trim();
  return md
    .split(/\r?\n/)
    .map((l) => l.trim())
    .filter(Boolean)
    .map((l) => {
      if (/^#{1,6}\s/.test(l)) return { kind: "h" as const, text: clean(l.replace(/^#+\s*/, "")) };
      if (/^[-*•]\s/.test(l)) return { kind: "li" as const, text: clean(l.replace(/^[-*•]\s*/, "")) };
      return { kind: "p" as const, text: clean(l) };
    })
    .filter((l) => l.text);
}

/** El porcentaje de «Descargando la versión X… 42 %», o null mientras no lo hay. */
export function percentOf(message: string | null): number | null {
  const m = message?.match(/(\d{1,3})\s*%/);
  return m ? Math.min(100, Number(m[1])) : null;
}

/**
 * Sigue montada aunque esté cerrada (`open` falso) o la pantalla esté bloqueada: así una descarga
 * en marcha no se pierde y, al volver a abrirla, enseña por dónde va.
 */
export function UpdateDialog({ info, open, onClose }: { info: UpdateInfo; open: boolean; onClose: () => void }) {
  const [phase, setPhase] = useState<Phase>("ask");
  const [error, setError] = useState("");
  const [portable, setPortable] = useState(false);
  const message = useTaskMessage("update", phase === "working");
  const [cancelling, setCancelling] = useState(false);
  const pct = percentOf(message);
  const lines = noteLines(info.notes).slice(0, 40);

  const start = async () => {
    setPhase("working");
    setCancelling(false);
    setError("");
    try {
      const r = await appcareApi.installUpdate();
      setPortable(r.how !== "installer");
      setPhase(r.how === "portable-manual" ? "manual" : "restarting");
    } catch (e) {
      const text = String(e);
      // Cancelada por el técnico: vuelve a la pregunta, sin error.
      if (/cancelada/i.test(text)) setPhase("ask");
      else {
        setError(text);
        setPhase("error");
      }
    }
  };

  // Mientras descarga se puede dejar en segundo plano (la barra lateral la vuelve a abrir);
  // cuando ya se está instalando, no hace falta cerrarla: AdminOps se cierra sola.
  const close = () => {
    if (phase !== "restarting") onClose();
  };

  if (!open) return null;

  const footer =
    phase === "ask" ? (
      <>
        <Button kind="ghost" onClick={onClose}>
          Más tarde
        </Button>
        <Button onClick={() => void start()}>
          <Download size={14} /> Actualizar ahora
        </Button>
      </>
    ) : phase === "working" ? (
      <>
        <Button
          kind="ghost"
          disabled={cancelling}
          onClick={() => {
            setCancelling(true);
            void appApi.cancelTask("update");
          }}
        >
          {cancelling ? "Cancelando…" : "Cancelar"}
        </Button>
        <Button kind="secondary" onClick={onClose}>
          Seguir en segundo plano
        </Button>
      </>
    ) : phase === "error" ? (
      <>
        <Button kind="ghost" onClick={onClose}>
          Cerrar
        </Button>
        <Button onClick={() => void start()}>
          <RotateCw size={14} /> Reintentar
        </Button>
      </>
    ) : phase === "manual" ? (
      <Button onClick={onClose}>Entendido</Button>
    ) : undefined;

  return (
    <Modal title={`Versión ${info.latest} disponible`} onClose={close} footer={footer} width="w-[540px]">
      {phase === "ask" && (
        <div className="space-y-3">
          <p className="text-sm text-dim">
            Tienes la {info.current}. Se descarga ({bytes(info.size)}), se comprueba su huella y se instala aquí mismo: AdminOps se cierra un momento y vuelve a abrirse
            sola. Tus datos, casos y ajustes se conservan.
          </p>
          {lines.length > 0 && (
            <div className="max-h-[42vh] overflow-y-auto rounded-lg border border-line bg-panel-2/50 px-4 py-3">
              <p className="mb-1.5 text-xs uppercase tracking-wide text-mute">Novedades</p>
              <div className="space-y-1 text-[13px] leading-relaxed">
                {lines.map((l, i) =>
                  l.kind === "h" ? (
                    <p key={i} className="pt-1.5 font-semibold text-ink first:pt-0">
                      {l.text}
                    </p>
                  ) : l.kind === "li" ? (
                    <p key={i} className="flex gap-2 text-dim">
                      <span className="mt-[7px] size-1 shrink-0 rounded-full bg-neon" />
                      <span>{l.text}</span>
                    </p>
                  ) : (
                    <p key={i} className="text-dim">
                      {l.text}
                    </p>
                  ),
                )}
              </div>
            </div>
          )}
        </div>
      )}

      {phase === "working" && (
        <div className="space-y-3 py-2">
          <p className="flex items-center gap-2 text-sm text-ink">
            <Loader2 size={15} className="animate-spin text-neon" />
            {message ?? "Preparando la descarga…"}
          </p>
          <div className="h-1.5 overflow-hidden rounded-full bg-panel-2">
            <div className={`h-full rounded-full bg-neon transition-[width] duration-300 ${pct === null ? "w-1/4 animate-pulse" : ""}`} style={pct === null ? undefined : { width: `${pct}%` }} />
          </div>
          <p className="text-xs text-mute">Con «Seguir en segundo plano» puedes usar AdminOps mientras tanto; el aviso de la barra lateral vuelve a abrir esta ventana.</p>
        </div>
      )}

      {phase === "restarting" && (
        <div className="flex items-start gap-3 py-2">
          <CheckCircle2 size={20} className="mt-0.5 shrink-0 text-ok" />
          <div className="space-y-1">
            <p className="text-sm text-ink">Descargada y comprobada.</p>
            <p className="text-sm text-dim">
              {portable
                ? `AdminOps se cierra para cambiar a la ${info.latest} y vuelve a abrirse sola en unos segundos.`
                : `Instalando la ${info.latest}. AdminOps se cierra en un momento y vuelve a abrirse sola al terminar.`}
            </p>
          </div>
        </div>
      )}

      {phase === "manual" && (
        <p className="py-2 text-sm text-dim">
          La {info.latest} está descargada en tu carpeta de Descargas (se abrió), pero esta carpeta del portable no se puede cambiar sola. Cierra AdminOps y descomprime el .zip
          encima de la carpeta del portable: tus datos se conservan.
        </p>
      )}

      {phase === "error" && <Notice tone="error" where="Actualizaciones">{error}</Notice>}
    </Modal>
  );
}
