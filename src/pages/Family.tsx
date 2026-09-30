import { Ban, Clock, Plus, Save, ShieldCheck, X } from "lucide-react";
import { useCallback, useEffect, useRef, useState } from "react";
import { useConfirm, useToast } from "../components/feedback";
import { Button, Card, inputClass, Loading } from "../components/ui";
import { familyApi, type UserHours } from "../lib/api";
import { useLiveEffect } from "../lib/useLiveEffect";

const FILTERS: [string, string, string][] = [
  ["none", "Sin filtro", "Los DNS que da el router (lo normal)."],
  ["malware", "Malware y phishing", "Bloquea webs de virus y estafas. Cloudflare 1.1.1.2."],
  ["family", "Malware y contenido adulto", "Lo anterior más webs para adultos. Cloudflare 1.1.1.3."],
  ["strict", "Estricto", "Adultos, proxies y VPN para saltarse el filtro, y búsqueda segura forzada. CleanBrowsing Family."],
];

const SUGGESTED = ["tiktok.com", "instagram.com", "facebook.com", "roblox.com", "x.com", "twitch.tv"];
const DAYS = ["Lun", "Mar", "Mié", "Jue", "Vie", "Sáb", "Dom"];

const grid = (fn: (day: number, hour: number) => boolean) => Array.from({ length: 7 }, (_, d) => Array.from({ length: 24 }, (_, h) => fn(d, h)));
const PRESETS: [string, boolean[][]][] = [
  ["Sin límite", grid(() => true)],
  ["Escolar: L-V 7–21 h, fin de semana 9–22 h", grid((d, h) => (d < 5 ? h >= 7 && h < 21 : h >= 9 && h < 22))],
  ["Solo tardes: 15–21 h", grid((_, h) => h >= 15 && h < 21)],
  ["Horario de oficina: L-V 8–18 h", grid((d, h) => d < 5 && h >= 8 && h < 18)],
];

export function Family() {
  return (
    <div className="mx-auto grid max-w-6xl grid-cols-12 gap-4 p-6">
      <DnsFilter />
      <BlockedSites />
      <Schedule />
    </div>
  );
}

function DnsFilter() {
  const [active, setActive] = useState<string | null>(null);
  const [choice, setChoice] = useState("none");
  const [busy, setBusy] = useState(false);
  const toast = useToast();

  const load = useCallback(() => {
    familyApi
      .filterStatus()
      .then((s) => {
        setActive(s.active);
        setChoice(s.active === "custom" ? "none" : s.active);
      })
      .catch((e) => toast("error", String(e)));
  }, [toast]);

  useEffect(load, [load]);

  const apply = async () => {
    setBusy(true);
    try {
      await familyApi.setFilter(choice);
      toast("ok", choice === "none" ? "Filtro quitado." : "Filtro activado en todos los adaptadores.");
      load();
    } catch (e) {
      toast("error", String(e));
    } finally {
      setBusy(false);
    }
  };

  return (
    <Card title="Filtro de navegación" icon={<ShieldCheck size={14} />} className="col-span-12 lg:col-span-6">
      <p className="mb-3 text-sm text-dim">Filtra por DNS: afecta a todo el equipo y a todos los navegadores y apps, sin instalar nada.</p>
      <div className="space-y-1.5">
        {FILTERS.map(([id, title, sub]) => (
          <label key={id} className={`flex cursor-pointer items-start gap-3 rounded-lg border p-2.5 ${choice === id ? "border-neon bg-neon/5" : "border-line hover:border-line-2"}`}>
            <input type="radio" checked={choice === id} onChange={() => setChoice(id)} className="mt-1 accent-[var(--color-neon)]" />
            <span>
              <span className="block text-sm text-ink">
                {title}
                {active === id && <span className="ml-2 rounded bg-ok/10 px-1.5 text-[11px] text-ok">activo</span>}
              </span>
              <span className="block text-xs text-mute">{sub}</span>
            </span>
          </label>
        ))}
      </div>
      {active === "custom" && <p className="mt-2 text-xs text-warn">Ahora hay DNS personalizados (no son de ningún filtro): se sustituirán.</p>}
      <div className="mt-3 flex items-center justify-between gap-3">
        <p className="text-[11px] text-mute">Si un navegador usa su propio «DNS seguro» (Firefox), actívalo también allí o desactívalo.</p>
        <Button onClick={apply} disabled={busy || choice === active}>
          Aplicar
        </Button>
      </div>
    </Card>
  );
}

function BlockedSites() {
  const [sites, setSites] = useState<string[] | null>(null);
  const [draft, setDraft] = useState("");
  const [dirty, setDirty] = useState(false);
  const toast = useToast();

  useLiveEffect(
    (vigente) => {
      familyApi
        .blocked()
        .then((s) => vigente() && setSites(s))
        .catch((e) => {
          if (!vigente()) return;
          setSites([]);
          toast("error", String(e));
        });
    },
    [toast],
  );

  const add = (value: string) => {
    const v = value.trim().toLowerCase();
    if (!v || !sites || sites.includes(v)) return;
    setSites([...sites, v]);
    setDirty(true);
    setDraft("");
  };

  const save = async () => {
    if (!sites) return;
    try {
      setSites(await familyApi.setBlocked(sites));
      setDirty(false);
      toast("ok", "Sitios bloqueados guardados.");
    } catch (e) {
      toast("error", String(e));
    }
  };

  return (
    <Card title="Sitios bloqueados" icon={<Ban size={14} />} className="col-span-12 lg:col-span-6">
      <p className="mb-3 text-sm text-dim">Webs concretas que no se abren en este equipo (se bloquean también con «www.»).</p>
      <form
        className="flex gap-2"
        onSubmit={(e) => {
          e.preventDefault();
          add(draft);
        }}
      >
        <input value={draft} onChange={(e) => setDraft(e.target.value)} placeholder="tiktok.com" className={inputClass} />
        <Button kind="ghost" onClick={() => add(draft)}>
          <Plus size={14} /> Añadir
        </Button>
      </form>
      <div className="mt-2 flex flex-wrap gap-1.5">
        {SUGGESTED.filter((s) => !sites?.includes(s)).map((s) => (
          <button key={s} onClick={() => add(s)} className="rounded-full border border-dashed border-line-2 px-2.5 py-0.5 text-xs text-mute hover:text-ink">
            + {s}
          </button>
        ))}
      </div>
      {sites && sites.length > 0 && (
        <ul className="mt-3 flex flex-wrap gap-1.5">
          {sites.map((s) => (
            <li key={s} className="flex items-center gap-1 rounded-full bg-panel-2 py-0.5 pr-1.5 pl-2.5 text-xs text-ink">
              {s}
              <button
                onClick={() => {
                  setSites(sites.filter((x) => x !== s));
                  setDirty(true);
                }}
                className="text-mute hover:text-bad"
                title="Quitar"
              >
                <X size={12} />
              </button>
            </li>
          ))}
        </ul>
      )}
      <div className="mt-3 flex justify-end">
        <Button onClick={save} disabled={!dirty}>
          <Save size={14} /> Guardar
        </Button>
      </div>
    </Card>
  );
}

function Schedule() {
  const [users, setUsers] = useState<UserHours[] | null>(null);
  const [selected, setSelected] = useState<string>("");
  const [hours, setHours] = useState<boolean[][]>(grid(() => true));
  const [dirty, setDirty] = useState(false);
  const paint = useRef<boolean | null>(null);
  const toast = useToast();
  const { confirm, dialog } = useConfirm();

  const load = useCallback(async () => {
    try {
      const list = await familyApi.hours();
      setUsers(list);
      const first = list.find((u) => !u.isSelf);
      setSelected((s) => s || first?.name || "");
    } catch (e) {
      setUsers([]);
      toast("error", String(e));
    }
  }, [toast]);

  useEffect(() => {
    void load();
  }, [load]);

  const user = users?.find((u) => u.name === selected) ?? null;
  useEffect(() => {
    if (user) {
      setHours(user.hours);
      setDirty(false);
    }
  }, [user]);

  useEffect(() => {
    const up = () => (paint.current = null);
    window.addEventListener("pointerup", up);
    return () => window.removeEventListener("pointerup", up);
  }, []);

  const set = (d: number, h: number, v: boolean) => {
    setHours((cur) => cur.map((row, i) => (i === d ? row.map((x, j) => (j === h ? v : x)) : row)));
    setDirty(true);
  };

  const save = async () => {
    if (!user) return;
    const none = hours.every((r) => r.every((x) => !x));
    if (none || user.admin) {
      const ok = await confirm({
        title: none ? "¿Bloquear el acceso a todas horas?" : `«${user.name}» es administrador`,
        danger: none,
        confirmLabel: "Guardar igualmente",
        body: <p>{none ? "Con este horario la cuenta no podrá iniciar sesión nunca." : "Los administradores pueden quitarse el límite. Para un menor, usa una cuenta estándar."}</p>,
      });
      if (!ok) return;
    }
    try {
      await familyApi.setHours(user.name, hours);
      toast("ok", `Horario de «${user.name}» guardado.`);
      void load();
    } catch (e) {
      toast("error", String(e));
    }
  };

  return (
    <Card title="Horario de uso" icon={<Clock size={14} />} className="col-span-12">
      <p className="mb-3 text-sm text-dim">
        Horas en las que cada usuario puede iniciar sesión. Fuera del horario Windows no le deja entrar; si ya tiene la sesión abierta, sigue hasta que
        la cierre o se bloquee la pantalla.
      </p>
      {users === null ? (
        <Loading />
      ) : (
        <>
          <div className="mb-3 flex flex-wrap items-center gap-3">
            <select value={selected} onChange={(e) => setSelected(e.target.value)} className="rounded-md border border-line bg-void/60 px-3 py-2 text-sm text-ink outline-none">
              {users.map((u) => (
                <option key={u.name} value={u.name} disabled={u.isSelf}>
                  {u.fullName || u.name}
                  {u.isSelf ? " (tu cuenta)" : u.admin ? " · administrador" : ""}
                  {u.restricted ? " · con horario" : ""}
                </option>
              ))}
            </select>
            <select
              value=""
              onChange={(e) => {
                const p = PRESETS[Number(e.target.value)];
                if (p) {
                  setHours(p[1]);
                  setDirty(true);
                }
              }}
              className="rounded-md border border-line bg-void/60 px-3 py-2 text-sm text-dim outline-none"
            >
              <option value="">Aplicar un horario…</option>
              {PRESETS.map(([label], i) => (
                <option key={label} value={i}>
                  {label}
                </option>
              ))}
            </select>
            <span className="ml-auto flex items-center gap-3 text-xs text-mute">
              <span className="flex items-center gap-1">
                <span className="size-3 rounded-sm bg-neon/70" /> Permitido
              </span>
              <span className="flex items-center gap-1">
                <span className="size-3 rounded-sm border border-line bg-void" /> Bloqueado
              </span>
            </span>
          </div>
          {!user ? (
            <p className="text-sm text-mute">No hay otras cuentas locales. Crea una cuenta estándar para el menor en Administración → Usuarios locales.</p>
          ) : (
            <div className="overflow-x-auto select-none">
              <table className="border-separate border-spacing-[3px]">
                <thead>
                  <tr>
                    <th />
                    {Array.from({ length: 24 }, (_, h) => (
                      <th key={h} className="w-6 text-center font-mono text-[10px] font-normal text-mute">
                        {h % 3 === 0 ? h : ""}
                      </th>
                    ))}
                  </tr>
                </thead>
                <tbody>
                  {hours.map((row, d) => (
                    <tr key={d}>
                      <td className="pr-2 text-xs text-dim">{DAYS[d]}</td>
                      {row.map((on, h) => (
                        <td
                          key={h}
                          onPointerDown={() => {
                            paint.current = !on;
                            set(d, h, !on);
                          }}
                          onPointerEnter={() => paint.current !== null && set(d, h, paint.current)}
                          title={`${DAYS[d]} ${h}:00–${h + 1}:00`}
                          className={`h-6 w-6 cursor-pointer rounded-sm ${on ? "bg-neon/70 hover:bg-neon" : "border border-line bg-void hover:border-line-2"}`}
                        />
                      ))}
                    </tr>
                  ))}
                </tbody>
              </table>
            </div>
          )}
          <div className="mt-3 flex items-center justify-between gap-3">
            <p className="text-[11px] text-mute">Haz clic o arrastra sobre las horas. Solo cuentas locales; tu propia cuenta no se puede limitar.</p>
            <Button onClick={save} disabled={!user || !dirty}>
              <Save size={14} /> Guardar horario
            </Button>
          </div>
        </>
      )}
      {dialog}
    </Card>
  );
}
