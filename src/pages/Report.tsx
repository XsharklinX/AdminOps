import { ExternalLink, FileText, FolderOpen, Loader2, Mail, Receipt, Zap } from "lucide-react";
import { useEffect, useState } from "react";
import { useToast } from "../components/feedback";
import { BillingEditor, SendReportModal, TemplatePicker } from "../components/service";
import { Button, Card, inputClass, Loading } from "../components/ui";
import { diagApi, EMPTY_BILLING, tweaksApi, workApi, type Billing, type Client, type Settings, type SnapshotInfo, type Template } from "../lib/api";

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
  const [clients, setClients] = useState<Client[]>([]);
  const [settings, setSettings] = useState<Settings | null>(null);
  const [clientId, setClientId] = useState("");
  const [client, setClient] = useState("");
  const [archive, setArchive] = useState(true);
  const [template, setTemplate] = useState<Template>("client");
  const [problem, setProblem] = useState("");
  const [notes, setNotes] = useState("");
  const [recommendations, setRecommendations] = useState("");
  const [billing, setBilling] = useState<Billing>(EMPTY_BILLING);
  const [busy, setBusy] = useState<string | null>(null);
  const [lastPath, setLastPath] = useState<string | null>(null);
  const [sending, setSending] = useState(false);
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
    workApi.clients().then(setClients);
    workApi.settings().then((s) => {
      setSettings(s);
      setTechnician((t) => t || s.technician);
    });
  }, []);

  const picked = clients.find((c) => c.id === clientId) ?? null;
  // Al elegir un cliente se propone el formato de su plantilla.
  useEffect(() => {
    if (picked) setTemplate(picked.report?.template ?? "client");
  }, [picked?.id]);

  // Entrega rápida: lo hecho hoy con AdminOps pasa a las observaciones.
  const todayWork = async () => {
    const today = new Date().setHours(0, 0, 0, 0) / 1000;
    const j = await tweaksApi.journal();
    const done = j.filter((e) => e.timestamp >= today && e.ok && e.op !== "restorePoint" && e.op !== "revert" && !e.reverted).reverse();
    return done.length ? `Trabajo realizado hoy:\n${done.map((e) => `- ${e.title}`).join("\n")}` : "";
  };

  const quickHandover = async () => {
    const work = await todayWork().catch(() => "");
    const merged = [notes.trim(), work].filter(Boolean).join("\n\n");
    setNotes(merged);
    await generate(merged);
  };

  const generate = async (notesOverride?: string) => {
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
      const path = await diagApi.generateReport({
        baseline,
        technician: technician.trim(),
        clientId: picked?.id ?? null,
        client: picked ? picked.name : client.trim(),
        notes: notesOverride ?? notes,
        template,
        billing,
        problem,
        recommendations,
        archive: !!picked && archive,
      });
      setLastPath(path);
      const pdf = path.toLowerCase().endsWith(".pdf");
      toast(pdf ? "ok" : "info", pdf ? "Informe PDF generado." : "No se pudo crear el PDF (falta Microsoft Edge): se guardó como HTML.");
      loadSnapshots();
    } catch (e) {
      toast("error", String(e));
    } finally {
      setBusy(null);
    }
  };

  const text = (label: string, value: string, set: (v: string) => void, placeholder: string, rows = 3) => (
    <label className="block">
      <span className="mb-1 block text-xs text-dim">{label}</span>
      <textarea value={value} onChange={(e) => set(e.target.value)} rows={rows} placeholder={placeholder} className={`${inputClass} resize-y`} />
    </label>
  );

  return (
    <div className="mx-auto grid max-w-6xl grid-cols-12 gap-4 p-6">
      <div className="col-span-12 flex flex-wrap items-center gap-3 rounded-xl border border-neon/30 bg-neon/5 px-4 py-3">
        <Zap size={16} className="shrink-0 text-neon" />
        <p className="min-w-0 flex-1 text-sm text-dim">
          <span className="font-medium text-ink">Entrega rápida:</span> añade a las observaciones todo lo hecho hoy con AdminOps y genera el informe comparando con el primer
          diagnóstico de hoy. Rellena antes el cliente si quieres.
        </p>
        <Button onClick={quickHandover} disabled={busy !== null}>
          <Zap size={13} /> Generar entrega de hoy
        </Button>
      </div>
      <Card title="Datos del informe" icon={<FileText size={14} />} className="col-span-12 lg:col-span-7">
        <div className="space-y-3">
          <div className="grid grid-cols-2 gap-3">
            <label className="block">
              <span className="mb-1 block text-xs text-dim">Técnico</span>
              <input value={technician} onChange={(e) => setTechnician(e.target.value)} placeholder="Tu nombre" className={inputClass} />
            </label>
            <label className="block">
              <span className="mb-1 block text-xs text-dim">Cliente</span>
              <select value={clientId} onChange={(e) => setClientId(e.target.value)} className={inputClass}>
                <option value="">— Sin ficha (escribir nombre) —</option>
                {clients.map((c) => (
                  <option key={c.id} value={c.id}>
                    {c.name}
                  </option>
                ))}
              </select>
            </label>
          </div>
          {picked ? (
            <label className="flex items-center gap-2 text-sm text-dim">
              <input type="checkbox" checked={archive} onChange={(e) => setArchive(e.target.checked)} className="size-4 accent-[var(--color-neon)]" />
              Guardar la visita en la ficha de {picked.name} (con garantía y próximo mantenimiento)
            </label>
          ) : (
            <input value={client} onChange={(e) => setClient(e.target.value)} placeholder="Nombre del cliente o empresa (opcional)" className={inputClass} />
          )}
          <div>
            <span className="mb-1 block text-xs text-dim">Tipo de informe</span>
            <TemplatePicker value={template} onChange={setTemplate} />
          </div>
          {text("Motivo de la visita", problem, setProblem, "Lo que cuenta el cliente: va lento, no enciende, virus…", 2)}
          {text("Observaciones del técnico", notes, setNotes, "Qué se encontró y qué se hizo, piezas cambiadas…", 4)}
          {text("Recomendaciones", recommendations, setRecommendations, "Cambiar el disco, ampliar memoria, hacer copias de seguridad…", 2)}
        </div>
      </Card>

      <Card title="Comparar con (antes)" className="col-span-12 lg:col-span-5">
        <p className="mb-3 text-xs text-dim">
          Cada diagnóstico guarda una foto del equipo. Elige la de antes de trabajar para mostrar al cliente la mejora, los problemas resueltos y el
          trabajo realizado desde entonces.
        </p>
        {snapshots === null ? (
          <Loading />
        ) : snapshots.length === 0 ? (
          <p className="text-xs text-warn">Aún no hay análisis guardados. Ejecuta un diagnóstico antes de empezar a trabajar para tener el "antes".</p>
        ) : (
          <div className="max-h-[430px] space-y-1 overflow-y-auto">
            <label className={`flex cursor-pointer items-center gap-2.5 rounded-md px-2.5 py-2 text-sm ${baseline === null ? "bg-neon/10 text-neon" : "text-dim hover:bg-panel-2"}`}>
              <input type="radio" checked={baseline === null} onChange={() => setBaseline(null)} className="accent-[var(--color-neon)]" />
              Sin comparación (trabajo de hoy)
            </label>
            {snapshots.map((s) => (
              <label
                key={s.timestamp}
                className={`flex cursor-pointer items-center gap-2.5 rounded-md px-2.5 py-2 text-sm ${baseline === s.timestamp ? "bg-neon/10 text-neon" : "text-dim hover:bg-panel-2"}`}
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

      <Card title="Presupuesto o recibo" icon={<Receipt size={14} />} className="col-span-12">
        <BillingEditor billing={billing} onChange={setBilling} settings={settings} />
      </Card>

      <div className="col-span-12 flex flex-wrap items-center justify-end gap-3">
        {lastPath && (
          <div className="mr-auto flex items-center gap-3 text-xs">
            <span className="text-ok">Informe listo.</span>
            <button onClick={() => diagApi.openReport(lastPath).catch((e) => toast("error", String(e)))} className="flex items-center gap-1 text-neon hover:underline">
              <ExternalLink size={12} /> Abrir
            </button>
            <button onClick={() => diagApi.revealReport(lastPath).catch((e) => toast("error", String(e)))} className="flex items-center gap-1 text-dim hover:text-ink">
              <FolderOpen size={12} /> Mostrar en carpeta
            </button>
            <button onClick={() => setSending(true)} className="flex items-center gap-1 text-dim hover:text-ink">
              <Mail size={12} /> Enviar por correo
            </button>
          </div>
        )}
        <Button onClick={() => generate()} disabled={busy !== null}>
          {busy ? <Loader2 size={15} className="animate-spin" /> : <FileText size={15} />}
          {busy ?? "Generar informe PDF"}
        </Button>
      </div>
      {sending && lastPath && <SendReportModal path={lastPath} client={picked ?? (client.trim() ? { name: client.trim() } : null)} onClose={() => setSending(false)} />}
    </div>
  );
}
