import { ArrowLeft, ArrowRight, Check, CheckCircle2, ClipboardCheck, ExternalLink, FileText, FolderOpen, Lightbulb, Mail, Play, Receipt, UserPlus, UserRound, Wand2, X } from "lucide-react";
import { useCallback, useEffect, useRef, useState } from "react";
import { useToast, useConfirm } from "../components/feedback";
import { BillingEditor, SendReportModal, SignaturePad, TemplatePicker } from "../components/service";
import { TaskStatus } from "../components/TaskStatus";
import { Button, Card, ErrorState, inputClass, Loading } from "../components/ui";
import { VisitChanges } from "../components/VisitChanges";
import { contactsApi, diagApi, tweaksApi, workApi, type ActiveSession, type Client, type Contact, type JournalEntry, type Settings, type Solution } from "../lib/api";
import { usePageActive } from "../lib/pageActive";
import { autoDone } from "../lib/visits";
import { SolutionEditor } from "./Knowledge";
import { useLiveEffect } from "../lib/useLiveEffect";

const since = (ts: number) => {
  const m = Math.floor((Date.now() / 1000 - ts) / 60);
  return m < 60 ? `${m} min` : `${Math.floor(m / 60)} h ${m % 60} min`;
};

export function Session({ onSessionChange, focus }: { onSessionChange: (active: boolean) => void; focus?: string | null }) {
  const [session, setSession] = useState<ActiveSession | null | undefined>(undefined);
  const [clients, setClients] = useState<Client[]>([]);
  const [settings, setSettings] = useState<Settings | null>(null);
  const [clientId, setClientId] = useState(focus ?? "");
  // «Empezar» desde la agenda: se elige ese cliente.
  useEffect(() => {
    if (focus) setClientId(focus);
  }, [focus]);
  const [newName, setNewName] = useState("");
  const [visitType, setVisitType] = useState("");
  const [contactId, setContactId] = useState("");
  const [contacts, setContacts] = useState<Contact[]>([]);
  const [solution, setSolution] = useState<Solution | null>(null);
  const [finished, setFinished] = useState<{ problem: string; notes: string; work: string[] } | null>(null);
  const active = usePageActive();
  const [busy, setBusy] = useState<"start" | "finish" | null>(null);
  const [work, setWork] = useState<JournalEntry[]>([]);
  const [report, setReport] = useState<{ path: string; client: Client | null } | null>(null);
  const [step, setStep] = useState<StepId | null>(null);
  const [sending, setSending] = useState(false);
  const [, tick] = useState(0);
  const toast = useToast();
  const { confirm, dialog } = useConfirm();
  const saveTimer = useRef<number | undefined>(undefined);

  // Lo que se está editando manda sobre lo guardado (el guardado va con retraso).
  const sessionRef = useRef<ActiveSession | null | undefined>(undefined);
  useEffect(() => {
    sessionRef.current = session;
  }, [session]);

  const [failed, setFailed] = useState<string | null>(null);
  const load = useCallback(async () => {
    let saved: ActiveSession | null;
    try {
      const [sv, c] = await Promise.all([workApi.session(), workApi.clients()]);
      saved = sv;
      setClients(c);
      setFailed(null);
    } catch (e) {
      setFailed(String(e));
      return;
    }
    onSessionChange(!!saved);
    const local = sessionRef.current;
    const s = saved && local && local.id === saved.id ? local : saved;
    if (s) {
      const [j, snaps] = await Promise.all([tweaksApi.journal().catch(() => []), diagApi.snapshots().catch(() => [])]);
      const mine = j.filter((e) => e.timestamp >= s.started);
      setWork(mine.filter((e) => e.ok && e.op !== "restorePoint"));
      // Checklist que se marca sola con lo que se ha hecho en AdminOps.
      const ts = snaps.map((x) => x.timestamp);
      const marked = s.checklist.map((c) => (!c.done && c.auto && autoDone(c.auto, mine, ts, s.started, s.baseline) ? { ...c, done: true } : c));
      if (marked.some((c, i) => c.done !== s.checklist[i].done)) {
        const next = { ...s, checklist: marked };
        setSession(next);
        workApi.updateSession(next).catch(() => {});
        return;
      }
    }
    setSession(s);
  }, [onSessionChange]);

  useLiveEffect(
    (vigente) => {
      void load();
      workApi.settings().then((s) => vigente() && setSettings(s)).catch(() => {});
      contactsApi
        .list()
        .then((l) => vigente() && setContacts(l.filter((c) => !c.deleted)))
        .catch(() => {});
    },
    [load],
  );

  // Mientras la página está a la vista: reloj y checklist automática al día.
  useEffect(() => {
    if (!active) return;
    void load();
    const t = window.setInterval(() => {
      tick((n) => n + 1);
      void load();
    }, 30000);
    return () => window.clearInterval(t);
  }, [active, load]);

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
      await workApi.startSession(id, visitType || null, contactId || null);
      setNewName("");
      setFinished(null);
      toast("ok", "Sesión iniciada: diagnóstico inicial guardado.");
    } catch (e) {
      toast("error", String(e));
    } finally {
      setBusy(null);
      void load();
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
    setStep(null);
    setBusy("finish");
    const client = clients.find((c) => c.id === final.clientId) ?? null;
    try {
      await workApi.updateSession(final);
      const path = await workApi.finishSession();
      setReport({ path, client });
      setFinished({ problem: final.problem, notes: final.notes, work: work.map((w) => w.title) });
      toast("ok", "Sesión finalizada. Informe guardado en la ficha del cliente.");
    } catch (e) {
      toast("error", String(e));
    } finally {
      setBusy(null);
      void load();
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
    window.clearTimeout(saveTimer.current);
    await workApi.cancelSession().catch((e) => toast("error", String(e)));
    setStep(null);
    void load();
  };

  if (session === undefined) return failed ? <ErrorState page message={failed} onRetry={() => void load()} /> : <Loading page />;

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
            {finished && (finished.problem || finished.notes) && (
              <button
                onClick={() =>
                  setSolution({
                    id: "",
                    title: finished.problem.split("\n")[0].slice(0, 100),
                    problem: finished.problem,
                    solution: [finished.notes, finished.work.length ? `Hecho con AdminOps:\n${finished.work.map((w) => `- ${w}`).join("\n")}` : ""].filter(Boolean).join("\n\n"),
                    tags: [],
                  })
                }
                className="flex items-center gap-1 text-xs text-dim hover:text-ink"
                title="Guardar lo que funcionó en Conocimiento → Soluciones"
              >
                <Lightbulb size={12} /> Guardar como solución
              </button>
            )}
          </div>
        )}
        {solution && <SolutionEditor initial={solution} tags={[]} onClose={() => setSolution(null)} onSaved={() => setFinished(null)} />}
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
          <div className="mb-3 grid grid-cols-2 gap-3">
            <label className="block">
              <span className="mb-1 block text-xs text-dim">Tipo de visita (su checklist)</span>
              <select value={visitType} onChange={(e) => setVisitType(e.target.value)} className={inputClass}>
                <option value="">Checklist general</option>
                {settings?.visitTypes.map((v) => (
                  <option key={v.name} value={v.name}>
                    {v.name} · {v.items.length} puntos
                  </option>
                ))}
              </select>
            </label>
            <label className="block">
              <span className="mb-1 block text-xs text-dim">Quién pidió el trabajo (Contactos)</span>
              <select value={contactId} onChange={(e) => setContactId(e.target.value)} className={inputClass}>
                <option value="">— Nadie en concreto —</option>
                {[...contacts]
                  .sort((a, b) => Number(b.clientId === clientId && !!clientId) - Number(a.clientId === clientId && !!clientId) || a.name.localeCompare(b.name, "es"))
                  .map((c) => (
                    <option key={c.id} value={c.id}>
                      {c.name}
                      {c.company ? ` · ${c.company}` : ""}
                    </option>
                  ))}
              </select>
            </label>
          </div>
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
  const pending = session.checklist.length - done;
  const quote = session.billing.kind === "quote";
  const area = (key: "problem" | "notes" | "recommendations", label: string, placeholder: string, rows = 3) => (
    <label className="block">
      <span className="mb-1 block text-xs text-dim">{label}</span>
      <textarea value={session[key]} onChange={(e) => update({ ...session, [key]: e.target.value })} rows={rows} placeholder={placeholder} className={`${inputClass} resize-y`} />
    </label>
  );

  // Una visita va en este orden, pero se puede saltar a cualquier paso. Se abre
  // por el primero que falta: el motivo si aún no se ha apuntado; si no, el trabajo.
  const current: StepId = step ?? (session.problem.trim() ? "work" : "reason");
  const index = STEPS.findIndex((x) => x.id === current);
  /** Qué lleva cada paso, para verlo sin entrar. */
  const state: Record<StepId, { done: boolean; note: string }> = {
    reason: { done: !!session.problem.trim(), note: session.problem.trim() ? "Apuntado" : "Falta" },
    work: { done: session.checklist.length > 0 && pending === 0, note: session.checklist.length ? `${done}/${session.checklist.length}${work.length ? ` · ${work.length} cambios` : ""}` : `${work.length} cambios` },
    report: { done: !!session.notes.trim(), note: session.notes.trim() ? "Apuntado" : "Falta" },
    billing: { done: session.billing.kind !== "none" && session.billing.lines.length > 0, note: session.billing.kind === "none" ? "Sin cobro" : `${session.billing.lines.length} ${session.billing.lines.length === 1 ? "línea" : "líneas"}` },
    close: { done: !!session.signature, note: session.signature ? "Firmado" : "Sin firmar" },
  };
  const small = "w-24 rounded-md border border-line bg-void/60 px-2 py-1.5 text-sm text-ink outline-none focus:border-neon/50";

  return (
    <div className="mx-auto max-w-5xl space-y-4 p-6">
      <div className="flex flex-wrap items-center gap-4 rounded-xl border border-line bg-panel px-5 py-3">
        <span className="size-2.5 animate-pulse rounded-full bg-ok" />
        <div className="min-w-0 flex-1">
          <div className="font-medium text-ink">{session.clientName}</div>
          <div className="text-xs text-dim">
            Equipo {session.host} · en curso desde hace {since(session.started)}
            {session.visitType && ` · ${session.visitType}`}
          </div>
        </div>
        <label className="flex items-center gap-1.5 text-xs text-dim" title="Quién pidió el trabajo">
          <UserRound size={13} />
          <select value={session.contactId} onChange={(e) => update({ ...session, contactId: e.target.value })} className="max-w-44 rounded-md border border-line bg-void/60 px-2 py-1 text-xs text-ink">
            <option value="">Sin contacto</option>
            {contacts.map((c) => (
              <option key={c.id} value={c.id}>
                {c.name}
              </option>
            ))}
          </select>
        </label>
        <button onClick={() => void cancel()} disabled={busy !== null} className="flex items-center gap-1 text-xs text-mute hover:text-bad">
          <X size={12} /> Descartar
        </button>
      </div>

      {/* Los pasos de la visita, con lo que lleva cada uno. */}
      <ol className="grid grid-cols-5 gap-1.5" aria-label="Pasos de la sesión">
        {STEPS.map((x, i) => {
          const on = x.id === current;
          const st = state[x.id];
          return (
            <li key={x.id}>
              <button
                onClick={() => setStep(x.id)}
                aria-current={on ? "step" : undefined}
                className={`flex w-full items-center gap-2 rounded-lg border px-2.5 py-2 text-left transition-colors ${on ? "border-neon/60 bg-neon/10" : "border-line bg-panel hover:border-line-2"}`}
              >
                <span className={`grid size-6 shrink-0 place-items-center rounded-full text-[11px] font-semibold ${st.done ? "bg-ok/20 text-ok" : on ? "bg-neon/20 text-neon" : "bg-panel-2 text-mute"}`}>
                  {st.done ? <Check size={12} /> : i + 1}
                </span>
                <span className="min-w-0">
                  <span className={`block truncate text-xs font-medium ${on ? "text-ink" : "text-dim"}`}>{x.label}</span>
                  <span className="block truncate text-[10px] text-mute">{st.note}</span>
                </span>
              </button>
            </li>
          );
        })}
      </ol>

      {busy === "finish" && <TaskStatus task="session" active fallback="Finalizando…" cancellable={false} className="justify-center" />}

      {current === "reason" && (
        <Card title="Por qué viene" icon={<ClipboardCheck size={14} />}>
          <div className="space-y-3">
            {area("problem", "Motivo de la visita", "Lo que cuenta el cliente: va lento, no enciende, virus…", 4)}
            {session.clientId && (
              <div className="empty:hidden">
                <VisitChanges clientId={session.clientId} host={session.host} />
              </div>
            )}
          </div>
          <p className="mt-1.5 text-[11px] text-mute">Todo se guarda solo, según escribes. Al empezar ya se guardó el diagnóstico del equipo tal como llegó.</p>
        </Card>
      )}

      {current === "work" && (
        <div className="grid grid-cols-12 gap-4">
          <Card title={`Checklist · ${done}/${session.checklist.length}`} icon={<ClipboardCheck size={14} />} className="col-span-12 lg:col-span-6">
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
                    <span className={`flex-1 text-sm ${c.done ? "text-dim line-through" : "text-ink"}`}>{c.text}</span>
                    {c.auto && (
                      <span title="Se marca sola cuando haces esa tarea con AdminOps">
                        <Wand2 size={12} className={c.done ? "text-ok" : "text-mute"} />
                      </span>
                    )}
                  </label>
                </li>
              ))}
            </ul>
            <p className="mt-2 text-[11px] text-mute">
              <Wand2 size={10} className="mr-1 inline" />
              se marca sola al hacer esa tarea con AdminOps. Los tipos de visita se configuran en Ajustes → Informes.
            </p>
          </Card>

          <Card title={`Hecho con AdminOps en esta sesión · ${work.length}`} className="col-span-12 lg:col-span-6">
            {work.length === 0 ? (
              <p className="text-sm text-mute">Todavía no se ha hecho ningún cambio. Trabaja con normalidad en el resto de AdminOps: todo lo que apliques aparecerá aquí y en el informe.</p>
            ) : (
              <ul className="space-y-1 text-sm">
                {work.map((e) => (
                  <li key={e.id} className="flex items-center gap-2">
                    <CheckCircle2 size={13} className="shrink-0 text-ok" />
                    <span className="truncate text-ink">{e.title}</span>
                    <span className="ml-auto shrink-0 font-mono text-[11px] text-mute">{new Date(e.timestamp * 1000).toLocaleTimeString("es", { timeStyle: "short" })}</span>
                  </li>
                ))}
              </ul>
            )}
          </Card>
        </div>
      )}

      {current === "report" && (
        <Card title="Lo que irá en el informe" icon={<FileText size={14} />}>
          <div className="space-y-3">
            {area("notes", "Observaciones del técnico", "Qué se encontró y qué se hizo, piezas cambiadas…", 5)}
            {area("recommendations", "Recomendaciones", "Cambiar el disco, ampliar memoria, hacer copias de seguridad…", 3)}
          </div>
          <p className="mt-1.5 text-[11px] text-mute">Además de esto, el informe lleva el motivo, la checklist, lo hecho con AdminOps y la comparación del equipo antes y después.</p>
        </Card>
      )}

      {current === "billing" && (
        <Card title="Presupuesto o recibo" icon={<Receipt size={14} />}>
          <BillingEditor billing={session.billing} onChange={(billing) => update({ ...session, billing })} settings={settings} />
        </Card>
      )}

      {current === "close" && (
        <Card title="Firma y cierre" icon={<FileText size={14} />}>
          <div className="space-y-5">
            <section>
              <h4 className="mb-2 text-xs font-medium text-dim">Tipo de informe</h4>
              <TemplatePicker value={session.template} onChange={(template) => update({ ...session, template })} />
            </section>

            <section className="flex flex-wrap gap-6">
              <label className="block">
                <span className="mb-1 block text-xs text-dim">Garantía de la mano de obra</span>
                <span className="flex items-center gap-2 text-sm text-dim">
                  <input
                    type="number"
                    min={0}
                    value={session.laborWarrantyDays}
                    onChange={(e) => update({ ...session, laborWarrantyDays: Math.max(0, Math.round(Number(e.target.value) || 0)) })}
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
                    value={session.maintenanceMonths}
                    onChange={(e) => update({ ...session, maintenanceMonths: Math.max(0, Math.round(Number(e.target.value) || 0)) })}
                    className={small}
                  />
                  meses (0: no recordar)
                </span>
              </label>
            </section>
            {quote && <p className="-mt-3 text-xs text-mute">Es un presupuesto: la garantía se aplica cuando se haga el trabajo, no aparece en este informe.</p>}

            <section>
              <h4 className="mb-2 text-xs font-medium text-dim">Firma del cliente {quote ? "(acepta el presupuesto)" : "(conforme con el servicio)"}</h4>
              <input value={session.signer} onChange={(e) => update({ ...session, signer: e.target.value })} placeholder="Nombre de quien firma" className={`mb-2 max-w-md ${inputClass}`} />
              <SignaturePad value={session.signature} onChange={(signature) => update({ ...session, signature })} />
              <p className="mt-1 text-[11px] text-mute">Opcional. Sin firma, el PDF deja el espacio para firmarlo en papel.</p>
            </section>

            <ul className="space-y-1 text-sm">
              {!session.problem.trim() && <li className="text-warn">No has apuntado el motivo de la visita.</li>}
              {pending > 0 && (
                <li className="text-warn">
                  {pending === 1 ? "Queda 1 punto" : `Quedan ${pending} puntos`} de la checklist sin marcar: {pending === 1 ? "aparecerá como no realizado" : "aparecerán como no realizados"}.
                </li>
              )}
              {!session.notes.trim() && <li className="text-warn">El informe irá sin observaciones del técnico.</li>}
            </ul>
            <div className="flex flex-wrap items-center gap-3 border-t border-line/60 pt-4">
              <p className="min-w-0 flex-1 text-xs text-mute">Se hará el diagnóstico final y se generará el PDF con la comparación antes/después, archivado en la ficha del cliente.</p>
              <Button onClick={() => void finish(session)} disabled={busy !== null}>
                <FileText size={14} /> Finalizar y generar informe
              </Button>
            </div>
          </div>
        </Card>
      )}

      <div className="flex items-center justify-between">
        <Button kind="ghost" onClick={() => setStep(STEPS[index - 1].id)} disabled={index === 0}>
          <ArrowLeft size={14} /> {index > 0 ? STEPS[index - 1].label : "Anterior"}
        </Button>
        {index < STEPS.length - 1 && (
          <Button onClick={() => setStep(STEPS[index + 1].id)}>
            {STEPS[index + 1].label} <ArrowRight size={14} />
          </Button>
        )}
      </div>
      {dialog}
    </div>
  );
}

type StepId = "reason" | "work" | "report" | "billing" | "close";
/** Una visita, de principio a fin. */
const STEPS: { id: StepId; label: string }[] = [
  { id: "reason", label: "Motivo" },
  { id: "work", label: "Trabajo" },
  { id: "report", label: "Informe" },
  { id: "billing", label: "Cobro" },
  { id: "close", label: "Firma y cierre" },
];
