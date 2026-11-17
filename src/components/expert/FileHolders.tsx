// ¿Quién tiene ese archivo? Elige o suelta un archivo, una carpeta o una unidad
// (el USB que no se deja expulsar) y dice qué programa lo tiene agarrado, y
// quién lo tiene abierto por la red. Mirar no cambia nada; cerrar sí.
import { FileSearch, FolderOpen, HardDrive, Loader2, Usb, XCircle } from "lucide-react";
import { useCallback, useEffect, useRef, useState } from "react";
import { useConfirm, useToast } from "../feedback";
import { Card, EmptyLine, inputClass, smallBtn, softBtn } from "../ui";
import { diagnoseApi, type DriveInfo, type Holder, type HolderReport } from "../../lib/api";
import { useDropped } from "../../lib/dropped";

const KIND: Record<HolderReport["kind"], string> = { file: "Archivo", folder: "Carpeta", drive: "Unidad" };

export function FileHolders() {
  const toast = useToast();
  const { confirm, dialog } = useConfirm();
  const [path, setPath] = useState("");
  const [drives, setDrives] = useState<DriveInfo[]>([]);
  const [report, setReport] = useState<HolderReport | null>(null);
  const [looking, setLooking] = useState(false);
  const [error, setError] = useState<string | null>(null);
  const [busy, setBusy] = useState<string | null>(null);
  // Programas que no se cerraron con educación (preguntan si guardar): se ofrece forzar.
  const [stuck, setStuck] = useState<Record<number, string>>({});
  const seq = useRef(0);

  const look = useCallback(async (p: string) => {
    const n = ++seq.current;
    setLooking(true);
    setError(null);
    try {
      const r = await diagnoseApi.holders(p);
      if (n === seq.current) setReport(r);
    } catch (e) {
      if (n === seq.current) {
        setReport(null);
        setError(String(e));
      }
    } finally {
      if (n === seq.current) setLooking(false);
    }
  }, []);

  useEffect(() => {
    diagnoseApi.drives().then(setDrives, () => setDrives([]));
  }, []);

  // Lo que se suelta sobre la ventana (la pantalla lo recoge al abrirse).
  useDropped("holders", (p) => {
    window.setTimeout(() => document.getElementById("whoholds")?.scrollIntoView({ behavior: "smooth", block: "start" }), 80);
    setPath(p);
    setStuck({});
    void look(p);
  });

  const pick = async (folder: boolean) => {
    try {
      const p = await diagnoseApi.pick(folder);
      if (p) {
        setPath(p);
        setStuck({});
        void look(p);
      }
    } catch (e) {
      toast("error", String(e));
    }
  };

  const closeProgram = async (h: Holder, force: boolean) => {
    const ok = await confirm({
      title: force ? `¿Cerrar ${h.name} a la fuerza?` : `¿Cerrar ${h.name}?`,
      danger: true,
      confirmLabel: force ? "Cerrar a la fuerza" : "Cerrar",
      body: force ? (
        <p>Se cerrará de golpe y se perderá lo que no esté guardado en ese programa.</p>
      ) : (
        <p>Se le pide que se cierre. Si tiene algo sin guardar, preguntará antes: guárdalo ahí. Si no se cierra, podrás forzarlo (y perder lo no guardado).</p>
      ),
    });
    if (!ok) return;
    setBusy(`p${h.pid}`);
    try {
      const r = await diagnoseApi.closeHolder(h.pid, h.exe, force);
      if (r.closed) {
        toast("ok", r.message);
        setStuck((s) => {
          const rest = { ...s };
          delete rest[h.pid];
          return rest;
        });
      } else {
        setStuck((s) => ({ ...s, [h.pid]: r.message }));
        toast("info", r.message);
      }
    } catch (e) {
      toast("error", String(e));
    } finally {
      setBusy(null);
      if (path) void look(path);
    }
  };

  const closeNet = async (id: string, who: string) => {
    const ok = await confirm({
      title: "¿Cerrar la conexión de red?",
      danger: true,
      confirmLabel: "Cerrar la conexión",
      body: <p>{who} dejará de tener ese archivo abierto desde su equipo. Si estaba editándolo, puede perder lo que no haya guardado.</p>,
    });
    if (!ok) return;
    setBusy(`n${id}`);
    try {
      await diagnoseApi.closeSmbFile(id);
      toast("ok", "Conexión cerrada: el archivo queda libre.");
    } catch (e) {
      toast("error", String(e));
    } finally {
      setBusy(null);
      if (path) void look(path);
    }
  };

  const free = report && report.local.length === 0 && report.network.length === 0;

  return (
    <Card id="whoholds" title="¿Quién tiene ese archivo?" icon={<FileSearch size={14} />} className="col-span-12">
      <p className="mb-3 text-xs text-dim">
        «No se puede expulsar el USB», «el archivo está abierto en otro programa», «no deja borrar la carpeta»: elige o arrastra aquí el archivo, la carpeta o la unidad, y te digo quién lo tiene.
      </p>
      <form
        className="flex flex-wrap items-center gap-2"
        onSubmit={(e) => {
          e.preventDefault();
          setStuck({});
          void look(path);
        }}
      >
        <input value={path} onChange={(e) => setPath(e.target.value)} placeholder={String.raw`E:\  o  C:\Carpeta\archivo.xlsx`} className={`${inputClass} max-w-md flex-1 font-mono`} aria-label="Ruta del archivo, carpeta o unidad" />
        <button type="button" onClick={() => void pick(false)} className={smallBtn}>
          <FileSearch size={12} /> Elegir archivo
        </button>
        <button type="button" onClick={() => void pick(true)} className={smallBtn}>
          <FolderOpen size={12} /> Elegir carpeta
        </button>
        <button type="submit" disabled={looking || !path.trim()} className={softBtn}>
          {looking ? <Loader2 size={12} className="animate-spin" /> : <FileSearch size={12} />} Mirar
        </button>
      </form>
      {drives.length > 0 && (
        <div className="mt-2 flex flex-wrap items-center gap-1.5 text-[11px] text-mute">
          Unidades:
          {drives.map((d) => (
            <button
              key={d.letter}
              onClick={() => {
                const p = `${d.letter}:\\`;
                setPath(p);
                setStuck({});
                void look(p);
              }}
              className="inline-flex items-center gap-1 rounded-full border border-line px-2 py-0.5 text-xs text-dim hover:border-neon/40 hover:text-ink"
              title={d.kind === "removable" ? "Unidad extraíble (USB)" : d.kind === "network" ? "Unidad de red" : "Disco del equipo"}
            >
              {d.kind === "removable" ? <Usb size={11} /> : <HardDrive size={11} />} {d.letter}:
            </button>
          ))}
        </div>
      )}

      {error && <p className="mt-3 text-sm text-bad">{error}</p>}

      {report && (
        <div className="mt-4 space-y-3">
          <p className="flex flex-wrap items-baseline gap-2 text-sm">
            <span className="rounded border border-line-2 px-1.5 text-[11px] text-mute">{KIND[report.kind]}</span>
            <span className="min-w-0 break-all font-mono text-xs text-ink select-text">{report.path}</span>
          </p>

          {free && <EmptyLine>Nadie lo tiene agarrado ahora mismo{report.scanned > 0 ? ` (se revisaron ${report.scanned} archivos${report.truncated ? ", no todos: la carpeta es muy grande" : ""})` : ""}. Si Windows sigue sin dejarte, prueba a cerrar las ventanas del Explorador que lo tengan abierto.</EmptyLine>}

          {report.local.length > 0 && (
            <div>
              <h4 className="mb-1 text-xs font-medium text-dim">En este equipo</h4>
              <ul className="space-y-1.5">
                {report.local.map((h) => (
                  <li key={h.pid} className="flex flex-wrap items-center gap-3 rounded-lg border border-line bg-void/40 px-3 py-2">
                    <div className="min-w-0 flex-1">
                      <div className="truncate text-sm text-ink">
                        {h.name} <span className="font-mono text-[11px] text-mute">PID {h.pid}</span>
                      </div>
                      <div className="text-xs text-dim">{stuck[h.pid] ?? h.note}</div>
                    </div>
                    {h.closable && (
                      <>
                        <button onClick={() => void closeProgram(h, false)} disabled={busy !== null} className={smallBtn}>
                          {busy === `p${h.pid}` ? <Loader2 size={12} className="animate-spin" /> : <XCircle size={12} />} Cerrar {h.name}
                        </button>
                        {stuck[h.pid] && (
                          <button onClick={() => void closeProgram(h, true)} disabled={busy !== null} className="inline-flex items-center gap-1.5 rounded-md border border-bad/60 bg-bad/10 px-2.5 py-1.5 text-xs font-medium text-bad hover:bg-bad/20 disabled:opacity-40">
                            Cerrar a la fuerza
                          </button>
                        )}
                      </>
                    )}
                  </li>
                ))}
              </ul>
              {report.truncated && <p className="mt-1 text-[11px] text-mute">La carpeta tiene muchos archivos: se revisaron {report.scanned}. Puede haber algún programa más.</p>}
            </div>
          )}

          {report.network.length > 0 && (
            <div>
              <h4 className="mb-1 text-xs font-medium text-dim">Por la red</h4>
              <ul className="space-y-1.5">
                {report.network.map((n) => (
                  <li key={n.fileId} className="flex flex-wrap items-center gap-3 rounded-lg border border-line bg-void/40 px-3 py-2">
                    <div className="min-w-0 flex-1">
                      <div className="text-sm text-ink">
                        {n.user || "Alguien"} <span className="text-dim">desde</span> {n.client || "otro equipo"}
                      </div>
                      <div className="truncate font-mono text-[11px] text-mute select-text">{n.path}</div>
                    </div>
                    <button onClick={() => void closeNet(n.fileId, n.user || "Esa persona")} disabled={busy !== null} className={smallBtn}>
                      {busy === `n${n.fileId}` ? <Loader2 size={12} className="animate-spin" /> : <XCircle size={12} />} Cerrar su conexión
                    </button>
                  </li>
                ))}
              </ul>
            </div>
          )}
          {report.netNote && <p className="text-[11px] text-mute">{report.netNote}</p>}
        </div>
      )}
      {dialog}
    </Card>
  );
}
