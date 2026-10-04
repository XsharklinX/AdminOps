// Hoy: todo lo pendiente del técnico en una sola lista.
//
// Lo primero que se ve al abrir AdminOps (que abre en el Panel): el caso
// abierto, los seguimientos que tocan o se pasaron, las visitas de la Agenda de
// hoy, los clientes con el mantenimiento vencido y los avisos de Windows sin
// leer. Ordenado por lo que corre más prisa, con la acción de cada cosa a mano.
// En modo usuario no se enseña: el Panel lo ve el cliente y esto es del técnico.
import { listen } from "@tauri-apps/api/event";
import { CalendarCheck, CalendarClock, Check, Clock, Loader2, Plus, TicketCheck, Trash2 } from "lucide-react";
import { useCallback, useEffect, useState } from "react";
import { logQuietly, agendaApi, alertsApi, casesApi, followupsApi, tomorrowMorning, type Case, type Followup, type Visit, type WindowsAlert } from "../lib/api";
import { CASE_CHANGED_EVENT } from "../lib/currentCase";
import { goToPage } from "../lib/navigate";
import { usePageActive } from "../lib/pageActive";
import { usePrefs } from "../lib/prefs";
import { useToast } from "./feedback";
import { inputClass } from "./ui";

type Tone = "bad" | "warn" | "neon" | "mute";

interface Item {
  key: string;
  tone: Tone;
  source: string;
  title: string;
  detail?: string;
  when: string;
  /** Orden: lo más urgente, antes. */
  rank: number;
  actions?: React.ReactNode;
  onOpen?: () => void;
}

const endOfToday = () => {
  const d = new Date();
  d.setHours(23, 59, 59, 999);
  return Math.floor(d.getTime() / 1000);
};
const startOfToday = () => {
  const d = new Date();
  d.setHours(0, 0, 0, 0);
  return Math.floor(d.getTime() / 1000);
};
const hora = (s: number) => new Date(s * 1000).toLocaleTimeString("es", { hour: "2-digit", minute: "2-digit" });
const dias = (s: number) => Math.floor((startOfToday() - s) / 86_400) + 1;

export function TodayCard() {
  const prefs = usePrefs();
  const active = usePageActive();
  const toast = useToast();
  const [followups, setFollowups] = useState<Followup[] | null>(null);
  const [visits, setVisits] = useState<Visit[]>([]);
  const [dueClients, setDueClients] = useState(0);
  const [openCase, setOpenCase] = useState<Case | null>(null);
  const [alerts, setAlerts] = useState<WindowsAlert[]>([]);
  const [adding, setAdding] = useState(false);

  const load = useCallback(() => {
    const today = [startOfToday(), endOfToday()];
    void followupsApi.list().then(setFollowups).catch(() => setFollowups([]));
    void agendaApi
      .list()
      .then((a) => {
        setVisits(a.visits.filter((v) => v.status === "planned" && v.start >= today[0] && v.start <= today[1]));
        setDueClients(a.due.length);
      })
      .catch(logQuietly("TodayCard"));
    void casesApi.current().then(setOpenCase).catch(logQuietly("TodayCard"));
    void alertsApi
      .list()
      .then((l) => setAlerts(l.filter((x) => !x.read && x.level !== "info")))
      .catch(logQuietly("TodayCard"));
  }, []);

  // Al entrar, al volver a la ventana, cuando otra parte (o la nota) cambia algo, y cada minuto.
  useEffect(() => {
    if (!active || prefs.mode === "user") return;
    load();
    const t = window.setInterval(load, 60_000);
    window.addEventListener("focus", load);
    window.addEventListener(CASE_CHANGED_EVENT, load);
    const offA = listen("followups-changed", load);
    const offB = listen("case-changed", load);
    return () => {
      window.clearInterval(t);
      window.removeEventListener("focus", load);
      window.removeEventListener(CASE_CHANGED_EVENT, load);
      void offA.then((f) => f());
      void offB.then((f) => f());
    };
  }, [active, load, prefs.mode]);

  if (prefs.mode === "user") return null;

  const run = (p: Promise<unknown>, ok?: string) =>
    p.then(() => {
      if (ok) toast("ok", ok);
      load();
    }, (e) => toast("error", String(e)));

  const now = Date.now() / 1000;
  const items: Item[] = [];

  if (openCase) {
    items.push({
      key: "case",
      tone: "warn",
      source: "Caso",
      title: [openCase.ticket, openCase.person].filter(Boolean).join(" · ") || "Caso abierto",
      detail: openCase.notes || undefined,
      when: `desde las ${hora(openCase.started)}`,
      rank: 1,
    });
  }

  for (const f of (followups ?? []).filter((x) => !x.done && x.due <= endOfToday())) {
    const vencido = f.due < startOfToday();
    items.push({
      key: `f-${f.id}`,
      tone: vencido ? "bad" : "warn",
      source: "Seguimiento",
      title: f.text,
      detail: [f.person, f.machine].filter(Boolean).join(" · ") || undefined,
      when: vencido ? (dias(f.due) === 1 ? "ayer" : `hace ${dias(f.due)} días`) : f.due > now ? `a las ${hora(f.due)}` : "hoy",
      rank: vencido ? 0 : 2,
      actions: (
        <>
          <IconBtn title="Hecho" onClick={() => void run(followupsApi.setDone(f.id, true), "Seguimiento hecho.")}>
            <Check size={14} />
          </IconBtn>
          <IconBtn title="Mañana" onClick={() => void run(followupsApi.snooze(f.id, 1), "Aplazado a mañana.")}>
            <Clock size={14} />
          </IconBtn>
          <IconBtn title="Quitar" onClick={() => void run(followupsApi.remove(f.id))} danger>
            <Trash2 size={13} />
          </IconBtn>
        </>
      ),
    });
  }

  for (const v of visits) {
    items.push({
      key: `v-${v.id}`,
      tone: "neon",
      source: "Agenda",
      title: v.title || v.clientName,
      detail: [v.title ? v.clientName : "", v.place, v.machines ? `${v.machines} equipos` : ""].filter(Boolean).join(" · ") || undefined,
      when: hora(v.start),
      rank: v.start < now ? 2 : 3,
      onOpen: () => goToPage("agenda"),
    });
  }

  if (dueClients > 0) {
    items.push({
      key: "due",
      tone: "mute",
      source: "Agenda",
      title: `${dueClients} ${dueClients === 1 ? "cliente necesita" : "clientes necesitan"} mantenimiento`,
      detail: "Vencido o en las próximas dos semanas, sin visita planificada",
      when: "planificar",
      rank: 5,
      onOpen: () => goToPage("agenda"),
    });
  }

  if (alerts.length > 0) {
    const peor = alerts.find((a) => a.level === "bad") ?? alerts[0];
    items.push({
      key: "alerts",
      tone: peor.level === "bad" ? "bad" : "warn",
      source: "Windows",
      title: alerts.length === 1 ? peor.title : `${alerts.length} avisos de Windows sin leer`,
      detail: alerts.length === 1 ? peor.detail : `El más grave: ${peor.title}`,
      when: "ahora",
      rank: peor.level === "bad" ? 1 : 4,
      onOpen: () => goToPage("history"),
    });
  }

  items.sort((a, b) => a.rank - b.rank);

  return (
    <section className="rounded-xl border border-line bg-panel">
      <header className="flex items-center gap-2 px-5 pt-4 pb-3">
        <CalendarCheck size={15} className="text-neon" />
        <h2 className="text-sm font-semibold text-ink">Hoy</h2>
        <span className="text-xs text-mute">{items.length === 0 ? "nada pendiente" : `${items.length} ${items.length === 1 ? "cosa" : "cosas"}`}</span>
        <button onClick={() => setAdding(!adding)} className="ml-auto flex items-center gap-1 rounded-md px-2 py-1 text-xs text-mute hover:bg-panel-2 hover:text-ink">
          <Plus size={13} /> Seguimiento
        </button>
      </header>
      {adding && <AddFollowup onDone={() => { setAdding(false); load(); }} />}
      {followups === null ? (
        <p className="flex items-center gap-2 px-5 pb-4 text-xs text-mute">
          <Loader2 size={12} className="animate-spin" /> Juntando lo pendiente…
        </p>
      ) : items.length === 0 ? (
        <p className="px-5 pb-4 text-xs text-mute">Ni casos, ni seguimientos, ni visitas para hoy. Lo que apuntes con «Seguimiento» o con la nota de llamada (Ctrl+Alt+N) aparecerá aquí cuando toque.</p>
      ) : (
        <ul className="space-y-1.5 px-3 pb-3">
          {items.map((it) => (
            <li key={it.key} className="flex items-center gap-3 overflow-hidden rounded-lg border border-line bg-panel-2 pr-2">
              <span className={`w-1 self-stretch ${{ bad: "bg-bad", warn: "bg-warn", neon: "bg-neon", mute: "bg-line-2" }[it.tone]}`} />
              <span className="w-24 shrink-0 font-mono text-[10.5px] tracking-wide text-mute uppercase">{it.source}</span>
              <button onClick={it.onOpen} disabled={!it.onOpen} className="min-w-0 flex-1 py-2 text-left disabled:cursor-default">
                <span className="block truncate text-[13.5px] text-ink">{it.title}</span>
                {it.detail && <span className="block truncate text-xs text-mute">{it.detail}</span>}
              </button>
              <span className="shrink-0 font-mono text-xs text-dim">{it.when}</span>
              {it.actions && <span className="flex shrink-0 items-center">{it.actions}</span>}
            </li>
          ))}
        </ul>
      )}
    </section>
  );
}

function IconBtn({ title, onClick, danger, children }: { title: string; onClick: () => void; danger?: boolean; children: React.ReactNode }) {
  return (
    <button onClick={onClick} title={title} aria-label={title} className={`rounded-md p-1.5 text-mute hover:bg-panel hover:text-ink ${danger ? "hover:text-bad" : ""}`}>
      {children}
    </button>
  );
}

/** Apuntar un seguimiento: qué y cuándo. */
function AddFollowup({ onDone }: { onDone: () => void }) {
  const [text, setText] = useState("");
  const [days, setDays] = useState(1);
  const [busy, setBusy] = useState(false);
  const toast = useToast();

  const save = async () => {
    if (!text.trim()) return;
    setBusy(true);
    try {
      await followupsApi.add({ text, due: days === 0 ? Math.floor(Date.now() / 1000) + 3600 : tomorrowMorning(days) });
      toast("ok", "Seguimiento apuntado.");
      onDone();
    } catch (e) {
      toast("error", String(e));
      setBusy(false);
    }
  };

  return (
    <form
      className="flex flex-wrap items-center gap-2 border-y border-line/70 bg-void/30 px-5 py-3"
      onSubmit={(e) => {
        e.preventDefault();
        void save();
      }}
    >
      <CalendarClock size={14} className="shrink-0 text-mute" />
      <input autoFocus value={text} onChange={(e) => setText(e.target.value)} placeholder="Volver a mirar el disco de PC-CONTA-07" className={`${inputClass} min-w-48 flex-1`} aria-label="Qué hay que hacer" />
      <select value={days} onChange={(e) => setDays(Number(e.target.value))} className={`${inputClass} w-auto`} aria-label="Cuándo">
        <option value={0}>Dentro de una hora</option>
        <option value={1}>Mañana</option>
        <option value={3}>En 3 días</option>
        <option value={7}>En una semana</option>
        <option value={30}>En un mes</option>
      </select>
      <button type="submit" disabled={busy || !text.trim()} className="flex h-9 items-center gap-1.5 rounded-lg bg-neon px-3 text-[13px] font-medium text-on-neon disabled:opacity-40">
        {busy ? <Loader2 size={14} className="animate-spin" /> : <TicketCheck size={14} />} Apuntar
      </button>
    </form>
  );
}
