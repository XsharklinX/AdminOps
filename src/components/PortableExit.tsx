// Diálogo «Salir sin dejar rastro» para el modo portable.
import { AlertCircle, HardDrive, Loader2, RotateCw, Trash2, Info } from "lucide-react";
import { useEffect, useState } from "react";
import { logQuietly, workApi, type AppInfo } from "../lib/api";
import { Button, Modal } from "./ui";
import { useToast } from "./feedback";
import { useCallback } from "react";

type Leftover = { kind: string; label: string; detail?: string };
type FailedItem = { label: string; error: string };
type CleanupResult = { ok: boolean; removed: string[]; failed: FailedItem[] };

type View = "loading" | "confirm" | "cleaning" | "failed";

export function PortableExit({ appInfo }: { appInfo: AppInfo | null }) {
  const [open, setOpen] = useState(false);
  const [view, setView] = useState<View>("loading");
  const [leftovers, setLeftovers] = useState<Leftover[]>([]);
  const [keepNightJobs, setKeepNightJobs] = useState(true);
  const [cleanupFailed, setCleanupFailed] = useState<FailedItem[]>([]);
  const [cleanupError, setCleanupError] = useState<string | null>(null);
  const toast = useToast();

  const quit = useCallback(() => {
    setOpen(false);
    void workApi.quit().catch(logQuietly("PortableExit"));
  }, []);

  const readLeftovers = useCallback(async () => {
    setView("loading");
    setCleanupFailed([]);
    setCleanupError(null);
    try {
      const list = await workApi.portableLeftovers();
      // Lo que solo es aviso («info») no se puede quitar: si no hay nada más, se sale sin preguntar.
      if (!appInfo?.portable || list.every((l) => l.kind === "info")) {
        quit();
        return;
      }
      setLeftovers(list);
      setView("confirm");
    } catch (e) {
      toast("error", `No se pudo consultar lo que hay que quitar: ${String(e)}. Se cierra de todas formas.`);
      quit();
    }
  }, [appInfo?.portable, quit, toast]);

  const runCleanup = useCallback(async () => {
    setView("cleaning");
    try {
      const result: CleanupResult = await workApi.portableCleanup(keepNightJobs);
      if (result.ok) {
        quit();
        return;
      }
      setCleanupFailed(result.failed);
      setView("failed");
    } catch (e) {
      setCleanupError(String(e));
      setView("failed");
    }
  }, [keepNightJobs, quit]);

  useEffect(() => {
    const onRequest = () => {
      setOpen(true);
      void readLeftovers();
    };
    window.addEventListener("adminops:portable-exit", onRequest);
    return () => window.removeEventListener("adminops:portable-exit", onRequest);
  }, [readLeftovers]);

  if (!open) return null;

  if (view === "loading" || view === "cleaning") {
    return (
      <Modal title="Salir sin dejar rastro" onClose={() => {}} width="w-[360px]" footer={null}>
        <div className="flex items-center gap-3 py-6 text-sm text-ink">
          <Loader2 size={18} className="animate-spin text-neon" />
          {view === "loading" ? "Consultando lo que ha usado esta sesión…" : "Quitando lo de esta sesión…"}
        </div>
      </Modal>
    );
  }

  if (view === "failed") {
    return (
      <Modal
        title="No se pudo quitar todo"
        onClose={() => setView("confirm")}
        width="w-[520px]"
        footer={
          <>
            <Button kind="ghost" onClick={quit}>
              Salir sin quitar
            </Button>
            <Button onClick={() => void runCleanup()}>
              <RotateCw size={14} /> Reintentar
            </Button>
          </>
        }
      >
        <div className="mb-3 flex items-start gap-2 text-sm text-ink">
          <AlertCircle size={16} className="mt-0.5 shrink-0 text-bad" />
          <p>Algunas cosas no se han podido quitar. Puedes reintentarlo o salir igualmente.</p>
        </div>
        {cleanupError && <p className="mb-3 text-sm text-bad">{cleanupError}</p>}
        <div className="space-y-2 rounded-lg border border-line bg-panel-2 p-3">
          {cleanupFailed.map((f, i) => (
            <div key={i} className="text-sm text-ink">
              <p className="font-medium">{f.label}</p>
              <p className="text-xs text-mute">{f.error}</p>
            </div>
          ))}
        </div>
      </Modal>
    );
  }

  return (
    <Modal
      title="Salir sin dejar rastro"
      onClose={() => setOpen(false)}
      width="w-[480px]"
      footer={
        <>
          <Button kind="ghost" onClick={() => setOpen(false)}>
            Cancelar
          </Button>
          <Button kind="ghost" onClick={quit}>
            Salir sin quitar
          </Button>
          <Button onClick={() => void runCleanup()}>
            <Trash2 size={14} /> Quitar y salir
          </Button>
        </>
      }
    >
      <p className="mb-3 text-sm text-dim">
        AdminOps es portable. Al cerrar puede quitar de este equipo lo que ha usado aquí; lo que arreglaste y el diario se quedan en el pendrive.
      </p>
      <div className="mb-4 space-y-2 rounded-lg border border-line bg-panel-2 p-3">
        {leftovers.filter((l) => l.kind !== "info").map((l, i) => (
          <div key={`${l.kind}-${i}`} className="flex items-start gap-2 text-sm text-ink">
            <HardDrive size={14} className="mt-0.5 shrink-0 text-mute" />
            <div>
              <p>{l.label}</p>
              {l.detail && <p className="whitespace-pre-line text-xs text-mute">{l.detail}</p>}
            </div>
          </div>
        ))}
      </div>
      {leftovers
        .filter((l) => l.kind === "info")
        .map((l, i) => (
          <p key={`info-${i}`} className="-mt-2 mb-4 flex items-start gap-2 text-xs text-mute">
            <Info size={13} className="mt-0.5 shrink-0" />
            <span>{l.label}</span>
          </p>
        ))}
      {leftovers.some((l) => l.kind === "task") && (
        <label className="flex items-start gap-2 text-sm text-ink">
          <input
            type="checkbox"
            checked={keepNightJobs}
            onChange={(e) => setKeepNightJobs(e.target.checked)}
            className="mt-1 accent-[var(--color-neon)]"
          />
          <span>
            Mantener la tarea programada de mantenimiento
            <span className="block text-xs text-dim">Si la dejaste a propósito para esta noche, no la quites.</span>
          </span>
        </label>
      )}
    </Modal>
  );
}
