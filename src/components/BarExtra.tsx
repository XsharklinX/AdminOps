import { useEffect, useMemo, useState } from "react";
import { contactsApi, disksApi, faultRecApi, kbdApi, printersApi, tweaksApi, type Contact, type KbdState, type PrinterInfo } from "../lib/api";
import { barApi, type BarWireless } from "../lib/api/rocketbar";
import { loadUiPref, saveUiPref } from "../lib/api/uipref";

/**
 * Las herramientas opcionales de la barra (apagadas de fábrica; se encienden en «Personalizar»): su panel al
 * lado de la barra. Bloquear no tiene panel: bloquea al tocarlo.
 */
export type ExtraTool = "contacts" | "printer" | "screens" | "wifi" | "keyboard" | "folders" | "restart";
export const EXTRA_TOOLS: readonly ExtraTool[] = ["contacts", "printer", "screens", "wifi", "keyboard", "folders", "restart"];
export const isExtra = (t: string): t is ExtraTool => (EXTRA_TOOLS as readonly string[]).includes(t);

/** Alto de cada panel (la ventana de la barra se ajusta a él). */
export const EXTRA_H: Record<ExtraTool, number> = { contacts: 300, printer: 236, screens: 128, wifi: 168, keyboard: 250, folders: 218, restart: 206 };

const btn = "rounded-md px-2 py-[4px] text-left text-[11.5px] font-medium text-[#e8eef4] transition-colors hover:bg-[#2e3b49] disabled:opacity-50";
const chip = "rounded-md bg-[#2e3b49] px-2 py-[3px] text-[11px] font-medium text-[#e8eef4] transition-colors hover:bg-[#3a4959] disabled:opacity-50";
const primary = "rounded-md bg-[#5b8def] px-2.5 py-[3px] text-[11.5px] font-semibold text-[#0c1422] disabled:opacity-50";
const title = "mb-1 text-[12px] font-semibold text-[#e8eef4]";
const muted = "text-[11px] text-[#a8b4c0]";

function Msg({ msg }: { msg: { text: string; ok: boolean } | null }) {
  if (!msg) return null;
  return <div className={`mt-1 text-[11px] leading-[14px] ${msg.ok ? "text-[#3ecf8e]" : "text-[#f2706b]"}`}>{msg.text}</div>;
}

/** Lo que tarda y luego dice si salió bien. */
function useMsg() {
  const [msg, setMsg] = useState<{ text: string; ok: boolean } | null>(null);
  const run = (p: Promise<unknown>, ok: string) => p.then(() => setMsg({ text: ok, ok: true }), (e) => setMsg({ text: String(e), ok: false }));
  return { msg, setMsg, run };
}

export function ExtraPanel({ tool, base }: { tool: ExtraTool; base: string }) {
  switch (tool) {
    case "contacts":
      return <Contacts base={base} />;
    case "printer":
      return <Printer base={base} />;
    case "screens":
      return <Screens base={base} />;
    case "wifi":
      return <Wireless base={base} />;
    case "keyboard":
      return <Keyboard base={base} />;
    case "folders":
      return <Folders base={base} />;
    case "restart":
      return <Restart base={base} />;
  }
}

/** Contactos: buscar a alguien y llamarle por Teams, escribirle o copiar su extensión. */
function Contacts({ base }: { base: string }) {
  const [all, setAll] = useState<Contact[] | null>(null);
  const [q, setQ] = useState("");
  const { msg, setMsg, run } = useMsg();
  useEffect(() => {
    contactsApi.list().then(
      (l) => setAll(l.filter((c) => !c.deleted).sort((a, b) => Number(b.favorite) - Number(a.favorite) || b.uses - a.uses || a.name.localeCompare(b.name))),
      (e) => setMsg({ text: String(e), ok: false }),
    );
  }, [setMsg]);
  const shown = useMemo(() => {
    const words = q.toLowerCase().normalize("NFD").replace(/\p{M}/gu, "").split(/\s+/).filter(Boolean);
    const plain = (c: Contact) => [c.name, c.role, c.company, c.extension, c.reason, ...c.tags].join(" ").toLowerCase().normalize("NFD").replace(/\p{M}/gu, "");
    return (all ?? []).filter((c) => words.every((w) => plain(c).includes(w))).slice(0, 4);
  }, [all, q]);
  const used = (c: Contact) => void contactsApi.touch(c.id).catch(() => {});
  return (
    <div className={base} role="dialog" aria-label="Contactos">
      <div className={title}>Contactos</div>
      <input
        autoFocus
        value={q}
        onChange={(e) => setQ(e.target.value)}
        placeholder="Buscar por nombre, área o extensión"
        className="mb-1 w-full rounded-md border border-[#2e3b49] bg-[#0e1319] px-2 py-[4px] text-[11.5px] text-[#e8eef4] outline-none placeholder:text-[#76869a] focus:border-[#5b8def]"
      />
      {all === null ? (
        <div className={muted}>Cargando…</div>
      ) : all.length === 0 ? (
        <div className={muted}>Aún no hay contactos. Se añaden en AdminOps → Contactos.</div>
      ) : shown.length === 0 ? (
        <div className={muted}>Nadie con «{q}».</div>
      ) : (
        shown.map((c) => (
          <div key={c.id} className="border-t border-[#2e3b49] py-[4px] first-of-type:border-t-0">
            <div className="truncate text-[11.5px] text-[#e8eef4]">
              <b>{c.name}</b>
              {c.role && <span className="text-[#76869a]"> · {c.role}</span>}
            </div>
            <div className="mt-[2px] flex flex-wrap gap-1">
              {c.email && (
                <>
                  <button type="button" className={primary} onClick={() => { used(c); void run(contactsApi.teams(c.email, true), `Llamando a ${c.name} por Teams`); }}>
                    Llamar
                  </button>
                  <button type="button" className={chip} onClick={() => { used(c); void run(contactsApi.teams(c.email, false), `Chat con ${c.name} abierto`); }}>
                    Teams
                  </button>
                  <button type="button" className={chip} onClick={() => { used(c); void run(contactsApi.email(c.email), `Correo a ${c.name}`); }}>
                    Correo
                  </button>
                </>
              )}
              {c.extension && (
                <button type="button" className={chip} title="Copiar la extensión" onClick={() => { used(c); void run(navigator.clipboard.writeText(c.extension), `Extensión ${c.extension} copiada`); }}>
                  Ext. {c.extension}
                </button>
              )}
              {!c.email && !c.extension && (c.phone || c.mobile) && (
                <button type="button" className={chip} onClick={() => { used(c); void run(navigator.clipboard.writeText(c.phone || c.mobile), "Teléfono copiado"); }}>
                  {c.phone || c.mobile}
                </button>
              )}
            </div>
          </div>
        ))
      )}
      <Msg msg={msg} />
    </div>
  );
}

function printerState(p: PrinterInfo): { text: string; tone: string } {
  if (p.offline) return { text: "sin conexión", tone: "#f2706b" };
  if (p.error) return { text: p.error.toLowerCase(), tone: "#f2706b" };
  if (p.jobs > 0) return { text: `${p.jobs} en cola`, tone: "#f5b84b" };
  return { text: "lista", tone: "#3ecf8e" };
}

/** La impresora: su estado, vaciar la cola atascada y cambiar la predeterminada. */
function Printer({ base }: { base: string }) {
  const [list, setList] = useState<PrinterInfo[] | null>(null);
  const [busy, setBusy] = useState(false);
  const { msg, setMsg } = useMsg();
  const load = () =>
    printersApi.list().then(
      (l) => setList(l.filter((p) => !p.virtual).sort((a, b) => Number(b.default) - Number(a.default)).slice(0, 4)),
      (e) => setMsg({ text: String(e), ok: false }),
    );
  useEffect(() => void load(), []); // eslint-disable-line react-hooks/exhaustive-deps
  const act = (p: Promise<unknown>, ok: (r: unknown) => string) => {
    setBusy(true);
    p.then(
      (r) => {
        setMsg({ text: ok(r), ok: true });
        void load();
      },
      (e) => setMsg({ text: String(e), ok: false }),
    ).finally(() => setBusy(false));
  };
  const def = list?.find((p) => p.default) ?? null;
  return (
    <div className={base} role="dialog" aria-label="La impresora">
      <div className={title}>La impresora</div>
      {list === null ? (
        <div className={muted}>Preguntando a Windows…</div>
      ) : list.length === 0 ? (
        <div className={muted}>No hay ninguna impresora instalada.</div>
      ) : (
        <>
          {def && (
            <div className="mb-1 rounded-md bg-[#0e1319] px-2 py-[5px]">
              <div className="truncate text-[11.5px] font-semibold text-[#e8eef4]">{def.name}</div>
              <div className="flex items-center justify-between text-[11px]">
                <span className="text-[#76869a]">Predeterminada</span>
                <span style={{ color: printerState(def).tone }}>{printerState(def).text}</span>
              </div>
              {def.jobs > 0 && (
                <button type="button" disabled={busy} className={`${primary} mt-1`} onClick={() => act(printersApi.clearQueue(def.name), (r) => (typeof r === "string" && r ? r : "Cola vaciada"))}>
                  Vaciar la cola
                </button>
              )}
            </div>
          )}
          {list
            .filter((p) => !p.default)
            .map((p) => (
              <div key={p.name} className="flex items-center justify-between gap-2 py-[2px] text-[11.5px]">
                <span className="min-w-0 truncate text-[#e8eef4]">{p.name}</span>
                <span className="shrink-0 text-[10.5px]" style={{ color: printerState(p).tone }}>
                  {printerState(p).text}
                </span>
                <button type="button" disabled={busy} className={`${chip} shrink-0`} onClick={() => act(printersApi.setDefault(p.name), () => `Ahora imprime en ${p.name}`)}>
                  Usar esta
                </button>
              </div>
            ))}
        </>
      )}
      <Msg msg={msg} />
    </div>
  );
}

/** Pantallas: lo de Win+P, con un clic. */
function Screens({ base }: { base: string }) {
  const { msg, run } = useMsg();
  const modes = [
    ["clone", "Duplicar", "Lo mismo en las dos (proyector)"],
    ["extend", "Extender", "Una pantalla más grande"],
    ["internal", "Solo esta", ""],
    ["external", "Solo la otra", ""],
  ] as const;
  return (
    <div className={base} role="dialog" aria-label="Pantallas">
      <div className={title}>Pantallas</div>
      <div className="grid grid-cols-2 gap-1">
        {modes.map(([m, label, hint]) => (
          <button key={m} type="button" className={`${chip} py-[5px] text-left`} title={hint || undefined} onClick={() => void run(barApi.displayMode(m), `Hecho: ${label.toLowerCase()}`)}>
            {label}
          </button>
        ))}
      </div>
      <Msg msg={msg} />
    </div>
  );
}

function Toggle({ on, disabled, onClick }: { on: boolean | null; disabled?: boolean; onClick: () => void }) {
  return (
    <button
      type="button"
      role="switch"
      aria-checked={!!on}
      disabled={disabled || on === null}
      onClick={onClick}
      className={`relative h-[18px] w-[32px] shrink-0 rounded-full transition-colors disabled:opacity-40 ${on ? "bg-[#3ecf8e]" : "bg-[#2e3b49]"}`}
    >
      <span className={`absolute top-[2px] h-[14px] w-[14px] rounded-full bg-white transition-all ${on ? "left-[16px]" : "left-[2px]"}`} />
    </button>
  );
}

/** Wi-Fi y Bluetooth: encender, apagar y «Reconectar» cuando la Wi-Fi se queda colgada. */
function Wireless({ base }: { base: string }) {
  const [s, setS] = useState<BarWireless | null>(null);
  const [busy, setBusy] = useState(false);
  const { msg, setMsg } = useMsg();
  const load = () => barApi.wireless().then(setS, (e) => setMsg({ text: String(e), ok: false }));
  useEffect(() => void load(), []); // eslint-disable-line react-hooks/exhaustive-deps
  const act = (p: Promise<unknown>, ok: (r: unknown) => string) => {
    setBusy(true);
    p.then(
      (r) => setMsg({ text: ok(r), ok: true }),
      (e) => setMsg({ text: String(e), ok: false }),
    ).finally(() => {
      setBusy(false);
      window.setTimeout(() => void load(), 2500);
    });
  };
  const row = "flex items-center justify-between gap-2 py-[3px] text-[11.5px] text-[#e8eef4]";
  return (
    <div className={base} role="dialog" aria-label="Wi-Fi y Bluetooth">
      <div className={title}>Wi-Fi y Bluetooth</div>
      {s === null ? (
        <div className={muted}>Mirando…</div>
      ) : (
        <>
          <div className={row}>
            <span>Wi-Fi{s.wifi === null && <span className="text-[#76869a]"> · no hay</span>}</span>
            <Toggle on={s.wifi} disabled={busy} onClick={() => act(barApi.setRadio("wifi", !s.wifi), (r) => (r ? "Wi-Fi encendida" : "Wi-Fi apagada"))} />
          </div>
          <div className="truncate text-[11px] text-[#76869a]">{s.line}</div>
          {s.wifi && (
            <button type="button" disabled={busy} className={`${primary} mt-1`} onClick={() => act(barApi.wifiReconnect(), (r) => String(r))}>
              Reconectar la Wi-Fi
            </button>
          )}
          <div className={`${row} mt-1 border-t border-[#2e3b49] pt-[5px]`}>
            <span>Bluetooth{s.bluetooth === null && <span className="text-[#76869a]"> · no hay</span>}</span>
            <Toggle on={s.bluetooth} disabled={busy} onClick={() => act(barApi.setRadio("bluetooth", !s.bluetooth), (r) => (r ? "Bluetooth encendido" : "Bluetooth apagado"))} />
          </div>
        </>
      )}
      <Msg msg={msg} />
    </div>
  );
}

/** El teclado en español: un clic cuando «la ñ no sale». */
const KBD_UNDO = "adminops.bar.kbdUndo";

function Keyboard({ base }: { base: string }) {
  const [s, setS] = useState<KbdState | null>(null);
  const [busy, setBusy] = useState(false);
  const { msg, setMsg } = useMsg();
  // La entrada del diario de «Dejar solo español», para poder volver a como estaba (de este usuario).
  // Se guarda en un archivo de AdminOps (no en localStorage) para que valga en modo técnico y en modo usuario.
  const [undo, setUndo] = useState<number | null>(null);
  useEffect(() => {
    let alive = true;
    void loadUiPref("kbdUndo", KBD_UNDO, (v): v is number => typeof v === "number" && v > 0).then((v) => alive && v && setUndo(v));
    return () => {
      alive = false;
    };
  }, []);
  const load = () => kbdApi.state().then(setS, (e) => setMsg({ text: String(e), ok: false }));
  useEffect(() => void load(), []); // eslint-disable-line react-hooks/exhaustive-deps
  const onlySpanish = !!s && s.rows.length > 0 && s.rows.every((r) => /^es\b/i.test(r.tag));
  const act = (p: Promise<string>, after?: () => void) => {
    setBusy(true);
    p.then(
      (text) => {
        setMsg({ text, ok: true });
        after?.();
        void load();
      },
      (e) => setMsg({ text: String(e), ok: false }),
    ).finally(() => setBusy(false));
  };
  const remember = (id: number | null) => {
    setUndo(id);
    saveUiPref("kbdUndo", id || null).catch(() => {});
  };
  return (
    <div className={base} role="dialog" aria-label="El teclado">
      <div className={title}>El teclado</div>
      {s === null ? (
        <div className={muted}>Mirando…</div>
      ) : (
        <>
          {/* Cada teclado instalado: tocarlo lo pone ya, en todos los programas (sin quitar los demás). */}
          <div className="max-h-[132px] overflow-y-auto">
            {s.rows.map((r) => (
              <button
                key={`${r.tag}-${r.klid ?? ""}`}
                type="button"
                disabled={busy || r.active || !r.klid || s.otherUser}
                className={`flex w-full items-center justify-between gap-2 rounded-md px-2 py-[3px] text-left text-[11.5px] ${r.active ? "bg-[#1b242e] text-[#e8eef4]" : "text-[#a8b4c0] hover:bg-[#1b242e] hover:text-[#e8eef4]"} disabled:cursor-default`}
                title={r.active ? "Es el que escribe ahora" : "Escribir con este teclado"}
                onClick={() => r.klid && act(kbdApi.use(r.langId, r.klid))}
              >
                <span className="min-w-0 truncate">
                  {r.language} · {r.layout}
                </span>
                {r.active && <span className="shrink-0 text-[#3ecf8e]">✓</span>}
              </button>
            ))}
          </div>
          {s.rows.length > 1 && <div className="mt-0.5 text-[10.5px] text-[#76869a]">También con {s.shortcut.label}</div>}
          <div className="mt-1 flex flex-wrap gap-1">
            {!onlySpanish && s.hasSpanish && !s.otherUser && (
              <button
                type="button"
                disabled={busy}
                className={chip}
                title={s.warning ?? "Quita los demás teclados: «la ñ no sale», teclas cambiadas"}
                onClick={() => {
                  setBusy(true);
                  kbdApi
                    .onlySpanish()
                    .then(
                      (r) => {
                        remember(r.entryId);
                        setMsg({ text: r.message, ok: true });
                        void load();
                      },
                      (e) => setMsg({ text: String(e), ok: false }),
                    )
                    .finally(() => setBusy(false));
                }}
              >
                Dejar solo español
              </button>
            )}
            {undo && (
              <button
                type="button"
                disabled={busy}
                className={chip}
                title="Vuelve a poner los teclados que había antes de «Dejar solo español»"
                onClick={() =>
                  act(
                    tweaksApi.revertEntry(undo).then((r) => r.message || "Teclados como estaban."),
                    () => remember(null),
                  )
                }
              >
                Volver a como estaba
              </button>
            )}
          </div>
          {!s.hasSpanish && <div className="mt-1 text-[11px] text-[#f5b84b]">No hay ningún español instalado: se añade desde AdminOps → Teclado.</div>}
          {s.otherUser && <div className="mt-1 text-[11px] text-[#a8b4c0]">Con AdminOps abierto con otra cuenta no se cambia el teclado de este usuario.</div>}
        </>
      )}
      <Msg msg={msg} />
    </div>
  );
}
const FOLDERS_KEY = "adminops.barFolders";
const leaf = (p: string) => p.replace(/[\\/]+$/, "").split(/[\\/]/).pop() || p;

/** Mis carpetas: Escritorio, Descargas, Documentos y hasta 3 elegidas (la de la oficina). */

function Folders({ base }: { base: string }) {
  const [mine, setMine] = useState<string[]>(() => {
    try {
      const v: unknown = JSON.parse(localStorage.getItem(FOLDERS_KEY) ?? "[]");
      return Array.isArray(v) ? v.filter((x): x is string => typeof x === "string").slice(0, 3) : [];
    } catch {
      return [];
    }
  });
  const { msg, setMsg } = useMsg();
  const save = (l: string[]) => {
    setMine(l);
    localStorage.setItem(FOLDERS_KEY, JSON.stringify(l));
  };
  const open = (which: string) => void barApi.openFolder(which).catch((e) => setMsg({ text: String(e), ok: false }));
  return (
    <div className={base} role="dialog" aria-label="Mis carpetas">
      <div className={title}>Mis carpetas</div>
      <div className="grid grid-cols-3 gap-1">
        {(
          [
            ["desktop", "Escritorio"],
            ["downloads", "Descargas"],
            ["documents", "Documentos"],
          ] as const
        ).map(([id, label]) => (
          <button key={id} type="button" className={`${chip} py-[5px] text-center`} onClick={() => open(id)}>
            {label}
          </button>
        ))}
      </div>
      {mine.map((p) => (
        <div key={p} className="mt-1 flex items-center gap-1">
          <button type="button" className={`${chip} min-w-0 flex-1 truncate py-[5px]`} onClick={() => open(p)}>
            {leaf(p)}
          </button>
          <button type="button" className={`${btn} shrink-0 px-1.5 text-[#76869a]`} aria-label={`Quitar ${leaf(p)}`} title="Quitar de la lista" onClick={() => save(mine.filter((x) => x !== p))}>
            ✕
          </button>
        </div>
      ))}
      {mine.length < 3 && (
        <button
          type="button"
          className={`${btn} mt-1 text-[#5b8def]`}
          onClick={() =>
            void disksApi.pickFolder().then(
              (p) => p && !mine.includes(p) && save([...mine, p]),
              (e) => setMsg({ text: String(e), ok: false }),
            )
          }
        >
          + Añadir una carpeta (la de la oficina)
        </button>
      )}
      <Msg msg={msg} />
    </div>
  );
}

/** Programas que suelen tener algo sin guardar. */
const EDITORS = /word|excel|powerpoint|outlook|bloc de notas|notepad|paint|acrobat|onenote|visio|project|access|publisher/i;

/** Reiniciar sin miedo: ahora o esta noche, sin cerrar nada a la fuerza. */
function Restart({ base }: { base: string }) {
  const [open, setOpen] = useState<string[] | null>(null);
  const [tonight, setTonight] = useState<number | null>(null);
  // «Reiniciar ahora» pide un segundo toque: un clic sin querer no apaga el equipo.
  const [sure, setSure] = useState(false);
  const { msg, setMsg } = useMsg();
  useEffect(() => {
    faultRecApi.windows().then(
      (w) => setOpen([...new Set(w.map((x) => x.app).filter((a) => EDITORS.test(a)))]),
      () => setOpen([]),
    );
    barApi.restart("").then((r) => setTonight(r.tonight), () => {});
  }, []);
  const at = (t: number) => new Date(t * 1000).toLocaleTimeString("es", { hour: "2-digit", minute: "2-digit" });
  return (
    <div className={base} role="dialog" aria-label="Reiniciar">
      <div className={title}>Reiniciar</div>
      {open === null ? (
        <div className={muted}>Mirando qué hay abierto…</div>
      ) : open.length ? (
        <div className="text-[11px] leading-[14px] text-[#f5b84b]">Abierto: {open.join(", ")}. Guarda antes; si algo queda sin guardar, Windows lo enseña y deja cancelar.</div>
      ) : (
        <div className="text-[11px] text-[#3ecf8e]">Nada abierto con trabajo sin guardar.</div>
      )}
      <div className="mt-1.5 flex flex-wrap gap-1">
        <button
          type="button"
          className={primary}
          onClick={() => {
            if (!sure) return setSure(true);
            setSure(false);
            void barApi.restart("now").catch((e) => setMsg({ text: String(e), ok: false }));
          }}
        >
          {sure ? "Sí, reiniciar ya" : "Reiniciar ahora"}
        </button>
        {sure && (
          <button type="button" className={chip} onClick={() => setSure(false)}>
            No
          </button>
        )}
        {tonight === null ? (
          <button
            type="button"
            className={chip}
            onClick={() =>
              void barApi.restart("tonight").then(
                (r) => {
                  setTonight(r.tonight);
                  setMsg({ text: r.tonight ? `Se reiniciará a las ${at(r.tonight)}. Deja AdminOps abierto.` : "Programado", ok: true });
                },
                (e) => setMsg({ text: String(e), ok: false }),
              )
            }
          >
            Esta noche
          </button>
        ) : (
          <button
            type="button"
            className={chip}
            onClick={() =>
              void barApi.restart("cancel").then(
                () => {
                  setTonight(null);
                  setMsg({ text: "Reinicio de esta noche cancelado", ok: true });
                },
                (e) => setMsg({ text: String(e), ok: false }),
              )
            }
          >
            Cancelar el de las {at(tonight)}
          </button>
        )}
      </div>
      <Msg msg={msg} />
    </div>
  );
}
