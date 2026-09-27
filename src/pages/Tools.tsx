import {
  Activity,
  Bluetooth,
  AppWindow,
  BadgeCheck,
  Bug,
  Calendar,
  ChartLine,
  Clock,
  Cog,
  Copy,
  Cpu,
  Database,
  Download,
  Eye,
  EyeOff,
  FileText,
  Flame,
  Folder,
  FolderOpen,
  FolderSymlink,
  Gauge,
  Globe,
  Grid3x3,
  Handshake,
  HardDrive,
  Info,
  Key,
  LayoutGrid,
  LifeBuoy,
  List,
  Loader2,
  Lock,
  MemoryStick,
  Monitor,
  Mouse,
  Network,
  Package,
  Pencil,
  Plus,
  Power,
  Printer,
  RefreshCw,
  Search,
  Settings,
  Shield,
  ShieldCheck,
  SlidersHorizontal,
  SquareTerminal,
  Star,
  Terminal,
  Trash2,
  TriangleAlert,
  Type,
  Undo2,
  Usb,
  Users,
  Volume2,
  Wifi,
  type LucideIcon,
} from "lucide-react";
import { useCallback, useEffect, useMemo, useState } from "react";
import { useConfirm, useToast } from "../components/feedback";
import { Button, Card, Modal, inputClass } from "../components/ui";
import { hwApi, toolboxApi, type CustomKind, type CustomTool, type Inventory, type ToolGroup, type ToolboxView } from "../lib/api";

const ICONS: Record<string, LucideIcon> = {
  terminal: Terminal,
  "terminal-admin": SquareTerminal,
  database: Database,
  activity: Activity,
  file: FileText,
  cog: Cog,
  cpu: Cpu,
  "hard-drive": HardDrive,
  monitor: Monitor,
  list: List,
  clock: Clock,
  users: Users,
  sliders: SlidersHorizontal,
  lock: Lock,
  flame: Flame,
  badge: BadgeCheck,
  "folder-share": FolderSymlink,
  printer: Printer,
  package: Package,
  settings: Settings,
  shield: Shield,
  undo: Undo2,
  info: Info,
  gauge: Gauge,
  chart: ChartLine,
  memory: MemoryStick,
  trash: Trash2,
  bug: Bug,
  alert: TriangleAlert,
  layout: LayoutGrid,
  network: Network,
  wifi: Wifi,
  power: Power,
  volume: Volume2,
  globe: Globe,
  calendar: Calendar,
  mouse: Mouse,
  key: Key,
  folder: Folder,
  search: Search,
  grid: Grid3x3,
  download: Download,
  star: Star,
  bluetooth: Bluetooth,
  type: Type,
  "life-buoy": LifeBuoy,
  refresh: RefreshCw,
  usb: Usb,
  handshake: Handshake,
  app: AppWindow,
  "folder-open": FolderOpen,
};

/** Iconos que se pueden elegir para un acceso propio. */
const CUSTOM_ICONS = ["app", "terminal", "folder", "globe", "cog", "package", "download", "shield", "key", "handshake", "monitor", "file", "star", "bug"];

const GROUPS: { id: ToolGroup; title: string }[] = [
  { id: "console", title: "Consolas y editores" },
  { id: "admin", title: "Administración de Windows" },
  { id: "diag", title: "Diagnóstico y rendimiento" },
  { id: "panel", title: "Panel de control" },
  { id: "settings", title: "Configuración de Windows" },
  { id: "folders", title: "Carpetas" },
  { id: "boot", title: "Arranque y recuperación" },
  { id: "remote", title: "Asistencia remota" },
];

const KIND_LABEL: Record<CustomKind, string> = { program: "Programa", folder: "Carpeta", url: "Web" };

/** Minúsculas y sin tildes, para buscar "configuracion" o "CONFIGURACIÓN" igual. */
const norm = (s: string) => s.normalize("NFD").replace(/[\u0300-\u036f]/g, "").toLowerCase();

interface Tile {
  id: string;
  name: string;
  description: string;
  icon: string;
  asAdmin: boolean;
  disabled: string | null;
  confirm: string | null;
  custom: CustomTool | null;
}

export function Tools({ isAdmin }: { isAdmin: boolean }) {
  const [data, setData] = useState<ToolboxView | null>(null);
  const [query, setQuery] = useState("");
  const [editing, setEditing] = useState<CustomTool | "new" | null>(null);
  const [launching, setLaunching] = useState<string | null>(null);
  const toast = useToast();
  const { confirm, dialog } = useConfirm();

  const load = useCallback(() => toolboxApi.list().then(setData).catch((e) => toast("error", String(e))), [toast]);
  useEffect(() => {
    load();
  }, [load]);

  const tiles = useMemo(() => {
    if (!data) return new Map<string, Tile>();
    const m = new Map<string, Tile>();
    for (const t of data.tools)
      m.set(t.id, {
        id: t.id,
        name: t.name,
        description: t.description,
        icon: t.icon,
        asAdmin: t.asAdmin,
        disabled: t.unavailable ?? (t.needsAdmin && !isAdmin ? "Requiere ejecutar AdminOps como administrador." : null),
        confirm: t.confirm,
        custom: null,
      });
    for (const c of data.custom)
      m.set(c.id, {
        id: c.id,
        name: c.name,
        description: c.kind === "program" ? `${c.target}${c.args ? ` ${c.args}` : ""}` : c.target,
        icon: c.icon,
        asAdmin: c.kind === "program" && data.elevated,
        disabled: null,
        confirm: null,
        custom: c,
      });
    return m;
  }, [data, isAdmin]);

  const results = useMemo(() => {
    const q = norm(query.trim());
    if (!q || !data) return null;
    const hit = (s: string) => norm(s).includes(q);
    return [
      ...data.custom.filter((c) => hit(c.name) || hit(c.target)).map((c) => c.id),
      ...data.tools.filter((t) => hit(t.name) || hit(t.description) || hit(t.keywords)).map((t) => t.id),
    ];
  }, [query, data]);

  const launch = async (t: Tile) => {
    if (t.disabled) return;
    if (t.confirm && !(await confirm({ title: t.name, body: t.confirm, confirmLabel: "Continuar", danger: true }))) return;
    setLaunching(t.id);
    try {
      await toolboxApi.launch(t.id);
    } catch (e) {
      toast("error", `${t.name}: ${e}`);
    } finally {
      // Breve indicación visual: el programa tarda un momento en aparecer.
      window.setTimeout(() => setLaunching((x) => (x === t.id ? null : x)), 900);
    }
  };

  const toggleFavorite = async (id: string) => {
    if (!data) return;
    try {
      const favorites = await toolboxApi.setFavorite(id, !data.favorites.includes(id));
      setData({ ...data, favorites });
    } catch (e) {
      toast("error", String(e));
    }
  };

  const removeCustom = async (c: CustomTool) => {
    if (!(await confirm({ title: "Eliminar acceso", body: `¿Eliminar «${c.name}» de tus accesos?`, confirmLabel: "Eliminar", danger: true })))
      return;
    try {
      await toolboxApi.deleteCustom(c.id);
      load();
    } catch (e) {
      toast("error", String(e));
    }
  };

  if (!data) return <p className="p-8 font-mono text-sm text-mute">Cargando herramientas…</p>;

  const favorites = data.favorites.filter((id) => tiles.has(id));
  const grid = (ids: string[]) => (
    <div className="grid grid-cols-2 gap-2 md:grid-cols-3 xl:grid-cols-4">
      {ids.map((id) => {
        const t = tiles.get(id)!;
        return (
          <ToolTile
            key={id}
            tile={t}
            favorite={data.favorites.includes(id)}
            busy={launching === id}
            onLaunch={() => launch(t)}
            onFavorite={() => toggleFavorite(id)}
            onEdit={t.custom ? () => setEditing(t.custom) : undefined}
            onDelete={t.custom ? () => removeCustom(t.custom!) : undefined}
          />
        );
      })}
    </div>
  );

  return (
    <div className="mx-auto max-w-6xl space-y-5 p-6">
      <MachineCard />

      <div className="flex items-center gap-3">
        <div className="relative w-96">
          <Search size={14} className="absolute top-1/2 left-3 -translate-y-1/2 text-mute" />
          <input
            autoFocus
            value={query}
            onChange={(e) => setQuery(e.target.value)}
            placeholder="Buscar: services, red, bios, impresoras, ncpa.cpl…"
            className="w-full rounded-md border border-line bg-panel py-1.5 pr-3 pl-8 text-sm text-ink outline-none placeholder:text-mute focus:border-neon/50"
          />
        </div>
        <span className="text-xs text-mute">
          {data.tools.length + data.custom.length} accesos · <ShieldCheck size={11} className="inline text-neon" /> = se abre como administrador
        </span>
        <div className="ml-auto">
          <Button onClick={() => setEditing("new")}>
            <Plus size={14} /> Nuevo acceso
          </Button>
        </div>
      </div>

      {results ? (
        <Section title={`Resultados (${results.length})`}>
          {results.length ? grid(results) : <p className="text-sm text-mute">Nada coincide con «{query}».</p>}
        </Section>
      ) : (
        <>
          {favorites.length > 0 && <Section title="Favoritos">{grid(favorites)}</Section>}
          <Section title="Mis accesos">
            {data.custom.length ? (
              grid(data.custom.map((c) => c.id))
            ) : (
              <button
                onClick={() => setEditing("new")}
                className="w-full rounded-lg border border-dashed border-line px-4 py-4 text-left text-sm text-mute transition-colors hover:border-neon/50 hover:text-ink"
              >
                <Plus size={14} className="mr-1.5 inline" />
                Añade tus propios programas, carpetas o páginas web (AnyDesk, tu carpeta de instaladores, el panel de tu router…).
                Marca con <Star size={11} className="inline" /> los que más uses para tenerlos arriba.
              </button>
            )}
          </Section>
          {GROUPS.map((g) => {
            const ids = data.tools.filter((t) => t.group === g.id).map((t) => t.id);
            return ids.length ? (
              <Section key={g.id} title={g.title}>
                {grid(ids)}
              </Section>
            ) : null;
          })}
        </>
      )}

      {editing && (
        <CustomEditor
          initial={editing === "new" ? null : editing}
          elevated={data.elevated}
          onClose={() => setEditing(null)}
          onSaved={() => {
            setEditing(null);
            load();
          }}
        />
      )}
      {dialog}
    </div>
  );
}

function Section({ title, children }: { title: string; children: React.ReactNode }) {
  return (
    <section>
      <h2 className="mb-2 text-[11px] font-semibold tracking-[0.14em] text-dim uppercase">{title}</h2>
      {children}
    </section>
  );
}

function ToolTile({
  tile,
  favorite,
  busy,
  onLaunch,
  onFavorite,
  onEdit,
  onDelete,
}: {
  tile: Tile;
  favorite: boolean;
  busy: boolean;
  onLaunch: () => void;
  onFavorite: () => void;
  onEdit?: () => void;
  onDelete?: () => void;
}) {
  const Icon = ICONS[tile.icon] ?? AppWindow;
  const off = !!tile.disabled;
  return (
    <div className={`group relative rounded-lg border border-line bg-panel transition-colors ${off ? "opacity-45" : "hover:border-neon/40 hover:bg-panel-2"}`}>
      <button
        onClick={onLaunch}
        disabled={off}
        title={tile.disabled ?? tile.description}
        className="flex w-full items-start gap-3 px-3 py-2.5 pr-8 text-left disabled:cursor-not-allowed"
      >
        <span className={`mt-0.5 grid size-8 shrink-0 place-items-center rounded-md border border-line bg-void/60 ${off ? "text-mute" : "text-neon"}`}>
          {busy ? <Loader2 size={15} className="animate-spin" /> : <Icon size={15} strokeWidth={1.8} />}
        </span>
        <span className="min-w-0 flex-1">
          <span className="flex items-center gap-1.5 text-[13px] font-medium text-ink">
            <span className="truncate">{tile.name}</span>
            {tile.asAdmin && <ShieldCheck size={11} className="shrink-0 text-neon" aria-label="Se abre como administrador" />}
          </span>
          <span className="line-clamp-2 text-[11px] leading-snug text-mute [overflow-wrap:anywhere]">{tile.disabled ?? tile.description}</span>
        </span>
      </button>
      <div className="absolute top-1.5 right-1.5 flex flex-col gap-0.5">
        <button
          onClick={onFavorite}
          title={favorite ? "Quitar de favoritos" : "Añadir a favoritos"}
          className={`rounded p-1 transition-opacity ${favorite ? "text-warn" : "text-mute opacity-0 group-hover:opacity-100 hover:text-ink"}`}
        >
          <Star size={12} fill={favorite ? "currentColor" : "none"} />
        </button>
        {onEdit && (
          <button onClick={onEdit} title="Editar" className="rounded p-1 text-mute opacity-0 group-hover:opacity-100 hover:text-ink">
            <Pencil size={12} />
          </button>
        )}
        {onDelete && (
          <button onClick={onDelete} title="Eliminar" className="rounded p-1 text-mute opacity-0 group-hover:opacity-100 hover:text-bad">
            <Trash2 size={12} />
          </button>
        )}
      </div>
    </div>
  );
}

function CustomEditor({
  initial,
  elevated,
  onClose,
  onSaved,
}: {
  initial: CustomTool | null;
  elevated: boolean;
  onClose: () => void;
  onSaved: () => void;
}) {
  const [t, setT] = useState<CustomTool>(initial ?? { id: "", name: "", kind: "program", target: "", args: "", icon: "" });
  const [error, setError] = useState<string | null>(null);
  const [saving, setSaving] = useState(false);

  const browse = async () => {
    try {
      const path = await toolboxApi.pickTarget(t.kind);
      if (path) {
        const base = path.split("\\").pop()?.replace(/\.(exe|bat|cmd|msc|cpl|lnk)$/i, "") ?? "";
        setT((x) => ({ ...x, target: path, name: x.name || base }));
      }
    } catch (e) {
      setError(String(e));
    }
  };

  const save = async () => {
    setSaving(true);
    setError(null);
    try {
      await toolboxApi.saveCustom(t);
      onSaved();
    } catch (e) {
      setError(String(e));
    } finally {
      setSaving(false);
    }
  };

  return (
    <Modal
      title={initial ? "Editar acceso" : "Nuevo acceso"}
      onClose={onClose}
      width="w-[560px]"
      footer={
        <>
          {error && <p className="mr-auto text-xs text-bad">{error}</p>}
          <Button kind="ghost" onClick={onClose}>
            Cancelar
          </Button>
          <Button onClick={save} disabled={saving || !t.name.trim() || !t.target.trim()}>
            Guardar
          </Button>
        </>
      }
    >
      <div className="space-y-3">
        <div className="flex gap-1 rounded-lg border border-line bg-void/60 p-1">
          {(Object.keys(KIND_LABEL) as CustomKind[]).map((k) => (
            <button
              key={k}
              onClick={() => setT({ ...t, kind: k, target: k === t.kind ? t.target : "" })}
              className={`flex-1 rounded-md py-1.5 text-sm transition-colors ${t.kind === k ? "bg-neon/10 text-neon" : "text-dim hover:text-ink"}`}
            >
              {KIND_LABEL[k]}
            </button>
          ))}
        </div>
        <label className="block">
          <span className="mb-1 block text-xs text-dim">{t.kind === "url" ? "Dirección" : t.kind === "folder" ? "Carpeta" : "Programa o archivo"}</span>
          <div className="flex gap-2">
            <input
              autoFocus
              value={t.target}
              onChange={(e) => setT({ ...t, target: e.target.value })}
              placeholder={t.kind === "url" ? "https://192.168.1.1" : t.kind === "folder" ? "D:\\Instaladores" : "C:\\Program Files\\AnyDesk\\AnyDesk.exe"}
              className={`${inputClass} font-mono text-xs`}
            />
            {t.kind !== "url" && (
              <Button kind="ghost" onClick={browse}>
                <FolderOpen size={14} /> Examinar…
              </Button>
            )}
          </div>
        </label>
        {t.kind === "program" && (
          <label className="block">
            <span className="mb-1 block text-xs text-dim">Argumentos (opcional)</span>
            <input value={t.args} onChange={(e) => setT({ ...t, args: e.target.value })} placeholder="/k ipconfig /all" className={`${inputClass} font-mono text-xs`} />
          </label>
        )}
        <label className="block">
          <span className="mb-1 block text-xs text-dim">Nombre</span>
          <input value={t.name} onChange={(e) => setT({ ...t, name: e.target.value })} maxLength={60} placeholder="AnyDesk" className={inputClass} />
        </label>
        <div>
          <span className="mb-1 block text-xs text-dim">Icono</span>
          <div className="flex flex-wrap gap-1">
            {CUSTOM_ICONS.map((n) => {
              const I = ICONS[n];
              const on = (t.icon || { program: "app", folder: "folder", url: "globe" }[t.kind]) === n;
              return (
                <button
                  key={n}
                  onClick={() => setT({ ...t, icon: n })}
                  className={`grid size-8 place-items-center rounded-md border ${on ? "border-neon/60 bg-neon/10 text-neon" : "border-line text-mute hover:text-ink"}`}
                >
                  <I size={15} />
                </button>
              );
            })}
          </div>
        </div>
        <p className="text-[11px] text-mute">
          {t.kind === "program"
            ? elevated
              ? "Los programas se abren con los mismos permisos que AdminOps (administrador)."
              : "Los programas se abren con los mismos permisos que AdminOps."
            : "Se abre con el Explorador del usuario, sin permisos de administrador."}{" "}
          Tus accesos se guardan con tus ajustes (y viajan en el USB en modo portable).
        </p>
      </div>
    </Modal>
  );
}

/** Ficha rápida del equipo: lo que el técnico suele apuntar o copiar. */
function MachineCard() {
  const [inv, setInv] = useState<Inventory | null>(null);
  const [error, setError] = useState<string | null>(null);
  const [reveal, setReveal] = useState(false);
  const toast = useToast();

  useEffect(() => {
    hwApi.inventory().then(setInv).catch((e) => setError(String(e)));
  }, []);

  const copy = (text: string, what: string) => navigator.clipboard.writeText(text).then(() => toast("ok", `${what} copiado al portapapeles.`));

  const sheet = inv
    ? [
        `Equipo: ${`${inv.manufacturer} ${inv.model}`.trim()}`,
        `Número de serie: ${inv.serial || "—"}`,
        `Windows: ${inv.os} ${inv.osVersion} (build ${inv.osBuild}, ${inv.architecture})`,
        `Clave de Windows (OEM): ${inv.productKey ?? "no hay clave en el firmware"}`,
        `Procesador: ${inv.cpu}`,
        `Memoria: ${Math.round(inv.ramTotal / 1024 ** 3)} GB`,
        `Placa base: ${inv.boardManufacturer} ${inv.boardProduct} · BIOS ${inv.biosVersion}`,
      ].join("\n")
    : "";

  const secret = (value: string | null, mask: string) =>
    value ? (
      <span className="inline-flex items-center gap-1.5 font-mono text-xs">
        {reveal ? value : mask}
        <button onClick={() => copy(value, "Dato")} className="text-mute hover:text-ink" title="Copiar">
          <Copy size={11} />
        </button>
      </span>
    ) : (
      <span className="text-xs text-mute">—</span>
    );

  return (
    <Card
      title="Ficha del equipo"
      icon={<Monitor size={14} />}
      right={
        inv && (
          <div className="flex items-center gap-3 text-[11px]">
            <button onClick={() => setReveal(!reveal)} className="flex items-center gap-1 text-mute hover:text-ink">
              {reveal ? <EyeOff size={11} /> : <Eye size={11} />} {reveal ? "Ocultar" : "Mostrar"} datos
            </button>
            <button onClick={() => copy(sheet, "Ficha")} className="flex items-center gap-1 text-neon hover:underline">
              <Copy size={11} /> Copiar ficha
            </button>
          </div>
        )
      }
    >
      {error ? (
        <p className="text-sm text-bad">{error}</p>
      ) : !inv ? (
        <p className="flex items-center gap-2 text-sm text-mute">
          <Loader2 size={13} className="animate-spin" /> Leyendo el equipo…
        </p>
      ) : (
        <div className="grid grid-cols-2 gap-x-6 gap-y-2 text-sm lg:grid-cols-4">
          <Field label="Equipo">{`${inv.manufacturer} ${inv.model}`.trim() || "—"}</Field>
          <Field label="Número de serie">{secret(inv.serial || null, "••••••••")}</Field>
          <Field label="Windows">
            {inv.os.replace(/^Microsoft /, "")} <span className="font-mono text-xs text-mute">{inv.osBuild}</span>
          </Field>
          <Field label="Clave OEM (firmware)">{secret(inv.productKey, "•••••-•••••-•••••-•••••-•••••")}</Field>
        </div>
      )}
    </Card>
  );
}

function Field({ label, children }: { label: string; children: React.ReactNode }) {
  return (
    <div className="min-w-0">
      <div className="text-[10px] tracking-widest text-mute uppercase">{label}</div>
      <div className="truncate text-ink">{children}</div>
    </div>
  );
}
