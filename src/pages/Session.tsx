import { CheckCircle2, ClipboardCheck, ExternalLink, FileText, FolderOpen, Play, UserPlus, X } from "lucide-react";
import { useCallback, useEffect, useRef, useState } from "react";
import { useConfirm, useToast } from "../components/feedback";
import { TaskStatus } from "../components/TaskStatus";
import { Card } from "../components/ui";
import { diagApi, tweaksApi, workApi, type ActiveSession, type Client, type JournalEntry } from "../lib/api";

const since = (ts: number) => {
  const m = Math.floor((Date.now() / 1000 - ts) / 60);
  return m < 60 ? `${m} min` : `${Math.floor(m / 60)} h ${m % 60} min`;
};

export function Session({ onSessionChange }: { onSessionChange: (active: boolean) => void }) {
  const [session, setSession] = useState<ActiveSession | null | undefined>(undefined);
  const [clients, setClients] = useState<Client[]>([]);
  const [clientId, setClientId] = useState("");
  const [newName, setNewName] = useState("");
  const [busy, setBusy] = useState<"start" | "finish" | null>(null);
  const [work, setWork] = useState<JournalEntry[]>([]);
  const [report, setReport] = useState<string | null>(null);
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

  // Guardado automático de checklist y notas.
  const update = (next: ActiveSession) => {
    setSession(next);
    window.clearTimeout(saveTimer.current);
    saveTimer.current = window.setTimeout(() => {
      workApi.updateSession(next.checklist, next.notes).catch((e) => toast("error", String(e)));
    }, 500);
  };

  const finish = async () => {
    if (!session) return;
    const pending = session.checklist.filter((c) => !c.done).length;
    const ok = await confirm({
      title: "¿Finalizar la sesión?",
      confirmLabel: "Finalizar y generar informe",
      body: (
        <>
          <p className="mb-2">Se hará el diagnóstico final y se generará el informe PDF con la comparación antes/después.</p>
          {pending > 0 && <p className="text-warn">Quedan {pending} puntos de la checklist sin marcar: aparecerán como no realizados.</p>}
        </>
      ),
    });
    if (!ok) return;
    window.clearTimeout(saveTimer.current);
    setBusy("finish");
    try {
      await workApi.updateSession(session.checklist, session.notes);
      const path = await workApi.finishSession();
      setReport(path);
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
            <span className="min-w-0 flex-1 truncate font-mono text-xs text-dim">{report}</span>
            <button onClick={() => diagApi.openReport(report)} className="flex items-center gap-1 text-xs text-neon hover:underline">
              <ExternalLink size={12} /> Abrir
            </button>
            <button onClick={() => diagApi.revealReport(report)} className="flex items-center gap-1 text-xs text-dim hover:text-ink">
              <FolderOpen size={12} /> Carpeta
            </button>
          </div>
        )}
        <Card title="Nueva sesión de servicio" icon={<ClipboardCheck size={14} />}>
          <p className="mb-4 text-sm text-dim">
            Al empezar se guarda un diagnóstico del equipo tal como llega. Trabaja con normalidad (ajustes, limpieza, reparaciones…) y al finalizar
            se genera el informe con la comparación, el trabajo realizado y la checklist, archivado en la ficha del cliente.
          </p>
          <label className="mb-1 block text-xs text-dim">Cliente</label>
          <select
            value={clientId}
            onChange={(e) => setClientId(e.target.value)}
            className="mb-3 w-full rounded-md border border-line bg-void/60 px-3 py-2 text-sm text-ink outline-none focus:border-neon/50"
          >
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
              <input
                value={newName}
                onChange={(e) => setNewName(e.target.value)}
                placeholder="Nombre del cliente o empresa"
                className="flex-1 rounded-md border border-line bg-void/60 px-3 py-2 text-sm text-ink outline-none placeholder:text-mute focus:border-neon/50"
              />
            </div>
          )}
          <button
            onClick={start}
            disabled={busy !== null}
            className="flex w-full items-center justify-center gap-2 rounded-md border border-neon/50 bg-neon/10 py-2.5 text-sm font-medium text-neon hover:bg-neon/20 disabled:opacity-50"
          >
            <Play size={14} /> Iniciar sesión
          </button>
          <TaskStatus task="session" active={busy === "start"} fallback="Diagnóstico inicial…" cancellable={false} className="mt-3 justify-center" />
        </Card>
      </div>
    );

  const done = session.checklist.filter((c) => c.done).length;

  return (
    <div className="mx-auto grid max-w-6xl grid-cols-12 gap-4 p-6">
      <div className="col-span-12 flex items-center gap-4 rounded-xl border border-ok/30 bg-ok/5 px-5 py-3">
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
        <button
          onClick={finish}
          disabled={busy !== null}
          className="flex items-center gap-1.5 rounded-md border border-neon/60 bg-neon/10 px-4 py-2 text-sm font-medium text-neon hover:bg-neon/20 disabled:opacity-50"
        >
          <FileText size={14} /> Finalizar y generar informe
        </button>
      </div>
      {busy === "finish" && <TaskStatus task="session" active fallback="Finalizando…" cancellable={false} className="col-span-12 justify-center" />}

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
                <span className={`text-sm ${c.done ? "text-dim line-through" : "text-ink"}`}>{c.text}</span>
              </label>
            </li>
          ))}
        </ul>
        <p className="mt-2 text-[11px] text-mute">Los puntos se configuran en Ajustes.</p>
      </Card>

      <Card title="Observaciones para el informe" className="col-span-12 lg:col-span-6">
        <textarea
          value={session.notes}
          onChange={(e) => update({ ...session, notes: e.target.value })}
          rows={10}
          placeholder="Qué problema traía el equipo, qué se hizo, recomendaciones, piezas a reemplazar…"
          className="w-full resize-y rounded-md border border-line bg-void/60 px-3 py-2 text-sm text-ink outline-none placeholder:text-mute focus:border-neon/50"
        />
        <p className="mt-1 text-[11px] text-mute">Se guarda automáticamente.</p>
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
      {dialog}
    </div>
  );
}
