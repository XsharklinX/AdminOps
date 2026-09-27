import { CheckCircle2, ClipboardCheck, ExternalLink, FileText, FolderOpen, Mail, Play, Receipt, UserPlus, X } from "lucide-react";
import { useCallback, useEffect, useRef, useState } from "react";
import { useToast, useConfirm } from "../components/feedback";
import { BillingEditor, SendReportModal, SignaturePad, TemplatePicker } from "../components/service";
import { TaskStatus } from "../components/TaskStatus";
import { Button, Card, inputClass, Modal } from "../components/ui";
import { diagApi, tweaksApi, workApi, type ActiveSession, type Client, type JournalEntry, type Settings } from "../lib/api";

const since = (ts: number) => {
  const m = Math.floor((Date.now() / 1000 - ts) / 60);
  return m < 60 ? `${m} min` : `${Math.floor(m / 60)} h ${m % 60} min`;
};

export function Session({ onSessionChange }: { onSessionChange: (active: boolean) => void }) {
  const [session, setSession] = useState<ActiveSession | null | undefined>(undefined);
  const [clients, setClients] = useState<Client[]>([]);
  const [settings, setSettings] = useState<Settings | null>(null);
  const [clientId, setClientId] = useState("");
  const [newName, setNewName] = useState("");
  const [busy, setBusy] = useState<"start" | "finish" | null>(null);
  const [work, setWork] = useState<JournalEntry[]>([]);
  const [report, setReport] = useState<{ path: string; client: Client | null } | null>(null);
  const [closing, setClosing] = useState(false);
  const [sending, setSending] = useState(false);
  const [, tick] = useState(0);
  const toast = useToast();
  const { confirm, dialog } = useConfirm();
  const saveTimer = useRef<number | undefined>(undefined);

  const load = useCallback(async () => {
    const [s, c] = await Promise.all([workApi.session(), workApi.clients()]);
    setSession(s);
    setClients(c);
    onSessionChange(!!s);
    if (s) {
      const j = await tweaksApi.journal();
      setWork(j.filter((e) => e.timestamp >= s.started && e.ok && e.op !== "restorePoint"));
    }
  }, [onSessionChange]);

  useEffect(() => {
    load();
    workApi.settings().then(setSettings);
    const t = window.setInterval(() => tick((n) => n + 1), 30000);
    return () => window.clearInterval(t);
  }, [load]);

  const start = async () => {
    setBusy("start");
    setReport(null);
    try {
      let id = clientId;
      if (!id) {
        if (!newName.trim()) {
          toast("info", "Elige un cliente o escribe el nombre de uno nuevo.");
          return;
        }
        id = (await workApi.saveClient({ name: newName.trim() })).id;
      }
      await workApi.startSession(id);
      setNewName("");
      toast("ok", "Sesión iniciada: diagnóstico inicial guardado.");
    } catch (e) {
      toast("error", String(e));
    } finally {
      setBusy(null);
      load();
    }
  };

  // Guardado automático de todo lo editable.
  const update = (next: ActiveSession) => {
    setSession(next);
    window.clearTimeout(saveTimer.current);
    saveTimer.current = window.setTimeout(() => {
      workApi.updateSession(next).catch((e) => toast("error", String(e)));
    }, 500);
  };

  const finish = async (final: ActiveSession) => {
    window.clearTimeout(saveTimer.current);
    setClosing(false);
    setBusy("finish");
    const client = clients.find((c) => c.id === final.clientId) ?? null;
    try {
      await workApi.updateSession(final);
      const path = await workApi.finishSession();
      setReport({ path, client });
      toast("ok", "Sesión finalizada. Informe guardado en la ficha del cliente.");
    } catch (e) {
      toast("error", String(e));
    } finally {
      setBusy(null);
      load();
    }
  };

  const cancel = async () => {
    const ok = await confirm({
      title: "¿Descartar la sesión?",
      danger: true,
      confirmLabel: "Descartar",
      body: <p>No se generará informe. Los cambios hechos en el equipo se mantienen (se pueden deshacer desde el Historial).</p>,
    });
    if (!ok) return;
    await workApi.cancelSession();
    load();
  };

  if (session === undefined) return <p className="p-8 font-mono text-sm text-mute">Cargando…</p>;

  if (!session)
    return (
      <div className="mx-auto max-w-3xl p-6">
        {report && (
          <div className="mb-4 flex items-center gap-3 rounded-xl border border-ok/30 bg-ok/5 px-4 py-3 text-sm">
            <CheckCircle2 size={16} className="shrink-0 text-ok" />
            <span className="min-w-0 flex-1 text-ink">Informe generado y archivado en la ficha del cliente.</span>
            <button onClick={() => diagApi.openReport(report.path)} className="flex items-center gap-1 text-xs text-neon hover:underline">
              <ExternalLink size={12} /> Abrir
            </button>
            <button onClick={() => diagApi.revealReport(report.path)} className="flex items-center gap-1 text-xs text-dim hover:text-ink">
              <FolderOpen size={12} /> Carpeta
            </button>
            <button onClick={() => setSending(true)} className="flex items-center gap-1 text-xs text-dim hover:text-ink">
              <Mail size={12} /> Enviar por correo
            </button>
          </div>
        )}
        {sending && report && <SendReportModal path={report.path} client={report.client} onClose={() => setSending(false)} />}
        <Card title="Nueva sesión de servicio" icon={<ClipboardCheck size={14} />}>
          <p className="mb-4 text-sm text-dim">
            Al empezar se guarda un diagnóstico del equipo tal como llega. Trabaja con normalidad (ajustes, limpieza, reparaciones…) y al finalizar
            se genera el informe con la comparación, el trabajo realizado, el presupuesto o recibo y la firma del cliente, archivado en su ficha.
          </p>
          <label className="mb-1 block text-xs text-dim">Cliente</label>
          <select value={clientId} onChange={(e) => setClientId(e.target.value)} className={`mb-3 ${inputClass}`}>
            <option value="">— Cliente nuevo —</option>
            {clients.map((c) => (
              <option key={c.id} value={c.id}>
                {c.name}
                {c.machines.length ? ` · ${c.machines.length} equipo(s)` : ""}
              </option>
            ))}
          </select>
          {!clientId && (
            <div className="mb-3 flex items-center gap-2">
              <UserPlus size={15} className="text-mute" />
              <input value={newName} onChange={(e) => setNewName(e.target.value)} placeholder="Nombre del cliente o empresa" className={inputClass} />
            </div>
          )}
          <div className="flex justify-end">
            <Button onClick={start} disabled={busy !== null}>
              <Play size={14} /> Iniciar sesión
            </Button>
          </div>
          <TaskStatus task="session" active={busy === "start"} fallback="Diagnóstico inicial…" cancellable={false} className="mt-3 justify-center" />
        </Card>
      </div>
    );

  const done = session.checklist.filter((c) => c.done).length;
  const area = (key: "problem" | "notes" | "recommendations", label: string, placeholder: string, rows = 3) => (
    <label className="block">
      <span className="mb-1 block text-xs text-dim">{label}</span>
      <textarea value={session[key]} onChange={(e) => update({ ...session, [key]: e.target.value })} rows={rows} placeholder={placeholder} className={`${inputClass} resize-y`} />
    </label>
  );

  return (
    <div className="mx-auto grid max-w-6xl grid-cols-12 gap-4 p-6">
      <div className="col-span-12 flex items-center gap-4 rounded-xl border border-line bg-panel px-5 py-3">
        <span className="size-2.5 animate-pulse rounded-full bg-ok" />
        <div className="flex-1">
          <div className="font-medium text-ink">{session.clientName}</div>
          <div className="text-xs text-dim">
            Equipo {session.host} · en curso desde hace {since(session.started)}
          </div>
        </div>
        <button onClick={cancel} disabled={busy !== null} className="flex items-center gap-1 text-xs text-mute hover:text-bad">
          <X size={12} /> Descartar
        </button>
        <Button onClick={() => setClosing(true)} disabled={busy !== null}>
          <FileText size={14} /> Finalizar y generar informe
        </Button>
      </div>
      {busy === "finish" && <TaskStatus task="session" active fallback="Finalizando…" cancellable={false} className="col-span-12 justify-center" />}

      <Card title={`Checklist · ${done}/${session.checklist.length}`} icon={<ClipboardCheck size={14} />} className="col-span-12 lg:col-span-5">
        <ul className="space-y-1">
          {session.checklist.map((c, i) => (
            <li key={i}>
              <label className="flex cursor-pointer items-center gap-3 rounded-md px-2 py-1.5 hover:bg-panel-2">
                <input
                  type="checkbox"
                  checked={c.done}
                  onChange={() => update({ ...session, checklist: session.checklist.map((x, j) => (j === i ? { ...x, done: !x.done } : x)) })}
                  className="size-4 accent-[var(--color-neon)]"
                />
                <span className={`text-sm ${c.done ? "text-dim line-through" : "text-ink"}`}>{c.text}</span>
              </label>
            </li>
          ))}
        </ul>
        <p className="mt-2 text-[11px] text-mute">Los puntos se configuran en Ajustes.</p>
      </Card>

      <Card title="Para el informe" className="col-span-12 lg:col-span-7">
        <div className="space-y-3">
          {area("problem", "Motivo de la visita", "Lo que cuenta el cliente: va lento, no enciende, virus…", 2)}
          {area("notes", "Observaciones del técnico", "Qué se encontró y qué se hizo, piezas cambiadas…", 3)}
          {area("recommendations", "Recomendaciones", "Cambiar el disco, ampliar memoria, hacer copias de seguridad…", 2)}
        </div>
        <p className="mt-1.5 text-[11px] text-mute">Se guarda automáticamente.</p>
      </Card>

      <Card title="Presupuesto o recibo" icon={<Receipt size={14} />} className="col-span-12">
        <BillingEditor billing={session.billing} onChange={(billing) => update({ ...session, billing })} settings={settings} />
      </Card>

      <Card title={`Trabajo realizado en esta sesión · ${work.length}`} className="col-span-12">
        {work.length === 0 ? (
          <p className="text-sm text-mute">Todavía no se ha hecho ningún cambio. Todo lo que apliques en AdminOps aparecerá aquí y en el informe.</p>
        ) : (
          <ul className="grid grid-cols-1 gap-x-6 gap-y-1 text-sm md:grid-cols-2">
            {work.map((e) => (
              <li key={e.id} className="flex items-center gap-2">
                <CheckCircle2 size={13} className="shrink-0 text-ok" />
                <span className="truncate text-ink">{e.title}</span>
                <span className="ml-auto shrink-0 font-mono text-[11px] text-mute">
                  {new Date(e.timestamp * 1000).toLocaleTimeString("es", { timeStyle: "short" })}
                </span>
              </li>
            ))}
          </ul>
        )}
      </Card>
      {closing && <FinishDialog session={session} onCancel={() => setClosing(false)} onConfirm={finish} />}
      {dialog}
    </div>
  );
}

/** Último paso: plantilla, garantía, mantenimiento y firma del cliente. */
function FinishDialog({ session, onCancel, onConfirm }: { session: ActiveSession; onCancel: () => void; onConfirm: (s: ActiveSession) => void }) {
  const [s, set] = useState(session);
  const pending = s.checklist.filter((c) => !c.done).length;
  const quote = s.billing.kind === "quote";
  const small = "w-24 rounded-md border border-line bg-void/60 px-2 py-1.5 text-sm text-ink outline-none focus:border-neon/50";
  return (
    <Modal
      title="Finalizar la sesión"
      onClose={onCancel}
      width="w-[620px]"
      footer={
        <>
          <Button kind="ghost" onClick={onCancel}>
            Volver
          </Button>
          <Button onClick={() => onConfirm(s)}>
            <FileText size={14} /> Finalizar y generar informe
          </Button>
        </>
      }
    >
      <div className="space-y-5">
        <section>
          <h4 className="mb-2 text-xs font-medium text-dim">Tipo de informe</h4>
          <TemplatePicker value={s.template} onChange={(template) => set({ ...s, template })} />
        </section>

        <section className="flex flex-wrap gap-6">
          <label className="block">
            <span className="mb-1 block text-xs text-dim">Garantía de la mano de obra</span>
            <span className="flex items-center gap-2 text-sm text-dim">
              <input
                type="number"
                min={0}
                value={s.laborWarrantyDays}
                onChange={(e) => set({ ...s, laborWarrantyDays: Math.max(0, Math.round(Number(e.target.value) || 0)) })}
                className={small}
              />
              días
            </span>
          </label>
          <label className="block">
            <span className="mb-1 block text-xs text-dim">Próximo mantenimiento en</span>
            <span className="flex items-center gap-2 text-sm text-dim">
              <input
                type="number"
                min={0}
                value={s.maintenanceMonths}
                onChange={(e) => set({ ...s, maintenanceMonths: Math.max(0, Math.round(Number(e.target.value) || 0)) })}
                className={small}
              />
              meses (0: no recordar)
            </span>
          </label>
        </section>
        {quote && <p className="-mt-3 text-xs text-mute">Es un presupuesto: la garantía se aplica cuando se haga el trabajo, no aparece en este informe.</p>}

        <section>
          <h4 className="mb-2 text-xs font-medium text-dim">Firma del cliente {quote ? "(acepta el presupuesto)" : "(conforme con el servicio)"}</h4>
          <input value={s.signer} onChange={(e) => set({ ...s, signer: e.target.value })} placeholder="Nombre de quien firma" className={`mb-2 ${inputClass}`} />
          <SignaturePad value={s.signature} onChange={(signature) => set({ ...s, signature })} />
          <p className="mt-1 text-[11px] text-mute">Opcional. Sin firma, el PDF deja el espacio para firmarlo en papel.</p>
        </section>

        {pending > 0 && <p className="text-sm text-warn">Quedan {pending} puntos de la checklist sin marcar: aparecerán como no realizados.</p>}
        <p className="text-xs text-mute">Se hará el diagnóstico final y se generará el PDF con la comparación antes/después.</p>
      </div>
    </Modal>
  );
}
