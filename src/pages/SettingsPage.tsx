import {
  ArrowDown,
  ArrowUp,
  BadgeCheck,
  Download,
  ImagePlus,
  PenLine,
  Plus,
  Receipt,
  Save,
  Trash2,
  Upload,
  X,
} from "lucide-react";
import {
  createContext,
  useCallback,
  useContext,
  useEffect,
  useState,
} from "react";
import type { PageId } from "../components/Sidebar";
import logo from "../assets/logo.svg";
import { useToast } from "../components/feedback";
import { SignaturePad } from "../components/service";
import { NAV } from "../components/Sidebar";
import { Button, Card, inputClass, Loading } from "../components/ui";
import { PerfPanel } from "../components/PerfPanel";
import {
  appApi,
  appcareApi,
  configApi,
  workApi,
  type AppInfo,
  type DataUsage,
  type Settings,
} from "../lib/api";
import { bytes } from "../lib/format";
import {
  ACCENTS,
  applyAppearance,
  exportPrefs,
  importPrefs,
  setPrefs,
  usePrefs,
  ZOOMS,
  type Accent,
} from "../lib/prefs";
import { getTheme, setTheme, type Theme } from "../lib/theme";
import { DataSafety } from "./settings/DataSafety";
import { Search } from "lucide-react";
import { PortalSettings } from "./settings/Portals";
import {
  findSettings,
  SECTIONS,
  type SettingsSection,
} from "./settings/catalog";
import { LockSettings } from "./settings/LockSettings";
import { VisitTypesEditor } from "./settings/VisitTypesEditor";
import { NavEditor } from "./settings/NavEditor";
import { ShortcutEditor } from "./settings/ShortcutEditor";

type Tab = SettingsSection;
const TAB_KEY = "adminops.settingsTab";

function readTab(): Tab {
  try {
    const t = localStorage.getItem(TAB_KEY);
    if (SECTIONS.some((x) => x.id === t)) return t as Tab;
  } catch {
    /* sin almacenamiento */
  }
  return "general";
}

export function SettingsPage({
  appInfo,
  onNavigate,
}: {
  appInfo: AppInfo | null;
  onNavigate: (p: PageId) => void;
}) {
  const [s, setS] = useState<Settings | null>(null);
  const [dirty, setDirty] = useState(false);
  const [query, setQuery] = useState("");
  const [highlight, setHighlight] = useState<string | null>(null);

  const [tab, setTab] = useState<Tab>(readTab);
  const toast = useToast();
  // Lleva la vista a la fila buscada en cuanto la sección la pinta.
  useEffect(() => {
    if (!highlight) return;
    const t = window.setTimeout(() => {
      document
        .querySelector(`[data-setting="${CSS.escape(highlight)}"]`)
        ?.scrollIntoView({ behavior: "smooth", block: "center" });
    }, 60);
    const clear = window.setTimeout(() => setHighlight(null), 3000);
    return () => {
      window.clearTimeout(t);
      window.clearTimeout(clear);
    };
  }, [highlight, tab]);

  useEffect(() => {
    workApi.settings().then(setS);
  }, []);

  const pickTab = (t: Tab) => {
    setTab(t);
    try {
      localStorage.setItem(TAB_KEY, t);
    } catch {
      /* sin almacenamiento */
    }
  };

  if (!s) return <Loading page />;

  const set = (patch: Partial<Settings>) => {
    setS({ ...s, ...patch });
    setDirty(true);
  };

  const save = async () => {
    try {
      await workApi.saveSettings(s);
      setDirty(false);
      toast("ok", "Ajustes guardados.");
    } catch (e) {
      toast("error", String(e));
    }
  };

  const current = SECTIONS.find((x) => x.id === tab) ?? SECTIONS[0];
  const results = findSettings(query);

  // Al llegar desde el buscador, la fila se resalta unos segundos.
  const goTo = (e: { section: Tab; title: string }) => {
    setQuery("");
    pickTab(e.section);
    setHighlight(e.title);
  };

  return (
    <div className="mx-auto max-w-6xl p-6">
      <div className="mb-5">
        <div className="relative">
          <Search
            size={15}
            className="absolute top-1/2 left-3 -translate-y-1/2 text-mute"
          />
          <input
            value={query}
            onChange={(e) => setQuery(e.target.value)}
            onKeyDown={(e) => {
              if (e.key === "Enter" && results[0]) goTo(results[0]);
              if (e.key === "Escape") setQuery("");
            }}
            placeholder="Buscar un ajuste: diagnóstico, contraseña, logo, zoom…"
            className="h-10 w-full rounded-lg border border-line bg-panel pr-3 pl-9 text-sm text-ink outline-none placeholder:text-mute focus:border-neon/50"
          />
          {query && (
            <button
              onClick={() => setQuery("")}
              className="absolute top-1/2 right-3 -translate-y-1/2 text-mute hover:text-ink"
              title="Borrar"
            >
              <X size={14} />
            </button>
          )}
        </div>
        {query.trim() && (
          <div className="mt-2 overflow-hidden rounded-lg border border-line bg-panel">
            {results.length === 0 ? (
              <p className="px-4 py-3 text-sm text-mute">
                Ningún ajuste coincide con «{query}».
              </p>
            ) : (
              results.map((e) => (
                <button
                  key={`${e.section}-${e.title}`}
                  onClick={() => goTo(e)}
                  className="flex w-full items-center gap-3 border-b border-line/60 px-4 py-2 text-left last:border-b-0 hover:bg-panel-2"
                >
                  <span className="flex-1 truncate text-sm text-ink">
                    {e.title}
                  </span>
                  <span className="shrink-0 text-xs text-mute">
                    {SECTIONS.find((x) => x.id === e.section)?.label}
                  </span>
                </button>
              ))
            )}
          </div>
        )}
      </div>

      <div className="grid grid-cols-12 gap-6">
        <nav
          aria-label="Secciones de ajustes"
          className="col-span-12 lg:col-span-3 lg:self-start"
        >
          <ul className="flex gap-1 overflow-x-auto lg:flex-col lg:overflow-visible">
            {SECTIONS.map((x) => {
              const Icon = x.icon;
              const on = tab === x.id;
              return (
                <li key={x.id} className="shrink-0 lg:shrink">
                  <button
                    onClick={() => pickTab(x.id)}
                    aria-current={on ? "page" : undefined}
                    className={`flex w-full items-center gap-2.5 rounded-lg px-3 py-2 text-left text-sm transition-colors ${
                      on
                        ? "bg-neon/10 font-medium text-neon"
                        : "text-dim hover:bg-panel-2 hover:text-ink"
                    }`}
                  >
                    <Icon size={16} strokeWidth={1.7} className="shrink-0" />
                    <span className="truncate">{x.label}</span>
                  </button>
                </li>
              );
            })}
          </ul>
        </nav>

        <HighlightCtx.Provider value={highlight}>
          <div className="col-span-12 min-w-0 lg:col-span-9">
            <header className="mb-4">
              <h2 className="text-lg font-semibold text-ink">
                {current.label}
              </h2>
              <p className="text-xs text-mute">{current.hint}</p>
            </header>

            {tab === "general" && (
              <General
                s={s}
                set={set}
                portable={!!appInfo?.portable}
                onImported={() => workApi.settings().then(setS)}
              />
            )}
            {tab === "appearance" && <Appearance />}
            {tab === "navigation" && (
              <div className="space-y-4">
                <ShortcutEditor />
                <NavEditor />
              </div>
            )}
            {tab === "security" && <LockSettings />}
            {tab === "portals" && (
              <PortalSettings
                s={s}
                set={set}
                onNavigate={onNavigate}
                Row={Row}
              />
            )}
            {tab === "reports" && <Reports s={s} set={set} />}
            {tab === "performance" && <PerfPanel />}
            {tab === "about" && <About appInfo={appInfo} />}
          </div>
        </HighlightCtx.Provider>
      </div>

      {dirty && (
        <div className="sticky bottom-0 z-30 -mx-6 -mb-6 mt-6 border-t border-line bg-panel px-6 py-3">
          <div className="mx-auto flex max-w-6xl items-center justify-between">
            <span className="text-sm text-warn">Cambios sin guardar</span>
            <Button onClick={save}>
              <Save size={14} /> Guardar ajustes
            </Button>
          </div>
        </div>
      )}
    </div>
  );
}

type SettingsProps = { s: Settings; set: (patch: Partial<Settings>) => void };

/** Ajuste al que ha llevado el buscador (se resalta un momento). */
const HighlightCtx = createContext<string | null>(null);

function Row({
  title,
  sub,
  children,
}: {
  title: string;
  sub?: string;
  children: React.ReactNode;
}) {
  const highlighted = useContext(HighlightCtx) === title;
  return (
    <div
      data-setting={title}
      className={`flex flex-wrap items-center gap-4 border-t border-line/60 py-3 first:border-t-0 first:pt-0 ${
        highlighted
          ? "-mx-2 rounded-lg bg-neon/10 px-2 ring-1 ring-neon/40"
          : ""
      }`}
    >
      <div className="min-w-0 flex-1">
        <div className="text-sm text-ink">{title}</div>
        {sub && <div className="text-xs text-mute">{sub}</div>}
      </div>
      <div className="shrink-0">{children}</div>
    </div>
  );
}

const selectClass =
  "rounded-md border border-line bg-void/60 px-3 py-1.5 text-sm text-ink outline-none focus:border-neon/50";

function General({
  s,
  set,
  portable,
  onImported,
}: SettingsProps & { portable: boolean; onImported: () => void }) {
  const prefs = usePrefs();
  const toast = useToast();
  const pages = NAV.filter((n) => n.id !== "settings");

  const doExport = async () => {
    try {
      const p = await configApi.export(exportPrefs());
      if (p) toast("ok", "Configuración exportada.");
    } catch (e) {
      toast("error", String(e));
    }
  };
  const doImport = async () => {
    try {
      const prefsIn = await configApi.import();
      if (prefsIn === null) return;
      importPrefs(prefsIn);
      onImported();
      window.dispatchEvent(new Event("adminops-lock"));
      toast("ok", "Configuración importada.");
    } catch (e) {
      toast("error", String(e));
    }
  };

  return (
    <div className="space-y-4">
      <Card title="Inicio y actualización">
        <Row
          title="Página al abrir AdminOps"
          sub="La que se muestra al arrancar."
        >
          <select
            value={prefs.startPage}
            onChange={(e) =>
              setPrefs({ startPage: e.target.value as typeof prefs.startPage })
            }
            className={selectClass}
          >
            <option value="last">La última que usé</option>
            {pages.map((p) => (
              <option key={p.id} value={p.id}>
                {p.label}
              </option>
            ))}
          </select>
        </Row>
        <Row
          title="Actualización del Panel"
          sub="Cada cuánto se refrescan CPU, memoria, red y procesos. Menos frecuente = menos consumo."
        >
          <select
            value={prefs.refreshMs}
            onChange={(e) => setPrefs({ refreshMs: Number(e.target.value) })}
            className={selectClass}
          >
            <option value={1000}>Cada segundo</option>
            <option value={2000}>Cada 2 segundos</option>
            <option value={5000}>Cada 5 segundos</option>
          </select>
        </Row>
        <Row
          title="Precargar los portales"
          sub="Carga en segundo plano el último portal usado de Tickets, Inventario web y Correo, para que al entrar ya esté listo. Los de sesión privada no se precargan."
        >
          <input
            type="checkbox"
            checked={prefs.preloadPortals}
            onChange={(e) => setPrefs({ preloadPortals: e.target.checked })}
            className="size-4 accent-[var(--color-neon)]"
          />
        </Row>
        <Row
          title="Diagnosticar al abrir AdminOps"
          sub="Analiza el equipo en segundo plano nada más abrir: al entrar en Diagnóstico ya está hecho o a medias."
        >
          <input
            type="checkbox"
            checked={prefs.diagnoseOnOpen}
            onChange={(e) => setPrefs({ diagnoseOnOpen: e.target.checked })}
            className="size-4 accent-[var(--color-neon)]"
          />
        </Row>
        <Row
          title="Vigilar errores de Windows"
          sub="Mientras AdminOps está abierta, revisa el Visor de eventos cada minuto y avisa (campana de arriba y notificación) de pantallazos, discos con fallos, programas que se cierran, falta de memoria o espacio…"
        >
          <input
            type="checkbox"
            checked={s.watchWindows}
            onChange={(e) => set({ watchWindows: e.target.checked })}
            className="size-4 accent-[var(--color-neon)]"
          />
        </Row>
        <Row
          title="Avisar al terminar tareas largas"
          sub="Notificación de Windows cuando una tarea de más de 20 s acaba con AdminOps en segundo plano."
        >
          <input
            type="checkbox"
            checked={s.notifyTasks}
            onChange={(e) => set({ notifyTasks: e.target.checked })}
            className="size-4 accent-[var(--color-neon)]"
          />
        </Row>
      </Card>

      <Card title="Cambios en el sistema">
        <Row
          title="Punto de restauración antes de cambiar el sistema"
          sub="Permite volver atrás con Restaurar sistema si algo sale mal. Crear uno tarda 1–2 minutos (como mucho uno cada 30 min)."
        >
          <select
            value={s.restorePoints}
            onChange={(e) =>
              set({
                restorePoints: e.target.value as Settings["restorePoints"],
              })
            }
            className={selectClass}
          >
            <option value="risky">Solo antes de cambios con riesgo</option>
            <option value="always">Antes de cualquier cambio</option>
            <option value="never">Nunca (no recomendado)</option>
          </select>
        </Row>
        <Autostart />
      </Card>

      <WhereStored portable={portable} />

      <DataSafety />

      <DataCare s={s} set={set} />

      <Card title="Red y dominio">
        <label className="block">
          <span className="mb-1 block text-xs text-dim">
            Dominio habitual (se propone al unir equipos)
          </span>
          <input
            value={s.defaultDomain}
            onChange={(e) => set({ defaultDomain: e.target.value })}
            placeholder="p. ej. empresa.local"
            className={inputClass}
          />
        </label>
      </Card>

      <Card title="Copia de la configuración">
        <p className="mb-3 text-sm text-dim">
          Guarda en un archivo tus ajustes (marca, precios, checklist, firma),
          los portales de Tickets y las preferencias de la interfaz, para
          llevarlos a otro equipo o recuperarlos. No incluye contraseñas ni
          clientes.
        </p>
        <div className="flex gap-2">
          <Button kind="ghost" onClick={doExport}>
            <Download size={14} /> Exportar
          </Button>
          <Button kind="ghost" onClick={doImport}>
            <Upload size={14} /> Importar
          </Button>
        </div>
      </Card>
    </div>
  );
}

/** Qué se guarda una vez para todos los equipos y qué es de cada equipo. */
function WhereStored({ portable }: { portable: boolean }) {
  const travels = [
    "Tus ajustes, marca, precios, checklist y firma",
    "Portales de Tickets (la dirección; la sesión iniciada puede pedirte entrar otra vez en otro equipo, porque Windows la protege por equipo)",
    "Accesos a routers, reconocidos por red (con su contraseña cifrada)",
    "Contactos, clientes, conexiones de acceso remoto y perfiles",
    "Apariencia, navegación, atajos y favoritos",
    "Nombres que pongas a dispositivos de la red",
    "Informes PDF (todos juntos)",
  ];
  const perPc = [
    "Diario de cambios y «Deshacer» (solo sirven en ese equipo)",
    "Diagnósticos y su comparación",
    "Avisos de Windows y línea de tiempo",
    "Sesión de servicio en curso",
    "Pruebas de velocidad y registro técnico",
    "Posición y tamaño de la ventana",
  ];
  return (
    <Card
      title={
        portable
          ? "Qué viaja en el USB y qué se queda por equipo"
          : "Qué se comparte y qué es de cada equipo"
      }
    >
      <p className="mb-3 text-sm text-dim">
        {portable
          ? "Con el portable, todo se guarda en la carpeta AdminOps-data del USB, sin dejar nada en el equipo del cliente. Lo tuyo se configura una vez y te acompaña; lo de cada equipo se separa por su nombre para no mezclarse."
          : "Instalada, los datos están en este equipo. Con la versión portable en un USB, lo de la izquierda te acompaña a todos los equipos."}
      </p>
      <div className="grid gap-4 md:grid-cols-2">
        <div>
          <p className="mb-1.5 text-xs font-medium text-ok">
            {portable
              ? "Viaja contigo (igual en todos)"
              : "Tuyo (igual en todos, con el portable)"}
          </p>
          <ul className="space-y-1 text-xs text-dim">
            {travels.map((t) => (
              <li key={t}>· {t}</li>
            ))}
          </ul>
        </div>
        <div>
          <p className="mb-1.5 text-xs font-medium text-neon">
            Propio de cada equipo
          </p>
          <ul className="space-y-1 text-xs text-dim">
            {perPc.map((t) => (
              <li key={t}>· {t}</li>
            ))}
          </ul>
        </div>
      </div>
      {portable && (
        <p className="mt-3 text-xs text-mute">
          Las contraseñas guardadas van cifradas con una clave del propio USB:
          funcionan en cualquier equipo, pero no en otro USB. Si pierdes el USB
          se pierde todo lo anterior: exporta de vez en cuando la configuración
          y los contactos (CSV), y activa el bloqueo con PIN en Seguridad.
        </p>
      )}
    </Card>
  );
}

function Autostart() {
  const [on, setOn] = useState<boolean | null>(null);
  const toast = useToast();
  useEffect(() => {
    appcareApi
      .autostart()
      .then(setOn)
      .catch(() => setOn(false));
  }, []);
  const toggle = async (v: boolean) => {
    try {
      await appcareApi.setAutostart(v);
      setOn(v);
      toast(
        "ok",
        v
          ? "AdminOps se abrirá minimizada al iniciar sesión."
          : "AdminOps ya no se abre al iniciar sesión.",
      );
    } catch (e) {
      toast("error", String(e));
    }
  };
  return (
    <Row
      title="Abrir AdminOps al iniciar Windows"
      sub="Minimizada en la barra de tareas, sin pedir permiso de administrador cada vez. En modo portable, si mueves la carpeta, vuelve a activarlo."
    >
      <input
        type="checkbox"
        checked={!!on}
        disabled={on === null}
        onChange={(e) => toggle(e.target.checked)}
        className="size-4 accent-[var(--color-neon)]"
      />
    </Row>
  );
}

function DataCare({ s, set }: SettingsProps) {
  const [usage, setUsage] = useState<DataUsage | null>(null);
  const [months, setMonths] = useState(6);
  const [parts, setParts] = useState({
    journal: true,
    snapshots: true,
    reports: false,
  });
  const [busy, setBusy] = useState(false);
  const [update, setUpdate] = useState<string | null>(null);
  const toast = useToast();
  const load = useCallback(() => {
    appcareApi
      .usage()
      .then(setUsage)
      .catch(() => {});
  }, []);
  useEffect(load, [load]);

  const clean = async () => {
    setBusy(true);
    try {
      const r = await appcareApi.cleanup(
        months,
        parts.journal,
        parts.snapshots,
        parts.reports,
      );
      toast(
        "ok",
        `Limpieza hecha: ${r.journal} entradas del historial, ${r.snapshots} análisis y ${r.reports} informes (${bytes(r.freed)}).`,
      );
      load();
    } catch (e) {
      toast("error", String(e));
    } finally {
      setBusy(false);
    }
  };

  const checkNow = async () => {
    setUpdate("Buscando…");
    try {
      const u = await appcareApi.checkUpdate();
      setUpdate(
        u.newer
          ? `Hay una versión nueva: ${u.latest}. Aparece en la barra lateral.`
          : `Tienes la última versión (${u.current}).`,
      );
      if (u.newer) appcareApi.openRelease(u.url).catch(() => {});
    } catch (e) {
      setUpdate(String(e));
    }
  };

  const check = (key: keyof typeof parts, label: string) => (
    <label className="flex items-center gap-2 text-sm text-dim">
      <input
        type="checkbox"
        checked={parts[key]}
        onChange={(e) => setParts({ ...parts, [key]: e.target.checked })}
        className="size-4 accent-[var(--color-neon)]"
      />
      {label}
    </label>
  );

  return (
    <>
      <Card title="Datos de AdminOps">
        {usage && (
          <div className="mb-4 grid grid-cols-2 gap-x-6 gap-y-1 text-sm md:grid-cols-4">
            <span className="text-dim">
              Historial:{" "}
              <span className="text-ink">{usage.journalEntries} entradas</span>
            </span>
            <span className="text-dim">
              Análisis:{" "}
              <span className="text-ink">
                {usage.snapshots} · {bytes(usage.snapshotsBytes)}
              </span>
            </span>
            <span className="text-dim">
              Informes:{" "}
              <span className="text-ink">
                {usage.reports} · {bytes(usage.reportsBytes)}
              </span>
            </span>
            <span className="text-dim">
              Registros:{" "}
              <span className="text-ink">{bytes(usage.logsBytes)}</span>
            </span>
          </div>
        )}
        <div className="flex flex-wrap items-center gap-3">
          <span className="text-sm text-dim">Borrar lo que tenga más de</span>
          <select
            value={months}
            onChange={(e) => setMonths(Number(e.target.value))}
            className={selectClass}
          >
            {[3, 6, 12, 24].map((m) => (
              <option key={m} value={m}>
                {m} meses
              </option>
            ))}
          </select>
          {check("journal", "Historial")}
          {check("snapshots", "Análisis")}
          {check("reports", "Informes (a la papelera)")}
          <Button
            kind="ghost"
            onClick={clean}
            disabled={
              busy || (!parts.journal && !parts.snapshots && !parts.reports)
            }
          >
            Limpiar ahora
          </Button>
        </div>
        <p className="mt-2 text-[11px] text-mute">
          Nunca se borran los ajustes aplicados que aún se pueden deshacer, el
          último análisis ni el de una sesión en curso. Los informes van a la
          papelera.
        </p>
        <div className="mt-3 border-t border-line/60 pt-3">
          <Row
            title="Limpieza automática al abrir"
            sub="Una vez al día como mucho: historial, análisis e informes más antiguos que lo elegido."
          >
            <select
              value={s.autoCleanupMonths}
              onChange={(e) =>
                set({ autoCleanupMonths: Number(e.target.value) })
              }
              className={selectClass}
            >
              <option value={0}>Desactivada</option>
              {[6, 12, 24].map((m) => (
                <option key={m} value={m}>
                  Más de {m} meses
                </option>
              ))}
            </select>
          </Row>
        </div>
      </Card>

      <Card title="Actualizaciones">
        <Row
          title="Avisar de versiones nuevas"
          sub="Al abrir, consulta en GitHub si hay una versión nueva de AdminOps y lo indica en la barra lateral. No descarga ni instala nada solo."
        >
          <input
            type="checkbox"
            checked={s.checkUpdates}
            onChange={(e) => set({ checkUpdates: e.target.checked })}
            className="size-4 accent-[var(--color-neon)]"
          />
        </Row>
        <div className="flex items-center gap-3 pt-1">
          <Button kind="ghost" onClick={checkNow}>
            Buscar ahora
          </Button>
          {update && <span className="text-xs text-dim">{update}</span>}
        </div>
      </Card>
    </>
  );
}

function Appearance() {
  const prefs = usePrefs();
  const [theme, setThemeState] = useState<Theme>(getTheme);
  const pick = (t: Theme) => {
    setTheme(t);
    setThemeState(t);
    applyAppearance();
  };
  const option = (
    t: Theme,
    title: string,
    sub: string,
    bg: string,
    bar: string,
  ) => (
    <button
      onClick={() => pick(t)}
      aria-pressed={theme === t}
      className={`flex flex-1 items-center gap-3 rounded-lg border p-3 text-left transition-colors ${theme === t ? "border-neon" : "border-line hover:border-line-2"}`}
    >
      <span
        className="flex h-10 w-14 shrink-0 flex-col justify-end gap-1 rounded-md border border-line-2 p-1.5"
        style={{ background: bg }}
      >
        <span className="h-1 w-8 rounded-full" style={{ background: bar }} />
        <span
          className="h-1 w-5 rounded-full"
          style={{ background: bar, opacity: 0.5 }}
        />
      </span>
      <span>
        <span className="block text-sm font-medium text-ink">{title}</span>
        <span className="block text-xs text-mute">{sub}</span>
      </span>
    </button>
  );
  return (
    <div className="space-y-4">
      <Card title="Tema">
        <div className="flex gap-3">
          {option(
            "dark",
            "Oscuro",
            "Menos brillo en talleres y de noche",
            "#111315",
            "#a5acb5",
          )}
          {option(
            "light",
            "Claro",
            "Más legible con mucha luz y en oficinas",
            "#f6f6f4",
            "#4b5058",
          )}
        </div>
      </Card>
      <Card title="Color de acento">
        <div className="flex flex-wrap gap-2">
          {(Object.keys(ACCENTS) as Accent[]).map((a) => (
            <button
              key={a}
              onClick={() => setPrefs({ accent: a })}
              aria-pressed={prefs.accent === a}
              className={`flex items-center gap-2 rounded-lg border px-3 py-2 text-sm ${prefs.accent === a ? "border-neon text-ink" : "border-line text-dim hover:border-line-2"}`}
            >
              <span
                className="size-4 rounded-full"
                style={{
                  background:
                    theme === "light" ? ACCENTS[a].light : ACCENTS[a].dark,
                }}
              />
              {ACCENTS[a].label}
            </button>
          ))}
        </div>
        <p className="mt-2 text-[11px] text-mute">
          Se usa en la selección, los botones principales y los enlaces. Los
          estados (bien, aviso, error) no cambian.
        </p>
      </Card>
      <Card title="Tamaño y movimiento">
        <Row
          title="Tamaño de la interfaz"
          sub="Textos, botones y espacios, todo a la vez."
        >
          <div className="inline-flex rounded-lg border border-line bg-void p-0.5">
            {ZOOMS.map((z) => (
              <button
                key={z.value}
                onClick={() => setPrefs({ zoom: z.value })}
                className={`rounded-md px-3 py-1.5 text-[13px] ${prefs.zoom === z.value ? "bg-panel-2 font-medium text-ink" : "text-dim hover:text-ink"}`}
              >
                {z.label}
              </button>
            ))}
          </div>
        </Row>
        <Row
          title="Reducir animaciones"
          sub="Quita transiciones y giros (más cómodo si marean o en equipos lentos)."
        >
          <input
            type="checkbox"
            checked={prefs.reduceMotion}
            onChange={(e) => setPrefs({ reduceMotion: e.target.checked })}
            className="size-4 accent-[var(--color-neon)]"
          />
        </Row>
      </Card>
    </div>
  );
}

function Reports({ s, set }: SettingsProps) {
  const [newItem, setNewItem] = useState("");
  const toast = useToast();

  const pickLogo = (file: File | undefined) => {
    if (!file) return;
    if (file.size > 500 * 1024) {
      toast("error", "El logo debe pesar menos de 500 KB.");
      return;
    }
    const r = new FileReader();
    r.onload = () => set({ logo: String(r.result) });
    r.readAsDataURL(file);
  };

  const move = (i: number, d: number) => {
    const list = [...s.checklist];
    const j = i + d;
    if (j < 0 || j >= list.length) return;
    [list[i], list[j]] = [list[j], list[i]];
    set({ checklist: list });
  };

  const input = inputClass;
  const numField = (
    key:
      | "taxRate"
      | "laborWarrantyDays"
      | "maintenanceMonths"
      | "quoteValidityDays",
    label: string,
    max: number,
  ) => (
    <label className="block">
      <span className="mb-1 block truncate text-xs text-dim" title={label}>
        {label}
      </span>
      <input
        type="number"
        min={0}
        max={max}
        step={key === "taxRate" ? "any" : 1}
        value={s[key]}
        onChange={(e) =>
          set({
            [key]: Math.min(max, Math.max(0, Number(e.target.value) || 0)),
          })
        }
        className={input}
      />
    </label>
  );
  const setItem = (i: number, patch: Partial<Settings["catalog"][number]>) =>
    set({
      catalog: s.catalog.map((c, j) => (j === i ? { ...c, ...patch } : c)),
    });
  const field = (key: keyof Settings, label: string, placeholder = "") => (
    <label className="block">
      <span className="mb-1 block text-xs text-dim">{label}</span>
      <input
        value={s[key] as string}
        onChange={(e) => set({ [key]: e.target.value })}
        placeholder={placeholder}
        className={input}
      />
    </label>
  );

  return (
    <div className="grid grid-cols-12 gap-4">
      <Card
        title="Tu marca en los informes"
        className="col-span-12 lg:col-span-7"
      >
        <div className="grid grid-cols-2 gap-3">
          {field("technician", "Técnico", "Tu nombre")}
          {field("company", "Empresa o marca", "Opcional")}
          {field("phone", "Teléfono")}
          {field("email", "Correo")}
          <div className="col-span-2">{field("website", "Web o redes")}</div>
          <label className="col-span-2 block">
            <span className="mb-1 block text-xs text-dim">
              Condiciones / garantía (pie del informe)
            </span>
            <textarea
              value={s.conditions}
              onChange={(e) => set({ conditions: e.target.value })}
              rows={3}
              placeholder="Ej.: Garantía de 30 días sobre el trabajo realizado. No incluye daños por software de terceros."
              className={`${input} resize-y`}
            />
          </label>
        </div>
      </Card>

      <Card title="Logo" className="col-span-12 lg:col-span-5">
        <div className="flex items-center gap-4">
          <div className="grid size-24 shrink-0 place-items-center rounded-xl border border-line bg-white">
            <img
              src={s.logo ?? logo}
              alt=""
              className="max-h-20 max-w-20 object-contain"
            />
          </div>
          <div className="space-y-2 text-xs">
            <label className="flex cursor-pointer items-center gap-1.5 rounded-md border border-neon/50 px-3 py-1.5 text-neon hover:bg-neon/10">
              <ImagePlus size={13} /> Elegir imagen
              <input
                type="file"
                accept="image/png,image/jpeg,image/webp,image/svg+xml"
                className="hidden"
                onChange={(e) => pickLogo(e.target.files?.[0])}
              />
            </label>
            {s.logo && (
              <button
                onClick={() => set({ logo: null })}
                className="flex items-center gap-1 text-mute hover:text-bad"
              >
                <X size={12} /> Quitar (usar el de AdminOps)
              </button>
            )}
            <p className="text-mute">
              PNG, JPG, WebP o SVG · máx. 500 KB. Aparece en la cabecera del
              informe.
            </p>
          </div>
        </div>
      </Card>

      <Card title="Checklist de servicio" className="col-span-12 lg:col-span-7">
        <ul className="mb-3 space-y-1">
          {s.checklist.map((item, i) => (
            <li
              key={i}
              className="group flex items-center gap-2 rounded-md px-2 py-1 hover:bg-panel-2"
            >
              <span className="w-5 font-mono text-[11px] text-mute">
                {i + 1}
              </span>
              <input
                value={item}
                onChange={(e) =>
                  set({
                    checklist: s.checklist.map((x, j) =>
                      j === i ? e.target.value : x,
                    ),
                  })
                }
                className="flex-1 bg-transparent text-sm text-ink outline-none"
              />
              <div className="flex gap-1 opacity-0 group-hover:opacity-100">
                <button
                  onClick={() => move(i, -1)}
                  className="text-mute hover:text-ink"
                >
                  <ArrowUp size={12} />
                </button>
                <button
                  onClick={() => move(i, 1)}
                  className="text-mute hover:text-ink"
                >
                  <ArrowDown size={12} />
                </button>
                <button
                  onClick={() =>
                    set({ checklist: s.checklist.filter((_, j) => j !== i) })
                  }
                  className="text-mute hover:text-bad"
                >
                  <Trash2 size={12} />
                </button>
              </div>
            </li>
          ))}
        </ul>
        <form
          className="flex gap-2"
          onSubmit={(e) => {
            e.preventDefault();
            if (!newItem.trim()) return;
            set({ checklist: [...s.checklist, newItem.trim()] });
            setNewItem("");
          }}
        >
          <input
            value={newItem}
            onChange={(e) => setNewItem(e.target.value)}
            placeholder="Nuevo punto…"
            className={input}
          />
          <button
            type="submit"
            className="flex items-center gap-1 rounded-md border border-line-2 px-3 text-xs text-dim hover:text-ink"
          >
            <Plus size={12} /> Añadir
          </button>
        </form>
        <p className="mt-2 text-[11px] text-mute">
          Se copia en cada sesión nueva; las sesiones en curso no cambian.
        </p>
      </Card>

      <Card
        title="Tu firma"
        icon={<PenLine size={14} />}
        className="col-span-12 lg:col-span-5"
      >
        <SignaturePad
          value={s.techSignature}
          onChange={(techSignature) => set({ techSignature })}
          height={130}
        />
        <p className="mt-1 text-[11px] text-mute">
          Aparece sobre tu nombre en todos los informes. Opcional.
        </p>
      </Card>

      <VisitTypesEditor s={s} set={set} />

      <Card
        title="Presupuestos, recibos y garantías"
        icon={<Receipt size={14} />}
        className="col-span-12"
      >
        <div className="grid grid-cols-2 gap-3 md:grid-cols-6">
          <label className="block">
            <span className="mb-1 block text-xs text-dim">Moneda</span>
            <input
              value={s.currency}
              onChange={(e) => set({ currency: e.target.value })}
              placeholder="RD$"
              className={input}
            />
          </label>
          <label className="block">
            <span className="mb-1 block text-xs text-dim">Impuesto</span>
            <input
              value={s.taxName}
              onChange={(e) => set({ taxName: e.target.value })}
              placeholder="ITBIS"
              className={input}
            />
          </label>
          {numField("taxRate", "Porcentaje (0: sin impuesto)", 100)}
          {numField("laborWarrantyDays", "Garantía mano de obra (días)", 3650)}
          {numField("maintenanceMonths", "Mantenimiento cada (meses)", 60)}
          {numField("quoteValidityDays", "Validez presupuesto (días)", 365)}
        </div>

        <h4 className="mt-5 mb-2 text-xs font-medium text-dim">
          Catálogo de servicios y piezas
        </h4>
        <table className="w-full text-[13px]">
          <thead>
            <tr className="text-left text-xs text-mute">
              <th className="pb-1.5 font-medium">Nombre</th>
              <th className="w-32 pb-1.5 font-medium">Precio</th>
              <th className="w-14 pb-1.5 text-center font-medium">Pieza</th>
              <th className="w-28 pb-1.5 font-medium">Garantía (días)</th>
              <th className="w-8" />
            </tr>
          </thead>
          <tbody>
            {s.catalog.map((c, i) => (
              <tr key={i}>
                <td className="py-1 pr-2">
                  <input
                    value={c.name}
                    onChange={(e) => setItem(i, { name: e.target.value })}
                    className={input}
                  />
                </td>
                <td className="py-1 pr-2">
                  <input
                    type="number"
                    min={0}
                    step="any"
                    value={c.price}
                    onChange={(e) =>
                      setItem(i, {
                        price: Math.max(0, Number(e.target.value) || 0),
                      })
                    }
                    className={input}
                  />
                </td>
                <td className="py-1 text-center">
                  <input
                    type="checkbox"
                    checked={c.part}
                    onChange={(e) => setItem(i, { part: e.target.checked })}
                    className="size-4 accent-[var(--color-neon)]"
                  />
                </td>
                <td className="py-1 pr-2">
                  <input
                    type="number"
                    min={0}
                    disabled={!c.part}
                    value={c.part ? c.warrantyDays : ""}
                    onChange={(e) =>
                      setItem(i, {
                        warrantyDays: Math.max(
                          0,
                          Math.round(Number(e.target.value) || 0),
                        ),
                      })
                    }
                    className={`${input} disabled:opacity-40`}
                  />
                </td>
                <td className="py-1 text-right">
                  <button
                    onClick={() =>
                      set({ catalog: s.catalog.filter((_, j) => j !== i) })
                    }
                    className="text-mute hover:text-bad"
                    title="Quitar"
                  >
                    <Trash2 size={13} />
                  </button>
                </td>
              </tr>
            ))}
          </tbody>
        </table>
        <button
          onClick={() =>
            set({
              catalog: [
                ...s.catalog,
                { name: "", price: 0, part: false, warrantyDays: 0 },
              ],
            })
          }
          className="mt-2 flex items-center gap-1 rounded-md border border-line-2 px-2.5 py-1.5 text-xs text-dim hover:text-ink"
        >
          <Plus size={12} /> Añadir al catálogo
        </button>
        <p className="mt-2 text-[11px] text-mute">
          En la sesión o el informe se añaden con un clic. Las piezas pueden
          llevar su propia garantía, que aparece en el recibo y en la ficha del
          cliente.
        </p>
      </Card>
    </div>
  );
}

function About({ appInfo }: { appInfo: AppInfo | null }) {
  const toast = useToast();
  return (
    <Card title="Acerca de" icon={<BadgeCheck size={14} />}>
      <div className="flex items-center gap-3">
        <img src={logo} alt="" className="size-12" />
        <div>
          <div className="font-semibold">AdminOps v{appInfo?.version}</div>
          <div className="text-sm text-dim">
            by <span className="font-semibold text-neon">David Bonilla</span>
          </div>
        </div>
      </div>
      <div className="mt-4 flex flex-wrap gap-2 text-xs">
        {(
          [
            ["data", "Carpeta de datos"],
            ["reports", "Carpeta de informes"],
            ["logs", "Registros"],
          ] as const
        ).map(([kind, label]) => (
          <button
            key={kind}
            onClick={() =>
              appApi.openFolder(kind).catch((e) => toast("error", String(e)))
            }
            className="rounded-md border border-line-2 px-2.5 py-1 text-dim hover:border-neon/40 hover:text-neon"
          >
            {label} →
          </button>
        ))}
      </div>
      <div className="mt-4 border-t border-line/60 pt-3">
        <Button
          kind="ghost"
          onClick={() =>
            appApi
              .supportPackage()
              .then(() =>
                toast("ok", "Paquete de soporte creado: se abrió su carpeta."),
              )
              .catch((e) => toast("error", String(e)))
          }
        >
          Crear paquete de soporte
        </Button>
        <p className="mt-1.5 text-[11px] text-mute">
          Un .zip con el registro de actividad, el último diagnóstico y la
          versión, para enviarlo si algo falla. Puede contener el nombre del
          equipo y del usuario: revísalo antes de compartirlo.
        </p>
      </div>
      <div className="mt-4 border-t border-line/60 pt-3 text-xs text-dim">
        <div className="mb-1 font-medium text-ink">Atajos de AdminOps</div>
        <div className="grid grid-cols-2 gap-1">
          <span>Ctrl + K · buscar o ejecutar</span>
          <span>Ctrl + , · Ajustes</span>
          <span>Ctrl + L · bloquear (si hay PIN)</span>
          <span>Alt + ← / → · página anterior / siguiente</span>
        </div>
      </div>
    </Card>
  );
}
