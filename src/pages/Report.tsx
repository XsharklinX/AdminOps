import { ExternalLink, FileText, FolderOpen, Loader2 } from "lucide-react";
import { useEffect, useState } from "react";
import { useToast } from "../components/feedback";
import { Card } from "../components/ui";
import { diagApi, type SnapshotInfo } from "../lib/api";

const TECH_KEY = "adminops.technician";

function readTech() {
  try {
    return localStorage.getItem(TECH_KEY) ?? "";
  } catch {
    return "";
  }
}

const when = (ts: number) => new Date(ts * 1000).toLocaleString("es", { dateStyle: "medium", timeStyle: "medium" });

export function Report() {
  const [snapshots, setSnapshots] = useState<SnapshotInfo[] | null>(null);
  const [baseline, setBaseline] = useState<number | null>(null);
  const [technician, setTechnician] = useState(readTech);
  const [client, setClient] = useState("");
  const [notes, setNotes] = useState("");
  const [busy, setBusy] = useState<string | null>(null);
  const [lastPath, setLastPath] = useState<string | null>(null);
  const toast = useToast();

  const loadSnapshots = () =>
    diagApi.snapshots().then((s) => {
      setSnapshots(s);
      // Por defecto, el primer análisis de hoy como "antes".
      const today = new Date().setHours(0, 0, 0, 0) / 1000;
      const first = [...s].reverse().find((x) => x.timestamp >= today);
      setBaseline((b) => b ?? (first && first.timestamp !== s[0]?.timestamp ? first.timestamp : null));
    });

  useEffect(() => {
    loadSnapshots();
  }, []);

  const generate = async () => {
    try {
      localStorage.setItem(TECH_KEY, technician);
    } catch {
      /* sin almacenamiento: no pasa nada */
    }
    setBusy("Analizando el estado actual…");
    try {
      // El informe usa un análisis recién hecho como "después".
      await diagApi.run();
      setBusy("Generando informe…");
      const path = await diagApi.generateReport(baseline, technician.trim(), client.trim(), notes);
      setLastPath(path);
      toast(
        path.toLowerCase().endsWith(".pdf") ? "ok" : "info",
        path.toLowerCase().endsWith(".pdf")
          ? "Informe PDF generado."
          : "No se pudo crear el PDF (falta Microsoft Edge): se guardó como HTML.",
      );
      loadSnapshots();
    } catch (e) {
      toast("error", String(e));
    } finally {
      setBusy(null);
    }
  };

  const input = "w-full rounded-md border border-line bg-void/60 px-3 py-2 text-sm text-ink outline-none placeholder:text-mute focus:border-neon/50";

  return (
    <div className="mx-auto grid max-w-5xl grid-cols-12 gap-4 p-6">
      <Card title="Datos del informe" icon={<FileText size={14} />} className="col-span-12 lg:col-span-7">
        <div className="space-y-3">
          <div className="grid grid-cols-2 gap-3">
            <label className="block">
              <span className="mb-1 block text-xs text-dim">Técnico</span>
              <input value={technician} onChange={(e) => setTechnician(e.target.value)} placeholder="Tu nombre" className={input} />
            </label>
            <label className="block">
              <span className="mb-1 block text-xs text-dim">Cliente</span>
              <input value={client} onChange={(e) => setClient(e.target.value)} placeholder="Nombre o empresa" className={input} />
            </label>
          </div>
          <label className="block">
            <span className="mb-1 block text-xs text-dim">Observaciones</span>
            <textarea
              value={notes}
              onChange={(e) => setNotes(e.target.value)}
              rows={6}
              placeholder="Trabajo realizado, recomendaciones, piezas a reemplazar…"
              className={`${input} resize-y`}
            />
          </label>
          <button
            onClick={generate}
            disabled={busy !== null}
            className="flex w-full items-center justify-center gap-2 rounded-md border border-neon/50 bg-neon/10 py-2.5 text-sm font-medium text-neon transition-colors hover:bg-neon/20 disabled:opacity-50"
          >
            {busy ? <Loader2 size={15} className="animate-spin" /> : <FileText size={15} />}
            {busy ?? "Generar informe PDF"}
          </button>
          {lastPath && (
            <div className="rounded-lg border border-line bg-void/40 px-3 py-2.5">
              <p className="mb-2 font-mono text-[11px] break-all text-dim select-text">{lastPath}</p>
              <div className="flex gap-3 text-xs">
                <button onClick={() => diagApi.openReport(lastPath).catch((e) => toast("error", String(e)))} className="flex items-center gap-1 text-neon hover:underline">
                  <ExternalLink size={12} /> Abrir
                </button>
                <button onClick={() => diagApi.revealReport(lastPath).catch((e) => toast("error", String(e)))} className="flex items-center gap-1 text-dim hover:text-ink">
                  <FolderOpen size={12} /> Mostrar en carpeta
                </button>
              </div>
            </div>
          )}
        </div>
      </Card>

      <Card title="Comparar con (antes)" className="col-span-12 lg:col-span-5">
        <p className="mb-3 text-xs text-dim">
          Cada diagnóstico guarda una foto del equipo. Elige la de antes de trabajar para mostrar al cliente la mejora y
          el trabajo realizado desde entonces.
        </p>
        {snapshots === null ? (
          <p className="font-mono text-xs text-mute">Cargando…</p>
        ) : snapshots.length === 0 ? (
          <p className="text-xs text-warn">
            Aún no hay análisis guardados. Ejecuta un diagnóstico antes de empezar a trabajar para tener el "antes".
          </p>
        ) : (
          <div className="max-h-96 space-y-1 overflow-y-auto">
            <label className={`flex cursor-pointer items-center gap-2.5 rounded-md px-2.5 py-2 text-sm ${baseline === null ? "bg-neon/10 text-neon" : "text-dim hover:bg-panel-2"}`}>
              <input type="radio" checked={baseline === null} onChange={() => setBaseline(null)} className="accent-[var(--color-neon)]" />
              Sin comparación (trabajo de hoy)
            </label>
            {snapshots.map((s) => (
              <label
                key={s.timestamp}
                className={`flex cursor-pointer items-center gap-2.5 rounded-md px-2.5 py-2 text-sm ${
                  baseline === s.timestamp ? "bg-neon/10 text-neon" : "text-dim hover:bg-panel-2"
                }`}
              >
                <input type="radio" checked={baseline === s.timestamp} onChange={() => setBaseline(s.timestamp)} className="accent-[var(--color-neon)]" />
                <span className="flex-1">{when(s.timestamp)}</span>
                <span className="font-mono text-[11px]">
                  <span className="text-bad">{s.bad}</span> · <span className="text-warn">{s.warn}</span>
                </span>
              </label>
            ))}
          </div>
        )}
      </Card>
    </div>
  );
}
