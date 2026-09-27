import { listen } from "@tauri-apps/api/event";
import { ArrowLeft, ArrowRight, ExternalLink, Globe, House, Loader2, Pencil, Plus, RotateCw, ShieldCheck, SquareArrowOutUpRight, Ticket, Trash2 } from "lucide-react";
import { useCallback, useEffect, useLayoutEffect, useRef, useState } from "react";
import { useConfirm, useToast } from "../components/feedback";
import { Button, Modal, inputClass } from "../components/ui";
import { portalsApi, type Portal } from "../lib/api";
import { windowRect } from "../lib/prefs";

const LAST = "adminops.lastPortal";

// Con el zoom de la interfaz aplicado: la vista web va en píxeles de la ventana.
const rectOf = windowRect;

/** `covered`: hay un diálogo de la app encima (la vista web nativa lo taparía). */
export function Tickets({ covered = false }: { covered?: boolean }) {
  const [portals, setPortals] = useState<Portal[] | null>(null);
  const [active, setActive] = useState<string | null>(null);
  const [editing, setEditing] = useState<Portal | "new" | null>(null);
  const [loading, setLoading] = useState(false);
  const [current, setCurrent] = useState<string | null>(null);
  const area = useRef<HTMLDivElement>(null);
  const toast = useToast();
  const { confirm, dialog } = useConfirm();
  const [confirming, setConfirming] = useState(false);

  const load = useCallback(async () => {
    const list = await portalsApi.list();
    setPortals(list);
    setActive((a) => {
      if (a && list.some((p) => p.id === a)) return a;
      let last: string | null = null;
      try {
        last = localStorage.getItem(LAST);
      } catch {
        /* sin almacenamiento */
      }
      return list.find((p) => p.id === last)?.id ?? list[0]?.id ?? null;
    });
  }, []);

  useEffect(() => {
    load().catch((e) => toast("error", String(e)));
  }, [load, toast]);

  // Al cerrar la página, su vista nativa (que va por encima de la interfaz) se oculta.
  const shownRef = useRef<string | null>(null);
  useEffect(() => () => void (shownRef.current && portalsApi.hide(shownRef.current)), []);

  useEffect(() => {
    const un = listen<{ id: string; url: string; loading: boolean }>("portal-load", (e) => {
      if (e.payload.id !== active) return;
      setLoading(e.payload.loading);
      setCurrent(e.payload.url);
    });
    return () => {
      un.then((f) => f());
    };
  }, [active]);

  // Mostrar / colocar la vista del portal activo sobre el área reservada.
  const overlay = editing !== null || confirming || covered;
  useLayoutEffect(() => {
    const el = area.current;
    // Solo la vista de esta página: la del router puede estar viva en otra.
    if (!active || !el || overlay) {
      if (shownRef.current) portalsApi.hide(shownRef.current);
      return;
    }
    shownRef.current = active;
    try {
      localStorage.setItem(LAST, active);
    } catch {
      /* sin almacenamiento */
    }
    setCurrent(null);
    portalsApi.show(active, rectOf(el)).catch((e) => toast("error", String(e)));
    const sync = () => portalsApi.bounds(active, rectOf(el));
    const ro = new ResizeObserver(sync);
    ro.observe(el);
    window.addEventListener("resize", sync);
    return () => {
      ro.disconnect();
      window.removeEventListener("resize", sync);
    };
  }, [active, overlay, portals, toast]);

  const remove = async (p: Portal) => {
    setConfirming(true);
    const ok = await confirm({ title: "Eliminar portal", body: `¿Quitar «${p.name}» de Tickets? Las sesiones guardadas no se borran.`, confirmLabel: "Eliminar", danger: true });
    setConfirming(false);
    if (!ok) return;
    await portalsApi.remove(p.id).catch((e) => toast("error", String(e)));
    setActive(null);
    load();
  };

  if (!portals) return <p className="p-8 font-mono text-sm text-mute">Cargando…</p>;

  const portal = portals.find((p) => p.id === active) ?? null;
  const nav = (a: "back" | "forward" | "reload" | "home") => active && portalsApi.nav(active, a).catch((e) => toast("error", String(e)));
  const iconBtn = "rounded-md p-1.5 text-dim transition-colors hover:bg-panel-2 hover:text-ink disabled:opacity-30";

  return (
    <div className="flex h-full flex-col">
      <div className="flex items-center gap-2 border-b border-line px-4 py-2">
        <div className="flex min-w-0 flex-1 items-center gap-1 overflow-x-auto">
          {portals.map((p) => (
            <button
              key={p.id}
              onClick={() => setActive(p.id)}
              className={`flex shrink-0 items-center gap-1.5 rounded-md px-3 py-1.5 text-sm transition-colors ${
                p.id === active ? "bg-neon/10 text-neon" : "text-dim hover:bg-panel-2 hover:text-ink"
              }`}
            >
              <Globe size={13} /> {p.name}
            </button>
          ))}
          <button onClick={() => setEditing("new")} className={iconBtn} title="Añadir portal">
            <Plus size={15} />
          </button>
        </div>
        {portal && (
          <div className="flex shrink-0 items-center gap-0.5">
            <button onClick={() => nav("back")} className={iconBtn} title="Atrás">
              <ArrowLeft size={15} />
            </button>
            <button onClick={() => nav("forward")} className={iconBtn} title="Adelante">
              <ArrowRight size={15} />
            </button>
            <button onClick={() => nav("reload")} className={iconBtn} title="Recargar">
              {loading ? <Loader2 size={15} className="animate-spin text-neon" /> : <RotateCw size={15} />}
            </button>
            <button onClick={() => nav("home")} className={iconBtn} title="Página inicial del portal">
              <House size={15} />
            </button>
            <span className="mx-1 h-5 w-px bg-line" />
            <button onClick={() => portalsApi.openWindow(portal.id).catch((e) => toast("error", String(e)))} className={iconBtn} title="Abrir en una ventana aparte">
              <SquareArrowOutUpRight size={15} />
            </button>
            <button onClick={() => portalsApi.openExternal(portal.id)} className={iconBtn} title="Abrir en el navegador">
              <ExternalLink size={15} />
            </button>
            <button onClick={() => setEditing(portal)} className={iconBtn} title="Editar portal">
              <Pencil size={15} />
            </button>
            <button onClick={() => remove(portal)} className={`${iconBtn} hover:text-bad`} title="Eliminar portal">
              <Trash2 size={15} />
            </button>
          </div>
        )}
      </div>
      {portal && current && (
        <div className="truncate border-b border-line/60 px-4 py-1 font-mono text-[11px] text-mute" title={current}>
          {current}
        </div>
      )}

      {portal ? (
        // Área que ocupa la vista web nativa (se pinta por encima de este div).
        <div ref={area} className="relative flex-1 bg-void">
          <p className="flex h-full items-center justify-center gap-2 text-sm text-mute">
            <Loader2 size={14} className="animate-spin" /> Abriendo {portal.name}…
          </p>
        </div>
      ) : (
        <Empty onAdd={() => setEditing("new")} />
      )}

      {editing && (
        <PortalEditor
          initial={editing === "new" ? null : editing}
          onClose={() => setEditing(null)}
          onSaved={(p) => {
            setEditing(null);
            setActive(p.id);
            load();
          }}
        />
      )}
      {dialog}
    </div>
  );
}

function Empty({ onAdd }: { onAdd: () => void }) {
  return (
    <div className="mx-auto max-w-xl p-10 text-center">
      <Ticket size={36} className="mx-auto mb-4 text-neon" />
      <h2 className="mb-2 text-lg font-semibold">Tus tickets, sin salir de AdminOps</h2>
      <p className="mb-5 text-sm text-dim">
        Añade la web donde gestionas los tickets o soportes (la intranet de tu empresa, GLPI, osTicket, Jira…) y se abrirá aquí mismo, con la
        sesión recordada.
      </p>
      <Button onClick={onAdd}>
        <Plus size={14} /> Añadir portal
      </Button>
      <p className="mt-6 flex items-start gap-2 text-left text-xs text-mute">
        <ShieldCheck size={14} className="mt-0.5 shrink-0 text-neon" />
        La web se abre aislada: no puede usar ninguna función de AdminOps y solo navega dentro de su dominio. Cualquier otro enlace se abre en tu
        navegador.
      </p>
    </div>
  );
}

function PortalEditor({ initial, onClose, onSaved }: { initial: Portal | null; onClose: () => void; onSaved: (p: Portal) => void }) {
  const [p, setP] = useState<Portal>(initial ?? { id: "", name: "", url: "", extraDomains: [] });
  const [extra, setExtra] = useState((initial?.extraDomains ?? []).join(", "));
  const [error, setError] = useState<string | null>(null);

  const save = async () => {
    setError(null);
    try {
      onSaved(await portalsApi.save({ ...p, extraDomains: extra.split(/[\s,;]+/).filter(Boolean) }));
    } catch (e) {
      setError(String(e));
    }
  };

  return (
    <Modal
      title={initial ? "Editar portal" : "Nuevo portal"}
      onClose={onClose}
      width="w-[540px]"
      footer={
        <>
          {error && <p className="mr-auto max-w-72 text-xs text-bad">{error}</p>}
          <Button kind="ghost" onClick={onClose}>
            Cancelar
          </Button>
          <Button onClick={save} disabled={!p.name.trim() || !p.url.trim()}>
            Guardar
          </Button>
        </>
      }
    >
      <div className="space-y-3">
        <label className="block">
          <span className="mb-1 block text-xs text-dim">Nombre</span>
          <input autoFocus value={p.name} onChange={(e) => setP({ ...p, name: e.target.value })} maxLength={40} placeholder="Intranet PGR" className={inputClass} />
        </label>
        <label className="block">
          <span className="mb-1 block text-xs text-dim">Dirección</span>
          <input
            value={p.url}
            onChange={(e) => setP({ ...p, url: e.target.value })}
            placeholder="https://intranet.pgr.gob.do"
            className={`${inputClass} font-mono text-xs`}
          />
        </label>
        <label className="block">
          <span className="mb-1 block text-xs text-dim">Otros dominios permitidos (opcional)</span>
          <input
            value={extra}
            onChange={(e) => setExtra(e.target.value)}
            placeholder="login.empresa.com, archivos.empresa.com"
            className={`${inputClass} font-mono text-xs`}
          />
          <span className="mt-1 block text-[11px] text-mute">
            Solo si el inicio de sesión o los archivos del portal están en otro dominio. Los subdominios del portal ya se permiten.
          </span>
        </label>
        <p className="text-[11px] text-mute">
          Al iniciar sesión marca «Recordarme» en la web si lo ofrece; AdminOps también te ofrecerá guardar la contraseña, como un navegador.
        </p>
      </div>
    </Modal>
  );
}
