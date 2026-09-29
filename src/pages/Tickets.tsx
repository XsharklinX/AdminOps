import {
  ArrowLeft,
  ArrowRight,
  ChevronDown,
  ChevronUp,
  ClipboardList,
  Download,
  ExternalLink,
  FileDown,
  FolderOpen,
  Globe,
  Hourglass,
  House,
  IdCard,
  Loader2,
  Lock,
  LockOpen,
  LogOut,
  Mail,
  Minus,
  Pencil,
  Plus,
  Printer,
  RotateCw,
  Search,
  ShieldCheck,
  SquareArrowOutUpRight,
  SquarePen,
  Ticket,
  Trash2,
  TriangleAlert,
  WifiOff,
  X,
  XCircle,
} from "lucide-react";
import { useCallback, useEffect, useLayoutEffect, useRef, useState } from "react";
import { useConfirm, useToast } from "../components/feedback";
import { SheetPanel } from "../components/MachineSheetCard";
import { Button, inputClass, Loading, Modal } from "../components/ui";
import { lockApi, portalsApi, type Portal, type PortalAction } from "../lib/api";
import { clearPortalError, lastPortalKey, unreadFromTitle, useOnline, usePortalView, type PortalDownload } from "../lib/portalState";
import { getPrefs, windowRect } from "../lib/prefs";

// Con el zoom de la interfaz aplicado: la vista web va en píxeles de la ventana.
const rectOf = windowRect;

export type PortalKind = "" | "inventory" | "mail";

const ZOOMS = [0.5, 0.67, 0.75, 0.8, 0.9, 1, 1.1, 1.25, 1.5, 1.75, 2];

/** Outlook listo para usar: la cuenta del trabajo (Microsoft 365) o la personal. */
const MAIL_PRESETS = {
  work: { name: "Correo", url: "https://outlook.office.com/mail/" },
  personal: { name: "Correo", url: "https://outlook.live.com/mail/" },
};

/**
 * Portales de cada tipo ya leídos: al volver a la página la vista se muestra al
 * momento, sin esperar a leer la lista (se actualiza igualmente por detrás).
 */
const listCache = new Map<PortalKind, Portal[]>();

/** Portal a mostrar: el actual si sigue existiendo, si no el último usado o el primero. */
function pickActive(list: Portal[], current: string | null, lastKey: string): string | null {
  if (current && list.some((p) => p.id === current)) return current;
  let last: string | null = null;
  try {
    last = localStorage.getItem(lastKey);
  } catch {
    /* sin almacenamiento */
  }
  return list.find((p) => p.id === last)?.id ?? list[0]?.id ?? null;
}

/**
 * Portales web dentro de AdminOps. `kind`: "" Tickets, "inventory" el inventario web
 * de la empresa, "mail" el correo de Outlook. `covered`: hay un diálogo de la app
 * encima (la vista web nativa lo taparía).
 */
export function Tickets({ covered = false, kind = "" }: { covered?: boolean; kind?: PortalKind }) {
  const lastKey = lastPortalKey(kind);
  const [sheetOpen, setSheetOpen] = useState(false);
  const [portals, setPortalsState] = useState<Portal[] | null>(() => listCache.get(kind) ?? null);
  const [active, setActive] = useState<string | null>(() => {
    const cached = listCache.get(kind);
    return cached ? pickActive(cached, null, lastKey) : null;
  });
  const setPortals = useCallback(
    (next: Portal[] | ((l: Portal[] | null) => Portal[] | null)) =>
      setPortalsState((l) => {
        const v = typeof next === "function" ? next(l) : next;
        if (v) listCache.set(kind, v);
        return v;
      }),
    [kind],
  );
  const [editing, setEditing] = useState<Portal | "new" | null>(null);
  const [menu, setMenu] = useState(false);
  const [findOpen, setFindOpen] = useState(false);
  const area = useRef<HTMLDivElement>(null);
  const toast = useToast();
  const { confirm, dialog } = useConfirm();
  const [confirming, setConfirming] = useState(false);
  const view = usePortalView(active);

  const load = useCallback(async () => {
    const list = (await portalsApi.list()).filter((p) => (p.kind ?? "") === kind);
    setPortals(list);
    setActive((a) => pickActive(list, a, lastKey));
  }, [kind, lastKey, setPortals]);

  useEffect(() => {
    load().catch((e) => toast("error", String(e)));
  }, [load, toast]);

  // Al cerrar la página, su vista nativa (que va por encima de la interfaz) se oculta.
  const shownRef = useRef<string | null>(null);
  useEffect(() => () => void (shownRef.current && portalsApi.hide(shownRef.current)), []);

  // La vista nativa se pinta por encima de todo: se oculta si algo de la app va
  // encima (diálogos, menús) o si la página no cargó (se enseña el aviso en su lugar).
  const overlay = editing !== null || confirming || covered || menu;
  const hidden = overlay || !!view.error;
  useLayoutEffect(() => {
    const el = area.current;
    // Solo la vista de esta página: la del router puede estar viva en otra.
    if (!active || !el || hidden) {
      if (shownRef.current) portalsApi.hide(shownRef.current);
      return;
    }
    shownRef.current = active;
    try {
      localStorage.setItem(lastKey, active);
    } catch {
      /* sin almacenamiento */
    }
    portalsApi.show(active, rectOf(el)).catch((e) => toast("error", String(e)));
    const sync = () => portalsApi.bounds(active, rectOf(el));
    const ro = new ResizeObserver(sync);
    ro.observe(el);
    window.addEventListener("resize", sync);
    return () => {
      ro.disconnect();
      window.removeEventListener("resize", sync);
    };
    // `findOpen`: la barra de búsqueda cambia el alto del área.
  }, [active, hidden, portals, toast, lastKey, findOpen]);

  const remove = async (p: Portal) => {
    setConfirming(true);
    const ok = await confirm({ title: "Eliminar portal", body: `¿Quitar «${p.name}»? Las sesiones guardadas en la web no se borran.`, confirmLabel: "Eliminar", danger: true });
    setConfirming(false);
    if (!ok) return;
    await portalsApi.remove(p.id).catch((e) => toast("error", String(e)));
    setActive(null);
    load();
  };

  const signOut = async (p: Portal) => {
    try {
      await portalsApi.signOut(p.id);
      toast("ok", p.private ? "Sesión cerrada: no queda nada guardado en este equipo." : "Cerrando la sesión…");
      if (p.private) {
        // La vista se cerró: al volver a mostrarla empieza de cero.
        shownRef.current = null;
        setActive(null);
        setTimeout(() => setActive(p.id), 50);
      }
    } catch (e) {
      toast("error", String(e));
    }
  };

  const compose = async () => {
    try {
      await portalsApi.compose("");
    } catch (e) {
      toast("error", String(e));
    }
  };

  if (!portals) return <Loading page />;

  const portal = portals.find((p) => p.id === active) ?? null;
  const nav = (a: PortalAction) => {
    if (!active) return;
    if (a === "reload" || a === "home") clearPortalError(active);
    portalsApi.nav(active, a).catch((e) => toast("error", String(e)));
  };
  const iconBtn = "rounded-md p-1.5 text-dim transition-colors hover:bg-panel-2 hover:text-ink disabled:opacity-30 disabled:hover:bg-transparent";
  const unread = kind === "mail" ? unreadFromTitle(view.title) : null;

  return (
    <div className="flex h-full flex-col">
      {/* Portales y acciones del portal */}
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
              {kind === "mail" ? <Mail size={13} /> : <Globe size={13} />} {p.name}
              {p.id === active && unread !== null && unread > 0 && <span className="rounded-full bg-neon px-1.5 text-[10px] font-semibold text-void">{unread}</span>}
              {p.private && <span title="Sesión privada: se cierra al salir de AdminOps" className="text-[10px] text-mute">· privada</span>}
            </button>
          ))}
          {/* Un solo correo: basta con uno configurado. */}
          {!(kind === "mail" && portals.length > 0) && (
            <button onClick={() => setEditing("new")} className={iconBtn} title={kind === "inventory" ? "Añadir web de inventario" : "Añadir portal"}>
              <Plus size={15} />
            </button>
          )}
        </div>
        {kind === "inventory" && portal && (
          <button
            onClick={() => setSheetOpen(!sheetOpen)}
            className={`flex shrink-0 items-center gap-1.5 rounded-md px-2.5 py-1.5 text-xs transition-colors ${sheetOpen ? "bg-neon/10 text-neon" : "text-dim hover:bg-panel-2 hover:text-ink"}`}
            title="Datos de este equipo para copiarlos en el formulario"
          >
            <IdCard size={13} /> Datos del equipo
          </button>
        )}
        {kind === "mail" && portal && (
          <>
            <button onClick={compose} className="flex shrink-0 items-center gap-1.5 rounded-md bg-neon/10 px-2.5 py-1.5 text-xs text-neon hover:bg-neon/20">
              <SquarePen size={13} /> Redactar
            </button>
            <button
              onClick={() => signOut(portal)}
              className="flex shrink-0 items-center gap-1.5 rounded-md px-2.5 py-1.5 text-xs text-dim hover:bg-panel-2 hover:text-ink"
              title="Cerrar la sesión del correo en este equipo"
            >
              <LogOut size={13} /> Cerrar sesión
            </button>
          </>
        )}
        {portal && (
          <div className="flex shrink-0 items-center gap-0.5">
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

      {portal && (
        <BrowserBar
          portal={portal}
          onNav={nav}
          findOpen={findOpen}
          setFindOpen={setFindOpen}
          menu={menu}
          setMenu={setMenu}
          onZoom={(z) =>
            portalsApi
              .zoom(portal.id, z)
              .then((saved) => setPortals((l) => l?.map((p) => (p.id === portal.id ? { ...p, zoom: saved } : p)) ?? l))
              .catch((e) => toast("error", String(e)))
          }
        />
      )}

      {portal && <NetworkNotice portal={portal} onReload={() => nav("reload")} />}

      {portal ? (
        // Área que ocupa la vista web nativa (se pinta por encima de este div).
        <div className="flex min-h-0 flex-1">
          <div ref={area} className="relative flex-1 bg-void">
            {view.loading && !view.error && <div className="absolute inset-x-0 top-0 z-10 h-0.5 overflow-hidden bg-neon/15"><div className="h-full w-1/3 animate-[portalbar_1.1s_ease-in-out_infinite] bg-neon" /></div>}
            {view.error ? (
              <ErrorPanel
                portal={portal}
                url={view.url}
                message={view.error}
                onRetry={() => nav("reload")}
                onHome={() => nav("home")}
                onExternal={() => portalsApi.openExternal(portal.id)}
              />
            ) : menu ? (
              <DownloadsMenu downloads={view.downloads} onClose={() => setMenu(false)} />
            ) : (
              <p className="flex h-full items-center justify-center gap-2 text-sm text-mute">
                <Loader2 size={14} className="animate-spin" /> Abriendo {portal.name}…
              </p>
            )}
          </div>
          {kind === "inventory" && sheetOpen && <SheetPanel onClose={() => setSheetOpen(false)} />}
        </div>
      ) : (
        <Empty kind={kind} onAdd={(preset) => setEditing(preset ?? "new")} />
      )}

      {editing && (
        <PortalEditor
          kind={kind}
          initial={editing === "new" ? null : editing}
          onClose={() => setEditing(null)}
          onSaved={(p) => {
            setEditing(null);
            clearPortalError(p.id);
            setActive(p.id);
            load();
          }}
        />
      )}
      {dialog}
    </div>
  );
}

/** Barra de navegador: historial, dirección, zoom, buscar, imprimir y descargas. */
function BrowserBar({
  portal,
  onNav,
  onZoom,
  findOpen,
  setFindOpen,
  menu,
  setMenu,
}: {
  portal: Portal;
  onNav: (a: PortalAction) => void;
  onZoom: (z: number) => void;
  findOpen: boolean;
  setFindOpen: (v: boolean) => void;
  menu: boolean;
  setMenu: (v: boolean) => void;
}) {
  const view = usePortalView(portal.id);
  const toast = useToast();
  const [address, setAddress] = useState("");
  const [editingAddress, setEditingAddress] = useState(false);
  const [query, setQuery] = useState("");
  const shownUrl = view.url ?? portal.url;
  useEffect(() => {
    if (!editingAddress) setAddress(shownUrl);
  }, [shownUrl, editingAddress]);

  const secure = shownUrl.startsWith("https://");
  const host = (() => {
    try {
      return new URL(shownUrl).host;
    } catch {
      return "";
    }
  })();
  const zoom = portal.zoom && portal.zoom > 0 ? portal.zoom : 1;
  const stepZoom = (dir: 1 | -1) => {
    const i = ZOOMS.findIndex((z) => z >= zoom - 0.001);
    const next = ZOOMS[Math.min(ZOOMS.length - 1, Math.max(0, (i < 0 ? ZOOMS.indexOf(1) : i) + dir))];
    if (next !== zoom) onZoom(next);
  };
  const go = async () => {
    setEditingAddress(false);
    if (!address.trim() || address === shownUrl) return;
    try {
      const inside = await portalsApi.go(portal.id, address);
      if (!inside) toast("info", "Esa dirección está fuera de este portal: se abrió en tu navegador.");
    } catch (e) {
      toast("error", String(e));
    }
  };
  const find = (backwards = false) => query.trim() && portalsApi.find(portal.id, query, backwards).catch(() => {});
  const running = view.downloads.filter((d) => d.state === "running").length;
  const btn = "rounded-md p-1.5 text-dim transition-colors hover:bg-panel-2 hover:text-ink disabled:opacity-30 disabled:hover:bg-transparent";

  return (
    <div className="border-b border-line">
      <div className="flex items-center gap-1 px-3 py-1.5">
        <button onClick={() => onNav("back")} disabled={!view.canBack} className={btn} title="Atrás">
          <ArrowLeft size={15} />
        </button>
        <button onClick={() => onNav("forward")} disabled={!view.canForward} className={btn} title="Adelante">
          <ArrowRight size={15} />
        </button>
        {view.loading ? (
          <button onClick={() => onNav("stop")} className={btn} title="Detener">
            <X size={15} />
          </button>
        ) : (
          <button onClick={() => onNav("reload")} className={btn} title="Recargar (la página, sin cerrar la sesión)">
            <RotateCw size={15} />
          </button>
        )}
        <button onClick={() => onNav("home")} className={btn} title="Página inicial del portal">
          <House size={15} />
        </button>

        {/* Dirección: se puede escribir otra dentro del portal */}
        <label
          className="mx-1 flex min-w-0 flex-1 items-center gap-2 rounded-md border border-line bg-void/60 px-2.5 py-1 focus-within:border-neon/50"
          title={secure ? `Conexión cifrada con ${host}` : `Conexión sin cifrar con ${host}`}
        >
          {secure ? <Lock size={12} className="shrink-0 text-ok" /> : <LockOpen size={12} className="shrink-0 text-warn" />}
          <input
            value={editingAddress ? address : shownUrl}
            onFocus={(e) => {
              setEditingAddress(true);
              setAddress(shownUrl);
              requestAnimationFrame(() => e.target.select());
            }}
            onBlur={() => setEditingAddress(false)}
            onChange={(e) => setAddress(e.target.value)}
            onKeyDown={(e) => {
              if (e.key === "Enter") void go();
              if (e.key === "Escape") {
                setEditingAddress(false);
                (e.target as HTMLInputElement).blur();
              }
            }}
            spellCheck={false}
            className="min-w-0 flex-1 bg-transparent font-mono text-[11px] text-dim outline-none focus:text-ink"
          />
          <button
            onMouseDown={(e) => e.preventDefault()}
            onClick={() => navigator.clipboard.writeText(shownUrl).then(() => toast("ok", "Dirección copiada."))}
            className="shrink-0 text-[10px] text-mute hover:text-ink"
          >
            Copiar
          </button>
        </label>

        <div className="flex items-center rounded-md border border-line">
          <button onClick={() => stepZoom(-1)} disabled={zoom <= ZOOMS[0]} className="px-1.5 py-1 text-dim hover:text-ink disabled:opacity-30" title="Reducir">
            <Minus size={12} />
          </button>
          <button onClick={() => onZoom(1)} className="min-w-10 font-mono text-[11px] text-dim hover:text-ink" title="Tamaño normal">
            {Math.round(zoom * 100)}%
          </button>
          <button onClick={() => stepZoom(1)} disabled={zoom >= ZOOMS[ZOOMS.length - 1]} className="px-1.5 py-1 text-dim hover:text-ink disabled:opacity-30" title="Ampliar">
            <Plus size={12} />
          </button>
        </div>
        <button onClick={() => setFindOpen(!findOpen)} className={`${btn} ${findOpen ? "text-neon" : ""}`} title="Buscar en la página">
          <Search size={15} />
        </button>
        <button onClick={() => onNav("print")} className={btn} title="Imprimir o guardar como PDF">
          <Printer size={15} />
        </button>
        <button onClick={() => setMenu(!menu)} className={`${btn} relative ${menu ? "text-neon" : ""}`} title="Descargas">
          <Download size={15} />
          {running > 0 && <span className="absolute -top-0.5 -right-0.5 size-2 animate-pulse rounded-full bg-neon" />}
          {running === 0 && view.downloads.length > 0 && <span className="absolute -top-0.5 -right-0.5 size-2 rounded-full bg-ok" />}
        </button>
      </div>
      {findOpen && (
        <div className="flex items-center gap-2 border-t border-line/60 px-3 py-1.5">
          <Search size={13} className="text-mute" />
          <input
            autoFocus
            value={query}
            onChange={(e) => setQuery(e.target.value)}
            onKeyDown={(e) => {
              if (e.key === "Enter") void find(e.shiftKey);
              if (e.key === "Escape") setFindOpen(false);
            }}
            placeholder="Buscar en la página (Intro: siguiente · Mayús+Intro: anterior)"
            className="min-w-0 flex-1 bg-transparent text-xs outline-none placeholder:text-mute"
          />
          <button onClick={() => find(true)} className={btn} title="Anterior">
            <ChevronUp size={14} />
          </button>
          <button onClick={() => find(false)} className={btn} title="Siguiente">
            <ChevronDown size={14} />
          </button>
          <button onClick={() => setFindOpen(false)} className={btn} title="Cerrar">
            <X size={14} />
          </button>
        </div>
      )}
    </div>
  );
}

const hostOf = (url: string) => {
  try {
    return new URL(url).host;
  } catch {
    return url;
  }
};

/**
 * Aviso cuando la culpa es de la red y no de AdminOps: el equipo está sin
 * conexión (la página se recarga sola al volver) o la web tarda en responder.
 * Va fuera del área de la vista web, que lo taparía.
 */
function NetworkNotice({ portal, onReload }: { portal: Portal; onReload: () => void }) {
  const view = usePortalView(portal.id);
  const online = useOnline();
  if (!online)
    return (
      <div className="flex items-center gap-2 border-b border-warn/30 bg-warn/10 px-4 py-1.5 text-xs text-warn">
        <WifiOff size={13} className="shrink-0" />
        <span className="min-w-0 flex-1">Este equipo está sin conexión a Internet. {portal.name} se recargará solo en cuanto vuelva la conexión.</span>
      </div>
    );
  if (!view.slow || view.error) return null;
  return (
    <div className="flex items-center gap-2 border-b border-line bg-panel-2/60 px-4 py-1.5 text-xs text-dim">
      <Hourglass size={13} className="shrink-0 text-warn" />
      <span className="min-w-0 flex-1">
        {hostOf(view.url ?? portal.url)} está tardando en responder: es la conexión o el servidor de la web, no AdminOps. Puedes seguir esperando.
      </span>
      <button onClick={onReload} className="shrink-0 text-neon hover:underline">
        Recargar
      </button>
      <button onClick={() => portalsApi.openExternal(portal.id)} className="shrink-0 text-dim hover:text-ink">
        Abrir en el navegador
      </button>
    </div>
  );
}

/** Descargas de esta sesión (solo el nombre del archivo: la ruta lleva el del usuario). */
function DownloadsMenu({ downloads, onClose }: { downloads: PortalDownload[]; onClose: () => void }) {
  const toast = useToast();
  const run = (p: Promise<void>) => p.catch((e) => toast("error", String(e)));
  return (
    <div className="mx-auto mt-6 max-w-lg rounded-xl border border-line bg-panel p-4">
      <div className="mb-3 flex items-center">
        <span className="flex items-center gap-2 text-sm font-medium text-ink">
          <Download size={14} /> Descargas
        </span>
        <button onClick={onClose} className="ml-auto text-xs text-mute hover:text-ink">
          Volver a la página
        </button>
      </div>
      {downloads.length === 0 ? (
        <p className="py-6 text-center text-sm text-mute">Lo que descargues desde el portal aparece aquí (se guarda en tu carpeta Descargas).</p>
      ) : (
        <ul className="divide-y divide-line/60">
          {downloads.map((d) => (
            <li key={d.index} className="flex items-center gap-3 py-2">
              {d.state === "running" ? <Loader2 size={14} className="animate-spin text-neon" /> : d.state === "done" ? <FileDown size={14} className="text-ok" /> : <XCircle size={14} className="text-bad" />}
              <span className="min-w-0 flex-1 truncate text-sm text-ink" title={d.name}>
                {d.name}
              </span>
              {d.state === "done" && (
                <>
                  <button onClick={() => run(portalsApi.openDownload(d.index))} className="text-xs text-neon hover:underline">
                    Abrir
                  </button>
                  <button onClick={() => run(portalsApi.revealDownload(d.index))} className="flex items-center gap-1 text-xs text-dim hover:text-ink" title="Mostrar en la carpeta">
                    <FolderOpen size={12} />
                  </button>
                </>
              )}
              {d.state === "running" && <span className="text-xs text-mute">Descargando…</span>}
              {d.state === "failed" && <span className="text-xs text-bad">Falló</span>}
            </li>
          ))}
        </ul>
      )}
    </div>
  );
}

/** En lugar de la página en blanco de WebView2: qué pasa y qué hacer. */
function ErrorPanel({
  portal,
  url,
  message,
  onRetry,
  onHome,
  onExternal,
}: {
  portal: Portal;
  url: string | null;
  message: string;
  onRetry: () => void;
  onHome: () => void;
  onExternal: () => void;
}) {
  const host = hostOf(url ?? portal.url);
  const online = useOnline();
  return (
    <div className="mx-auto mt-16 max-w-md p-6 text-center">
      {online ? <TriangleAlert size={32} className="mx-auto mb-3 text-warn" /> : <WifiOff size={32} className="mx-auto mb-3 text-warn" />}
      <h2 className="mb-1 text-base font-semibold text-ink">No se pudo abrir {host}</h2>
      <p className="mb-5 text-sm text-dim">{message}</p>
      {!online && <p className="-mt-3 mb-5 text-xs text-mute">Se reintentará solo cuando vuelva la conexión.</p>}
      <div className="flex flex-wrap justify-center gap-2">
        <Button onClick={onRetry}>
          <RotateCw size={14} /> Reintentar
        </Button>
        <Button kind="ghost" onClick={onHome}>
          <House size={14} /> Página inicial
        </Button>
        <Button kind="ghost" onClick={onExternal}>
          <ExternalLink size={14} /> Abrir en el navegador
        </Button>
      </div>
    </div>
  );
}

function Empty({ kind, onAdd }: { kind: PortalKind; onAdd: (preset?: Portal) => void }) {
  if (kind === "mail") {
    const preset = (which: keyof typeof MAIL_PRESETS): Portal => ({ id: "", ...MAIL_PRESETS[which], extraDomains: [], kind: "mail", private: true, autofill: true });
    return (
      <div className="mx-auto max-w-xl p-10 text-center">
        <Mail size={36} className="mx-auto mb-4 text-neon" />
        <h2 className="mb-2 text-lg font-semibold">Tu correo de Outlook dentro de AdminOps</h2>
        <p className="mb-5 text-sm text-dim">
          Sin configurar Outlook en cada equipo. Guarda tu cuenta (cifrada) y AdminOps rellena el inicio de sesión: solo confirmas la verificación en el móvil.
        </p>
        <div className="flex flex-wrap justify-center gap-2">
          <Button onClick={() => onAdd(preset("work"))}>
            <Mail size={14} /> Outlook del trabajo (Microsoft 365)
          </Button>
          <Button kind="ghost" onClick={() => onAdd(preset("personal"))}>
            Outlook.com personal
          </Button>
        </div>
        <p className="mt-6 flex items-start gap-2 text-left text-xs text-mute">
          <ShieldCheck size={14} className="mt-0.5 shrink-0 text-neon" />
          Con «sesión privada» (recomendado en equipos de clientes) no se guarda nada en el equipo y la sesión se cierra al salir de AdminOps.
        </p>
      </div>
    );
  }
  const inventory = kind === "inventory";
  return (
    <div className="mx-auto max-w-xl p-10 text-center">
      {inventory ? <ClipboardList size={36} className="mx-auto mb-4 text-neon" /> : <Ticket size={36} className="mx-auto mb-4 text-neon" />}
      <h2 className="mb-2 text-lg font-semibold">{inventory ? "El inventario de tu empresa, sin salir de AdminOps" : "Tus tickets, sin salir de AdminOps"}</h2>
      <p className="mb-5 text-sm text-dim">
        {inventory
          ? "Añade la web donde tu empresa lleva el inventario de equipos (por ejemplo https://inventario.pgr.gob.do) y se abrirá aquí mismo. Con «Datos del equipo» copias modelo, número de serie, IP… para rellenar el formulario."
          : "Añade la web donde gestionas los tickets o soportes (la intranet de tu empresa, GLPI, osTicket, Jira…) y se abrirá aquí mismo, con la sesión recordada."}
      </p>
      <Button onClick={() => onAdd()}>
        <Plus size={14} /> {inventory ? "Añadir web de inventario" : "Añadir portal"}
      </Button>
      <p className="mt-6 flex items-start gap-2 text-left text-xs text-mute">
        <ShieldCheck size={14} className="mt-0.5 shrink-0 text-neon" />
        La web se abre aislada: no puede usar ninguna función de AdminOps y solo navega dentro de su dominio. Cualquier otro enlace se abre en tu
        navegador.
      </p>
    </div>
  );
}

function PortalEditor({ kind, initial, onClose, onSaved }: { kind: PortalKind; initial: Portal | null; onClose: () => void; onSaved: (p: Portal) => void }) {
  const mail = kind === "mail";
  const [p, setP] = useState<Portal>(initial ?? { id: "", name: "", url: "", extraDomains: [], kind, zoom: getPrefs().portalZoom });
  // En el correo los dominios de Microsoft se añaden solos: aquí solo los propios.
  const [extra, setExtra] = useState((initial?.extraDomains ?? []).filter((d) => !mail || !/(microsoft|office|outlook|live|msauth|msftauth|sharepoint)/.test(d)).join(", "));
  const [user, setUser] = useState("");
  const [password, setPassword] = useState("");
  const [hasPassword, setHasPassword] = useState(false);
  const [clearPassword, setClearPassword] = useState(false);
  const [lockOn, setLockOn] = useState(true);
  const [error, setError] = useState<string | null>(null);

  useEffect(() => {
    if (initial?.id)
      portalsApi
        .login(initial.id)
        .then((l) => {
          setUser(l?.user ?? "");
          setHasPassword(!!l?.hasPassword);
        })
        .catch(() => {});
    lockApi
      .status()
      .then((s) => setLockOn(s.enabled))
      .catch(() => {});
  }, [initial?.id]);

  const save = async () => {
    setError(null);
    try {
      const saved = await portalsApi.save({ ...p, kind, extraDomains: extra.split(/[\s,;]+/).filter(Boolean) });
      if (user.trim() || password || clearPassword || hasPassword) {
        await portalsApi.setLogin(saved.id, user, clearPassword ? "" : password || undefined);
      }
      onSaved(saved);
    } catch (e) {
      setError(String(e));
    }
  };

  const check = (checked: boolean, onChange: (v: boolean) => void, title: string, sub: string) => (
    <label className="flex items-start gap-2.5">
      <input type="checkbox" checked={checked} onChange={(e) => onChange(e.target.checked)} className="mt-0.5 size-4 accent-[var(--color-neon)]" />
      <span>
        <span className="block text-sm text-ink">{title}</span>
        <span className="block text-[11px] text-mute">{sub}</span>
      </span>
    </label>
  );

  return (
    <Modal
      title={initial?.id ? (mail ? "Editar correo" : "Editar portal") : mail ? "Configurar el correo" : "Nuevo portal"}
      onClose={onClose}
      width="w-[560px]"
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
        <div className="grid grid-cols-5 gap-3">
          <label className="col-span-2 block">
            <span className="mb-1 block text-xs text-dim">Nombre</span>
            <input autoFocus value={p.name} onChange={(e) => setP({ ...p, name: e.target.value })} maxLength={40} placeholder={mail ? "Correo" : "Intranet PGR"} className={inputClass} />
          </label>
          <label className="col-span-3 block">
            <span className="mb-1 block text-xs text-dim">Dirección</span>
            <input value={p.url} onChange={(e) => setP({ ...p, url: e.target.value })} placeholder="https://intranet.pgr.gob.do" className={`${inputClass} font-mono text-xs`} />
          </label>
        </div>

        <fieldset className="rounded-lg border border-line p-3">
          <legend className="px-1 text-xs text-dim">Inicio de sesión guardado {mail ? "" : "(opcional)"}</legend>
          <div className="grid grid-cols-2 gap-3">
            <input value={user} onChange={(e) => setUser(e.target.value)} placeholder={mail ? "tu.correo@empresa.com" : "Usuario"} autoComplete="off" className={inputClass} />
            <input
              type="password"
              value={password}
              onChange={(e) => {
                setPassword(e.target.value);
                setClearPassword(false);
              }}
              placeholder={hasPassword && !clearPassword ? "•••••••• (guardada)" : "Contraseña"}
              autoComplete="new-password"
              className={inputClass}
            />
          </div>
          {hasPassword && !password && (
            <button onClick={() => setClearPassword(!clearPassword)} className="mt-1 text-[11px] text-mute hover:text-bad">
              {clearPassword ? "Se borrará la contraseña guardada (deshacer)" : "Borrar la contraseña guardada"}
            </button>
          )}
          <p className="mt-2 text-[11px] text-mute">
            Se guarda cifrada con AdminOps (en el USB si usas la versión portable, para llevarla a cualquier equipo). Nunca se muestra ni sale de AdminOps.
            {mail && " Si la cuenta tiene verificación en dos pasos, solo tendrás que confirmarla en el móvil."}
          </p>
          {!lockOn && (user || password) && (
            <p className="mt-2 flex items-start gap-1.5 text-[11px] text-warn">
              <TriangleAlert size={12} className="mt-0.5 shrink-0" /> AdminOps no tiene bloqueo con PIN: quien use este equipo (o tenga el USB) podría entrar con tu cuenta. Actívalo en Ajustes → Seguridad.
            </p>
          )}
          <div className="mt-3">{check(!!p.autofill, (v) => setP({ ...p, autofill: v }), "Rellenar el inicio de sesión automáticamente", mail ? "Escribe la cuenta y la contraseña en la página de Microsoft y pulsa Siguiente." : "Rellena usuario y contraseña cuando la página los pida (no envía el formulario).")}</div>
        </fieldset>

        <div className="space-y-2.5">
          {check(
            !!p.private,
            (v) => setP({ ...p, private: v }),
            "Sesión privada: cerrar la sesión al salir de AdminOps",
            "No se guardan cookies ni historial en este equipo. Recomendado en equipos de clientes. Con la cuenta guardada, volver a entrar es un clic.",
          )}
          {!mail &&
            check(
              p.popups === "window",
              (v) => setP({ ...p, popups: v ? "window" : "" }),
              "Abrir las ventanas emergentes en una ventana aparte",
              "Para webs que abren informes, adjuntos o formularios en ventana propia (si no, se abren dentro del portal).",
            )}
        </div>

        <label className="block">
          <span className="mb-1 block text-xs text-dim">{mail ? "Página de inicio de sesión de tu empresa (opcional)" : "Otros dominios permitidos (opcional)"}</span>
          <input value={extra} onChange={(e) => setExtra(e.target.value)} placeholder={mail ? "sts.pgr.gob.do" : "login.empresa.com, archivos.empresa.com"} className={`${inputClass} font-mono text-xs`} />
          <span className="mt-1 block text-[11px] text-mute">
            {mail
              ? "Solo si al iniciar sesión Microsoft te lleva a una página de tu empresa. Los dominios de Microsoft y el de tu dominio habitual ya se permiten."
              : "Solo si el inicio de sesión o los archivos del portal están en otro dominio. Los subdominios del portal ya se permiten."}
          </span>
        </label>
        {!mail && (
          <p className="text-[11px] text-mute">
            Webs de la empresa con la cuenta de Windows (dominio, como *.pgr.gob.do): tras guardar, cierra y vuelve a abrir AdminOps y entrará sola con tu usuario de Windows, sin
            pedir contraseña.
          </p>
        )}
      </div>
    </Modal>
  );
}
