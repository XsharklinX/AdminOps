// Personas: la ficha de alguien del dominio.
//
// La mayoría de los tickets empiezan con una persona, no con un equipo. Aquí se
// busca por nombre, usuario, correo o extensión y se hace lo de todos los días
// sin abrir la consola de Active Directory: ver si la cuenta está bloqueada o
// caducada, desbloquearla, darle una contraseña temporal, y las contraseñas de
// su equipo (LAPS y recuperación de BitLocker). Todo con los permisos del propio
// técnico: si el dominio no le deja, se lo dice.
import {
  Briefcase,
  Check,
  Copy,
  Eye,
  EyeOff,
  FolderLock,
  KeyRound,
  Loader2,
  LockOpen,
  MessagesSquare,
  Monitor,
  Search,
  ShieldAlert,
  TicketPlus,
  TriangleAlert,
  UserRound,
  UserX,
} from "lucide-react";
import { useEffect, useRef, useState } from "react";
import { useToast } from "../components/feedback";
import { Button, Card, EmptyState, inputClass, Modal } from "../components/ui";
import { casesApi, contactsApi, officeApi, peopleApi, portalsApi, type Case, type LapsPassword, type Person, type PersonHit, type RecoveryKey } from "../lib/api";
import { openCase } from "../lib/currentCase";
import { Avatar } from "../components/contacts/Avatar";
import { useLiveEffect } from "../lib/useLiveEffect";

const DAY = 86_400;
const fecha = (s: number) => (s ? new Date(s * 1000).toLocaleDateString("es", { day: "numeric", month: "short", year: "numeric" }) : "—");
const hace = (s: number) => {
  if (!s) return "—";
  const d = Math.floor((Date.now() / 1000 - s) / DAY);
  return d <= 0 ? "hoy" : d === 1 ? "ayer" : `hace ${d} días`;
};

/** Copia al portapapeles y lo dice. */
function useCopy() {
  const toast = useToast();
  return (text: string, what: string) =>
    navigator.clipboard.writeText(text).then(
      () => toast("ok", `${what} copiada.`),
      () => toast("error", "No se pudo copiar."),
    );
}

/** Las últimas personas abiertas: la mayoría de las veces se vuelve a las mismas. */
const RECENT_KEY = "adminops.people.recent";
interface Recent {
  sam: string;
  name: string;
  detail: string;
}
function readRecent(): Recent[] {
  try {
    const v: unknown = JSON.parse(localStorage.getItem(RECENT_KEY) ?? "[]");
    return Array.isArray(v) ? (v as Recent[]).filter((x) => x && typeof x.sam === "string").slice(0, 8) : [];
  } catch {
    return [];
  }
}
function pushRecent(r: Recent): Recent[] {
  const next = [r, ...readRecent().filter((x) => x.sam.toLowerCase() !== r.sam.toLowerCase())].slice(0, 8);
  try {
    localStorage.setItem(RECENT_KEY, JSON.stringify(next));
  } catch {
    /* sin almacenamiento */
  }
  return next;
}

export function People({ focus }: { focus: string | null }) {
  // Desde Ctrl+K llega o lo que buscar, o «pc:NOMBRE» para ver las contraseñas de un equipo.
  const pc = focus?.startsWith("pc:") ? focus.slice(3) : null;
  const [query, setQuery] = useState(pc ? "" : (focus ?? ""));
  const [hits, setHits] = useState<PersonHit[] | null>(null);
  const [searching, setSearching] = useState(false);
  const [error, setError] = useState<string | null>(null);
  const [selected, setSelected] = useState<string | null>(null);
  const [recent, setRecent] = useState<Recent[]>(readRecent);
  const input = useRef<HTMLInputElement>(null);

  const search = async (q = query) => {
    if (q.trim().length < 2) return;
    setSearching(true);
    setError(null);
    try {
      const r = await peopleApi.search(q);
      setHits(r);
      // Un solo resultado: se abre directamente.
      setSelected(r.length === 1 ? r[0].sam : null);
    } catch (e) {
      setError(String(e));
      setHits(null);
    } finally {
      setSearching(false);
    }
  };

  // Llegar desde Ctrl+K con algo escrito: se busca directamente.
  useEffect(() => {
    if (pc) return;
    if (focus && focus.trim().length >= 2) {
      setQuery(focus);
      void search(focus);
    } else input.current?.focus();
    // eslint-disable-next-line react-hooks/exhaustive-deps -- solo cuando llega un foco nuevo
  }, [focus]);

  // `@container`: las columnas dependen del hueco que tiene la página, no del
  // ancho de la ventana. Así también se lee bien en media pantalla, al lado del
  // ticket (pantalla dividida).
  return (
    <div className="@container">
    <div className="mx-auto grid max-w-6xl grid-cols-12 gap-4 p-6">
      <div className="col-span-12 @3xl:col-span-4">
        <form
          className="relative"
          onSubmit={(e) => {
            e.preventDefault();
            void search();
          }}
        >
          <Search size={15} className="pointer-events-none absolute top-1/2 left-3 -translate-y-1/2 text-mute" />
          <input
            ref={input}
            value={query}
            onChange={(e) => setQuery(e.target.value)}
            placeholder="Nombre, usuario, correo o extensión"
            className={`${inputClass} py-2.5 pr-24 pl-9`}
            aria-label="Buscar una persona"
          />
          <button
            type="submit"
            disabled={searching || query.trim().length < 2}
            className="absolute top-1/2 right-1.5 flex -translate-y-1/2 items-center gap-1 rounded-md bg-neon/15 px-2.5 py-1.5 text-xs font-medium text-neon transition-colors hover:bg-neon/25 disabled:opacity-40"
          >
            {searching ? <Loader2 size={13} className="animate-spin" /> : null} Buscar
          </button>
        </form>
        {error && <p className="mt-3 flex items-start gap-2 text-xs text-bad"><TriangleAlert size={13} className="mt-0.5 shrink-0" />{error}</p>}
        {hits && hits.length === 0 && <p className="mt-4 text-sm text-mute">Nadie en el dominio coincide con «{query}».</p>}
        {hits && hits.length > 0 && (
          <>
            <p className="mt-3 mb-1.5 text-[11px] text-mute">
              {hits.length} {hits.length === 1 ? "resultado" : "resultados"}
            </p>
            <ul className="space-y-1">
              {hits.map((h) => (
                <li key={h.sam}>
                  <button
                    onClick={() => setSelected(h.sam)}
                    className={`flex w-full items-center gap-2.5 rounded-lg border px-3 py-2 text-left transition-colors ${
                      selected === h.sam ? "border-neon/50 bg-neon/10" : "border-line bg-panel hover:border-line-2"
                    }`}
                  >
                    <Avatar c={{ name: h.name || h.sam, favorite: false }} size={32} />
                    <span className="min-w-0 flex-1">
                      <span className="block truncate text-sm text-ink">{h.name || h.sam}</span>
                      <span className="block truncate text-xs text-mute">
                        {h.sam}
                        {h.department && ` · ${h.department}`}
                        {h.extension && ` · ext. ${h.extension}`}
                      </span>
                    </span>
                    {h.disabled && <span className="shrink-0 rounded border border-line px-1.5 text-[10px] text-mute">desactivada</span>}
                    {!h.disabled && h.lockedHint && <span className="shrink-0 rounded border border-warn/50 px-1.5 text-[10px] text-warn">bloqueo</span>}
                  </button>
                </li>
              ))}
            </ul>
          </>
        )}
        {!hits && !error && recent.length > 0 && (
          <>
            <p className="mt-4 mb-1.5 flex items-center justify-between text-[11px] font-medium tracking-wide text-mute uppercase">
              Recientes
              <button
                onClick={() => {
                  try {
                    localStorage.removeItem(RECENT_KEY);
                  } catch {
                    /* sin almacenamiento */
                  }
                  setRecent([]);
                }}
                className="font-normal tracking-normal normal-case hover:text-ink"
              >
                Olvidar
              </button>
            </p>
            <ul className="space-y-1">
              {recent.map((r) => (
                <li key={r.sam}>
                  <button
                    onClick={() => setSelected(r.sam)}
                    className={`flex w-full items-center gap-2.5 rounded-lg border px-3 py-2 text-left transition-colors ${selected === r.sam ? "border-neon/50 bg-neon/10" : "border-line bg-panel hover:border-line-2"}`}
                  >
                    <Avatar c={{ name: r.name || r.sam, favorite: false }} size={30} />
                    <span className="min-w-0 flex-1">
                      <span className="block truncate text-sm text-ink">{r.name || r.sam}</span>
                      <span className="block truncate text-xs text-mute">{r.detail || r.sam}</span>
                    </span>
                  </button>
                </li>
              ))}
            </ul>
          </>
        )}
        {!hits && !error && recent.length === 0 && (
          <p className="mt-4 text-xs text-mute">
            Busca en el dominio de la empresa con tu cuenta de Windows. Lo que puedas ver y cambiar es lo mismo que en la consola de Active Directory.
          </p>
        )}
        {hits && (
          <button
            onClick={() => {
              setHits(null);
              setQuery("");
              input.current?.focus();
            }}
            className="mt-3 text-xs text-mute hover:text-ink"
          >
            Limpiar la búsqueda
          </button>
        )}
        <ComputerSecrets initial={pc} />
      </div>

      <div className="col-span-12 @3xl:col-span-8">
        {selected ? (
          <PersonCard key={selected} sam={selected} onLoaded={(p) => setRecent(pushRecent({ sam: p.sam, name: p.name, detail: [p.department, p.extension && `ext. ${p.extension}`].filter(Boolean).join(" · ") }))} />
        ) : (
          <EmptyState icon={<UserRound size={28} />} title="Elige a una persona">
            Su cuenta del dominio, sus equipos y lo que ya se hizo con ella, con desbloquear y restablecer la contraseña a un clic.
          </EmptyState>
        )}
      </div>
    </div>
    </div>
  );
}

function PersonCard({ sam, onLoaded }: { sam: string; onLoaded?: (p: Person) => void }) {
  // La última versión del aviso al padre, sin que el efecto de carga dependa de ella.
  const loaded = useRef(onLoaded);
  useEffect(() => {
    loaded.current = onLoaded;
  }, [onLoaded]);
  const [p, setP] = useState<Person | null>(null);
  const [error, setError] = useState<string | null>(null);
  const [history, setHistory] = useState<Case[]>([]);
  const [busy, setBusy] = useState(false);
  const [resetting, setResetting] = useState(false);
  const toast = useToast();

  const load = () =>
    peopleApi
      .details(sam)
      .then((d) => {
        setP(d);
        setError(null);
        return d;
      })
      .catch((e) => {
        setError(String(e));
        return null;
      });

  useLiveEffect(
    (vigente) => {
      void peopleApi
        .details(sam)
        .then((d) => {
          if (!vigente()) return;
          setP(d);
          loaded.current?.(d);
          void casesApi.forPerson(d.sam, d.name).then((h) => vigente() && setHistory(h)).catch(() => {});
        })
        .catch((e) => vigente() && setError(String(e)));
    },
    [sam],
  );

  if (error) return <Card><p className="flex items-start gap-2 text-sm text-bad"><TriangleAlert size={14} className="mt-0.5 shrink-0" />{error}</p></Card>;
  if (!p) return <Card><p className="flex items-center gap-2 text-sm text-mute"><Loader2 size={14} className="animate-spin" /> Leyendo su cuenta del dominio…</p></Card>;

  const now = Date.now() / 1000;
  const expiraEn = p.passwordExpires ? Math.ceil((p.passwordExpires - now) / DAY) : null;

  const unlock = async () => {
    setBusy(true);
    try {
      await peopleApi.unlock(p.sam);
      toast("ok", `Cuenta de ${p.name || p.sam} desbloqueada.`);
      await load();
    } catch (e) {
      toast("error", String(e));
    } finally {
      setBusy(false);
    }
  };

  const teams = async () => {
    if (!p.mail) return;
    try {
      const inApp = await portalsApi.teams(p.mail, false);
      if (!inApp) await contactsApi.teams(p.mail, false);
    } catch (e) {
      toast("error", String(e));
    }
  };

  const problem = p.disabled
    ? { tone: "border-line bg-panel-2 text-dim", text: "La cuenta está desactivada: no puede iniciar sesión hasta que se vuelva a activar en el dominio.", action: null }
    : p.locked
      ? { tone: "border-bad/40 bg-bad/10 text-bad", text: "La cuenta está bloqueada por demasiados intentos con la contraseña equivocada.", action: "unlock" as const }
      : p.passwordExpired
        ? { tone: "border-bad/40 bg-bad/10 text-bad", text: "La contraseña ha caducado: no puede entrar hasta cambiarla.", action: "reset" as const }
        : expiraEn !== null && expiraEn <= 7
          ? { tone: "border-warn/40 bg-warn/10 text-warn", text: `La contraseña caduca en ${expiraEn} ${expiraEn === 1 ? "día" : "días"}: conviene que la cambie ya.`, action: null }
          : null;

  return (
    <div className="space-y-4">
      {problem && (
        <div className={`flex flex-wrap items-center gap-3 rounded-xl border px-4 py-3 text-sm ${problem.tone}`}>
          <ShieldAlert size={16} className="shrink-0" />
          <span className="min-w-0 flex-1">{problem.text}</span>
          {problem.action === "unlock" && (
            <Button onClick={() => void unlock()} disabled={busy}>
              {busy ? <Loader2 size={14} className="animate-spin" /> : <LockOpen size={14} />} Desbloquear
            </Button>
          )}
          {problem.action === "reset" && (
            <Button onClick={() => setResetting(true)} disabled={busy}>
              <KeyRound size={14} /> Dar una contraseña temporal
            </Button>
          )}
        </div>
      )}
      <Card>
        <div className="flex flex-wrap items-center gap-3">
          <Avatar c={{ name: p.name || p.sam, favorite: false }} size={44} />
          <div className="min-w-0 flex-1">
            <div className="text-base font-semibold text-ink">{p.name || p.sam}</div>
            <div className="truncate font-mono text-xs text-mute select-text">{p.sam}</div>
          </div>
          <div className="flex flex-wrap gap-1.5">
            {p.disabled && <Pill tone="mute" icon={<UserX size={11} />}>Cuenta desactivada</Pill>}
            {p.locked && <Pill tone="bad" icon={<ShieldAlert size={11} />}>Cuenta bloqueada</Pill>}
            {p.passwordExpired && <Pill tone="bad">Contraseña caducada</Pill>}
            {!p.passwordExpired && expiraEn !== null && expiraEn <= 7 && <Pill tone="warn">La contraseña caduca en {expiraEn} {expiraEn === 1 ? "día" : "días"}</Pill>}
            {!p.disabled && !p.locked && !p.passwordExpired && (expiraEn === null || expiraEn > 7) && <Pill tone="ok">Cuenta en orden</Pill>}
          </div>
        </div>

        <dl className="mt-4 grid grid-cols-[auto_minmax(0,1fr)] gap-x-5 gap-y-1.5 text-sm @2xl:grid-cols-[auto_minmax(0,1fr)_auto_minmax(0,1fr)]">
          <Kv k="Departamento" v={[p.department, p.office].filter(Boolean).join(" · ")} />
          <Kv k="Cargo" v={p.title} />
          <Kv k="Extensión" v={p.extension} mono />
          <Kv k="Teléfono" v={[p.phone, p.mobile].filter(Boolean).join(" · ")} mono />
          <Kv k="Correo" v={p.mail} />
          <Kv k="Responsable" v={p.manager} />
          <Kv k="Último inicio" v={p.lastLogon ? `${fecha(p.lastLogon)} (${hace(p.lastLogon)})` : ""} />
          <Kv k="Contraseña" v={p.passwordNeverExpires ? "No caduca" : p.passwordLastSet ? `cambiada ${hace(p.passwordLastSet)}${p.passwordExpires ? `, caduca el ${fecha(p.passwordExpires)}` : ""}` : ""} />
        </dl>

        <div className="mt-4 flex flex-wrap gap-2">
          {p.locked && (
            <Button onClick={() => void unlock()} disabled={busy}>
              {busy ? <Loader2 size={14} className="animate-spin" /> : <LockOpen size={14} />} Desbloquear
            </Button>
          )}
          <Button kind={p.locked ? "ghost" : "primary"} onClick={() => setResetting(true)} disabled={busy || p.disabled}>
            <KeyRound size={14} /> Restablecer contraseña
          </Button>
          <Button kind="ghost" onClick={() => openCase({ person: p.name || p.sam, sam: p.sam, machine: p.signedInOn[0] ?? "" })}>
            <TicketPlus size={14} /> Abrir caso
          </Button>
          {p.mail && (
            <Button kind="ghost" onClick={() => void teams()}>
              <MessagesSquare size={14} /> Teams
            </Button>
          )}
        </div>
        <p className="mt-2 text-[11px] text-mute">Lo que cambia la cuenta queda en el diario y el modo auditoría lo bloquea.</p>

        {p.groups.length > 0 && (
          <details className="mt-3">
            <summary className="cursor-pointer text-xs text-dim hover:text-ink">Grupos ({p.groups.length})</summary>
            <div className="mt-2 flex flex-wrap gap-1.5">
              {p.groups.map((g) => (
                <span key={g} className="rounded border border-line bg-panel-2 px-1.5 py-0.5 text-[11px] text-dim">
                  {g}
                </span>
              ))}
            </div>
          </details>
        )}
      </Card>

      <div className="grid grid-cols-1 gap-4 @2xl:grid-cols-2">
        <Card title="Sus equipos" icon={<Monitor size={14} />}>
          {p.signedInOn.length === 0 ? (
            <p className="text-xs text-mute">
              No se sabe en qué equipo está. Comprueba los puestos <strong>a fondo</strong> (Administración → Puestos) y aparecerá aquí el equipo donde tenga la sesión abierta.
            </p>
          ) : (
            <ul className="space-y-2">
              {p.signedInOn.map((h) => (
                <li key={h} className="rounded-lg border border-line bg-panel-2 px-3 py-2">
                  <div className="flex items-center gap-2">
                    <span className="min-w-0 flex-1 truncate font-mono text-sm text-ink select-text">{h}</span>
                    <span className="text-[11px] text-ok">sesión abierta</span>
                  </div>
                  <div className="mt-1.5 flex flex-wrap gap-1">
                    <Button kind="ghost" onClick={() => void officeApi.rdp(h).catch((e) => toast("error", String(e)))}>
                      Conectar
                    </Button>
                  </div>
                  <MachineSecrets computer={h} />
                </li>
              ))}
            </ul>
          )}
        </Card>

        <Card title="Lo que ya se hizo con esta persona" icon={<Briefcase size={14} />}>
          {history.length === 0 ? (
            <p className="text-xs text-mute">Ningún caso cerrado todavía. Al cerrar uno con ella, aparecerá aquí.</p>
          ) : (
            <ul className="space-y-1.5">
              {history.map((c) => (
                <li key={c.id} className="flex items-center gap-2 text-sm">
                  <span className="min-w-0 flex-1 truncate text-dim" title={c.resolution}>
                    {c.ticket ? `${c.ticket} · ` : ""}
                    {c.resolution.split("\n").find((l) => l.startsWith("• "))?.slice(2) ?? "Sin cambios en el equipo"}
                  </span>
                  <span className="shrink-0 text-[11px] text-mute">{fecha(c.started)}</span>
                </li>
              ))}
            </ul>
          )}
        </Card>
      </div>


      {resetting && (
        <ResetDialog
          person={p}
          onClose={() => setResetting(false)}
          onDone={() => {
            void load();
          }}
        />
      )}
    </div>
  );
}

function Kv({ k, v, mono = false }: { k: string; v: string; mono?: boolean }) {
  return (
    <>
      <dt className="text-mute">{k}</dt>
      <dd className={`min-w-0 break-words text-ink ${mono ? "font-mono text-[13px]" : ""}`}>{v || "—"}</dd>
    </>
  );
}

function Pill({ tone, icon, children }: { tone: "ok" | "warn" | "bad" | "mute"; icon?: React.ReactNode; children: React.ReactNode }) {
  const c = { ok: "border-ok/40 bg-ok/10 text-ok", warn: "border-warn/40 bg-warn/10 text-warn", bad: "border-bad/40 bg-bad/10 text-bad", mute: "border-line-2 text-mute" }[tone];
  return <span className={`flex items-center gap-1 rounded-full border px-2 py-0.5 text-[11px] ${c}`}>{icon}{children}</span>;
}

/**
 * Restablecer la contraseña del dominio. La temporal la genera AdminOps (fácil
 * de dictar y válida para el dominio) y se enseña una sola vez.
 */
function ResetDialog({ person, onClose, onDone }: { person: Person; onClose: () => void; onDone: () => void }) {
  const [mustChange, setMustChange] = useState(true);
  const [unlock, setUnlock] = useState(true);
  const [busy, setBusy] = useState(false);
  const [error, setError] = useState<string | null>(null);
  const [password, setPassword] = useState<string | null>(null);
  const copy = useCopy();

  const go = async () => {
    setBusy(true);
    setError(null);
    try {
      setPassword(await peopleApi.resetPassword(person.sam, mustChange, unlock));
      onDone();
    } catch (e) {
      setError(String(e));
    } finally {
      setBusy(false);
    }
  };

  const check = (v: boolean, set: (b: boolean) => void, label: string, sub: string) => (
    <label className="flex items-start gap-2.5">
      <input type="checkbox" checked={v} onChange={(e) => set(e.target.checked)} className="mt-0.5 size-4 accent-[var(--color-neon)]" />
      <span>
        <span className="block text-sm text-ink">{label}</span>
        <span className="block text-[11px] text-mute">{sub}</span>
      </span>
    </label>
  );

  return (
    <Modal
      title={password ? "Contraseña temporal" : `Restablecer la contraseña de ${person.name || person.sam}`}
      onClose={onClose}
      width="w-[480px]"
      footer={
        password ? (
          <Button onClick={onClose}>Hecho</Button>
        ) : (
          <>
            {error && <p className="mr-auto max-w-64 text-xs text-bad">{error}</p>}
            <Button kind="ghost" onClick={onClose}>
              Cancelar
            </Button>
            <Button onClick={() => void go()} disabled={busy}>
              {busy ? <Loader2 size={14} className="animate-spin" /> : <KeyRound size={14} />} Restablecer
            </Button>
          </>
        )
      }
    >
      {password ? (
        <div className="space-y-3">
          <div className="flex items-center gap-3 rounded-lg border border-ok/40 bg-ok/10 p-3">
            <span className="min-w-0 flex-1 font-mono text-lg tracking-wide text-ink select-text">{password}</span>
            <button onClick={() => void copy(password, "Contraseña")} className="text-dim hover:text-ink" title="Copiar">
              <Copy size={16} />
            </button>
          </div>
          <p className="text-xs text-dim">
            Díctasela o pásasela en persona. {mustChange ? "Tendrá que cambiarla al iniciar sesión. " : ""}Solo se enseña ahora: AdminOps no la guarda en ningún sitio, ni en el
            diario.
          </p>
        </div>
      ) : (
        <div className="space-y-3">
          <p className="text-sm text-dim">AdminOps genera una contraseña temporal fácil de dictar que cumple las reglas del dominio.</p>
          {check(mustChange, setMustChange, "Obligar a cambiarla al iniciar sesión", "Lo normal: la temporal solo sirve para entrar una vez.")}
          {check(unlock, setUnlock, "Desbloquear también la cuenta", "Casi siempre se ha bloqueado de tanto intentarlo.")}
        </div>
      )}
    </Modal>
  );
}

/** LAPS y BitLocker de un equipo concreto, a demanda. */
function MachineSecrets({ computer }: { computer: string }) {
  const [laps, setLaps] = useState<LapsPassword | null>(null);
  const [keys, setKeys] = useState<RecoveryKey[] | null>(null);
  const [shown, setShown] = useState(false);
  const [busy, setBusy] = useState<"laps" | "bl" | null>(null);
  const toast = useToast();
  const copy = useCopy();

  // La contraseña se tapa sola al minuto: no se queda a la vista en la pantalla.
  useEffect(() => {
    if (!shown) return;
    const t = window.setTimeout(() => setShown(false), 60_000);
    return () => window.clearTimeout(t);
  }, [shown]);

  const getLaps = async () => {
    setBusy("laps");
    try {
      setLaps(await peopleApi.laps(computer));
      setShown(true);
    } catch (e) {
      toast("error", String(e));
    } finally {
      setBusy(null);
    }
  };
  const getKeys = async () => {
    setBusy("bl");
    try {
      setKeys(await peopleApi.bitlocker(computer, null));
    } catch (e) {
      toast("error", String(e));
    } finally {
      setBusy(null);
    }
  };

  return (
    <div className="mt-2 space-y-1.5 border-t border-line/70 pt-2 text-xs">
      <div className="flex items-center gap-2">
        <span className="flex-1 text-dim">Administrador local (LAPS)</span>
        {laps && shown ? (
          <>
            <span className="font-mono text-ink select-text">{laps.password}</span>
            <button onClick={() => void copy(laps.password, "Contraseña")} className="text-mute hover:text-ink" title="Copiar">
              <Copy size={12} />
            </button>
            <button onClick={() => setShown(false)} className="text-mute hover:text-ink" title="Tapar">
              <EyeOff size={12} />
            </button>
          </>
        ) : (
          <button onClick={() => void getLaps()} disabled={busy !== null} className="flex items-center gap-1 text-neon hover:underline disabled:opacity-50">
            {busy === "laps" ? <Loader2 size={11} className="animate-spin" /> : <Eye size={11} />} Mostrar
          </button>
        )}
      </div>
      {laps && shown && <p className="text-[11px] text-mute">{laps.account} · {laps.source}{laps.expires ? ` · cambia el ${fecha(laps.expires)}` : ""} · se tapa sola en un minuto</p>}
      <div className="flex items-center gap-2">
        <span className="flex-1 text-dim">Recuperación de BitLocker</span>
        {!keys && (
          <button onClick={() => void getKeys()} disabled={busy !== null} className="flex items-center gap-1 text-neon hover:underline disabled:opacity-50">
            {busy === "bl" ? <Loader2 size={11} className="animate-spin" /> : <FolderLock size={11} />} Ver claves
          </button>
        )}
      </div>
      {keys && <KeyList keys={keys} />}
    </div>
  );
}

function KeyList({ keys }: { keys: RecoveryKey[] }) {
  const copy = useCopy();
  return (
    <ul className="space-y-1.5">
      {keys.map((k) => (
        <li key={k.name} className="rounded-md border border-line bg-panel px-2 py-1.5">
          <div className="flex items-center gap-2 text-[11px] text-mute">
            <span className="flex-1">
              {k.computer && <span className="font-mono text-dim">{k.computer} · </span>}
              ID <span className="font-mono">{k.keyId.slice(0, 8)}</span> · {fecha(k.created)}
            </span>
            <button onClick={() => void copy(k.password, "Clave")} className="hover:text-ink" title="Copiar la clave">
              <Copy size={11} />
            </button>
          </div>
          <div className="font-mono text-[12px] break-all text-ink select-text">{k.password}</div>
        </li>
      ))}
    </ul>
  );
}

/**
 * Contraseñas de un equipo cualquiera, aunque no se sepa de quién es: para
 * cuando estás delante de él. Y la clave de BitLocker por el ID que enseña la
 * pantalla de recuperación, que es lo único que ves cuando un equipo la pide.
 */
function ComputerSecrets({ initial }: { initial: string | null }) {
  const [computer, setComputer] = useState(initial ?? "");
  const [open, setOpen] = useState<string | null>(initial ? initial.toUpperCase() : null);
  const [keyId, setKeyId] = useState("");
  const [keys, setKeys] = useState<RecoveryKey[] | null>(null);
  const [busy, setBusy] = useState(false);
  const toast = useToast();

  const byId = async () => {
    setBusy(true);
    try {
      setKeys(await peopleApi.bitlocker(null, keyId));
    } catch (e) {
      setKeys(null);
      toast("error", String(e));
    } finally {
      setBusy(false);
    }
  };

  return (
    <Card title="Contraseñas de un equipo" icon={<KeyRound size={14} />} className="mt-6">
      <form
        className="flex gap-2"
        onSubmit={(e) => {
          e.preventDefault();
          if (computer.trim()) setOpen(computer.trim().toUpperCase());
        }}
      >
        <input value={computer} onChange={(e) => setComputer(e.target.value)} placeholder="Nombre del equipo" className={`${inputClass} font-mono text-xs`} aria-label="Nombre del equipo" />
        <Button kind="ghost" onClick={() => computer.trim() && setOpen(computer.trim().toUpperCase())} disabled={!computer.trim()}>
          <Check size={14} />
        </Button>
      </form>
      {open && (
        <div className="mt-2">
          <div className="font-mono text-xs text-ink">{open}</div>
          <MachineSecrets key={open} computer={open} />
        </div>
      )}

      <div className="mt-4 border-t border-line/70 pt-3">
        <p className="mb-2 text-[11px] text-mute">¿Un equipo pide la clave de recuperación al arrancar? Escribe el ID que enseña la pantalla.</p>
        <form
          className="flex gap-2"
          onSubmit={(e) => {
            e.preventDefault();
            void byId();
          }}
        >
          <input value={keyId} onChange={(e) => setKeyId(e.target.value)} placeholder="ID de la clave (p. ej. 1A2B3C4D)" className={`${inputClass} font-mono text-xs`} aria-label="ID de la clave de recuperación" />
          <Button kind="ghost" onClick={() => void byId()} disabled={busy || keyId.trim().length < 8}>
            {busy ? <Loader2 size={14} className="animate-spin" /> : <Search size={14} />}
          </Button>
        </form>
        {keys && <div className="mt-2"><KeyList keys={keys} /></div>}
      </div>
    </Card>
  );
}
