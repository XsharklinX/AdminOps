import { CheckCircle2, Loader2, Search, Undo2, X } from "lucide-react";
import { useCallback, useEffect, useRef, useState } from "react";
import { type PageId } from "../components/Sidebar";
import { Card, ErrorState, Loading, Toggle } from "../components/ui";
import { PerfPanel } from "../components/PerfPanel";
import { type AppInfo, type Settings, workApi } from "../lib/api";
import { PortalSettings } from "./settings/Portals";
import { findSettings, LEGACY_SECTIONS, SECTIONS, type SettingsSection } from "./settings/catalog";
import { LockSettings } from "./settings/LockSettings";
import { NavEditor } from "./settings/NavEditor";
import { ShortcutEditor } from "./settings/ShortcutEditor";
import { About } from "./settings/About";
import { Appearance } from "./settings/Appearance";
import { AppPreview } from "./settings/AppPreview";
import { Summary } from "./settings/Summary";
import { setPrefs, usePrefs, type Prefs, type PrefsChange } from "../lib/prefs";
import { AlertSettings, DataSettings, DomainCard, StartWindow, SystemChanges } from "./settings/General";
import { DataCare } from "./settings/DataCare";
import { Reports } from "./settings/Reports";
import { HighlightCtx, Row } from "./settings/shared";

type Tab = SettingsSection;
const TAB_KEY = "adminops.settingsTab";

function readTab(): Tab {
  try {
    const saved = localStorage.getItem(TAB_KEY);
    const t = (saved && LEGACY_SECTIONS[saved]) ?? saved;
    if (SECTIONS.some((x) => x.id === t)) return t as Tab;
  } catch {
    /* sin almacenamiento */
  }
  return "summary";
}

/** Pestañas de pantallas abiertas (Ajustes → Navegación). */
function NavTabsCard() {
  const prefs = usePrefs();
  return (
    <Card title="Pantallas abiertas">
      <Row
        title="Pestañas de las pantallas abiertas"
        sub="Debajo de la barra de arriba, una pestaña por cada pantalla que tienes abierta. Ctrl+Tab pasa a la siguiente, Ctrl+Mayús+Tab a la anterior y Ctrl+W cierra la actual."
      >
        <Toggle checked={prefs.pageTabs} onChange={(v) => setPrefs({ pageTabs: v })} />
      </Row>
    </Card>
  );
}

/** Cuánto se ofrece «Deshacer» tras un cambio. */
const UNDO_MS = 8000;
/** Cambios seguidos del mismo ajuste (escribir un texto) cuentan como uno. */
const SAME_CHANGE_MS = 3000;

export function SettingsPage({
  appInfo,
  onNavigate,
  focus,
}: {
  appInfo: AppInfo | null;
  onNavigate: (p: PageId) => void;
  /** Sección a la que llevar (Primeros pasos, enlaces): portals, security, general… */
  focus?: string | null;
}) {
  const [s, setS] = useState<Settings | null>(null);
  // Guardado automático: lo cambiado espera un momento (por si se sigue escribiendo) y se guarda.
  const pending = useRef<Settings | null>(null);
  const [saveState, setSaveState] = useState<"idle" | "saving" | "saved" | "error">("idle");
  // El último cambio, para deshacerlo: cómo estaba antes (ajustes guardados o preferencias de este equipo).
  const [undo, setUndo] = useState<{ settings?: Settings; prefs?: Prefs; keys: string; at: number } | null>(null);
  const restoring = useRef(false);
  const [saveError, setSaveError] = useState("");
  const flush = useCallback(async () => {
    const next = pending.current;
    if (!next) return;
    pending.current = null;
    setSaveState("saving");
    try {
      await workApi.saveSettings(next);
      setSaveState("saved");
    } catch (e) {
      setSaveError(String(e));
      setSaveState("error");
    }
  }, []);
  useEffect(() => {
    if (!pending.current) return;
    const t = window.setTimeout(() => void flush(), 600);
    return () => window.clearTimeout(t);
  }, [s, flush]);
  // Al salir de Ajustes no se pierde lo último que se tocó.
  useEffect(() => () => void flush(), [flush]);
  useEffect(() => {
    if (saveState !== "saved") return;
    const t = window.setTimeout(() => setSaveState("idle"), 2000);
    return () => window.clearTimeout(t);
  }, [saveState]);
  // Las preferencias (apariencia, navegación…) se guardan al momento: también se pueden deshacer.
  useEffect(() => {
    const on = (e: Event) => {
      const before = (e as CustomEvent<PrefsChange>).detail?.before;
      if (!before || restoring.current) return;
      setUndo((u) => (u?.prefs && Date.now() - u.at < SAME_CHANGE_MS ? { ...u, at: Date.now() } : { prefs: before, keys: "prefs", at: Date.now() }));
    };
    window.addEventListener("adminops-prefs", on);
    return () => window.removeEventListener("adminops-prefs", on);
  }, []);
  useEffect(() => {
    if (!undo) return;
    const t = window.setTimeout(() => setUndo(null), UNDO_MS - (Date.now() - undo.at));
    return () => window.clearTimeout(t);
  }, [undo]);
  const [query, setQuery] = useState("");
  const [highlight, setHighlight] = useState<string | null>(null);

  const [tab, setTab] = useState<Tab>(readTab);
  // Lleva la vista a la fila buscada en cuanto la sección la pinta.
  useEffect(() => {
    if (!highlight) return;
    let flashed: Element | null = null;
    const t = window.setTimeout(() => {
      // Una fila lleva el título exacto; una tarjeta puede llevar algo detrás («Portales configurados · 3»).
      const name = CSS.escape(highlight);
      const el = document.querySelector(`[data-setting="${name}"]`) ?? document.querySelector(`[data-setting^="${name}"]`);
      if (!el) return;
      const block = el.tagName === "SECTION";
      el.scrollIntoView({ behavior: "smooth", block: block ? "start" : "center" });
      // Las filas se resaltan solas (HighlightCtx); a las tarjetas se les marca el borde.
      if (block) {
        el.classList.add("setting-flash");
        flashed = el;
      }
    }, 60);
    const clear = window.setTimeout(() => setHighlight(null), 3000);
    return () => {
      window.clearTimeout(t);
      window.clearTimeout(clear);
      flashed?.classList.remove("setting-flash");
    };
  }, [highlight, tab]);

  const [failed, setFailed] = useState<string | null>(null);
  const loadSettings = useCallback(() => {
    setFailed(null);
    workApi
      .settings()
      .then(setS)
      .catch((e) => setFailed(String(e)));
  }, []);
  useEffect(loadSettings, [loadSettings]);

  // Desde Primeros pasos o un enlace: abrir esa sección.
  useEffect(() => {
    // Los enlaces de antes («general», «performance») llevan a donde está eso ahora.
    const target = focus ? (LEGACY_SECTIONS[focus] ?? focus) : null;
    if (target && SECTIONS.some((x) => x.id === target)) setTab(target as Tab);
  }, [focus]);

  const pickTab = (t: Tab) => {
    setTab(t);
    try {
      localStorage.setItem(TAB_KEY, t);
    } catch {
      /* sin almacenamiento */
    }
  };

  if (!s) return failed ? <ErrorState page message={failed} onRetry={loadSettings} /> : <Loading page />;

  const set = (patch: Partial<Settings>) => {
    const next = { ...s, ...patch };
    pending.current = next;
    setS(next);
    const keys = Object.keys(patch).sort().join(",");
    // Reintentar (sin cambios) no es un cambio; escribir letra a letra en el mismo campo, uno solo.
    if (keys && !restoring.current) {
      setUndo((u) => (u?.settings && u.keys === keys && Date.now() - u.at < SAME_CHANGE_MS ? { ...u, at: Date.now() } : { settings: s, keys, at: Date.now() }));
    }
  };
  const undoLast = () => {
    if (!undo) return;
    restoring.current = true;
    if (undo.settings) {
      pending.current = undo.settings;
      setS(undo.settings);
    }
    if (undo.prefs) setPrefs(undo.prefs);
    restoring.current = false;
    setUndo(null);
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
    <div className="mx-auto max-w-(--page-max) p-6">
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
          <ul className="no-scrollbar flex gap-1 overflow-x-auto lg:flex-col lg:overflow-visible">
            {SECTIONS.map((x, i) => {
              const Icon = x.icon;
              const on = tab === x.id;
              const firstOfGroup = i === 0 || SECTIONS[i - 1].group !== x.group;
              return (
                <li key={x.id} className="shrink-0 lg:shrink">
                  {firstOfGroup && (
                    <div className={`hidden px-3 pb-1 text-[10.5px] font-medium tracking-wider text-mute uppercase lg:block ${i ? "pt-4" : ""}`}>{x.group}</div>
                  )}
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

            {tab === "start" && <StartWindow s={s} set={set} />}
            {tab === "alerts" && <AlertSettings s={s} set={set} />}
            {tab === "data" && <DataSettings s={s} set={set} portable={!!appInfo?.portable} onImported={loadSettings} />}
            {tab === "summary" && <Summary s={s} appInfo={appInfo} onGo={(section, title) => goTo({ section, title: title ?? "" })} />}
            {tab === "appearance" && (
              <div className="grid items-start gap-4 xl:grid-cols-[minmax(0,1fr)_300px]">
                <Appearance />
                <div className="xl:sticky xl:top-4">
                  <AppPreview />
                </div>
              </div>
            )}
            {tab === "navigation" && (
              <div className="space-y-4">
                <NavTabsCard />
                <ShortcutEditor />
                <NavEditor />
              </div>
            )}
            {tab === "security" && (
              <div className="space-y-4">
                <LockSettings />
                <SystemChanges s={s} set={set} />
              </div>
            )}
            {tab === "portals" && (
              <div className="space-y-4">
                <PortalSettings s={s} set={set} onNavigate={onNavigate} Row={Row} />
                <DomainCard s={s} set={set} />
              </div>
            )}
            {tab === "reports" && <Reports s={s} set={set} />}
            {tab === "about" && (
              <div className="space-y-4">
                <About appInfo={appInfo} />
                <DataCare s={s} set={set} part="updates" />
                <PerfPanel />
              </div>
            )}
          </div>
        </HighlightCtx.Provider>
      </div>

      {(saveState !== "idle" || undo) && (
        <div className="pointer-events-none sticky bottom-3 z-30 mt-4 flex justify-end" role="status">
          <span
            className={`pointer-events-auto flex items-center gap-1.5 rounded-full border bg-panel px-3 py-1.5 text-xs shadow-lg ${
              saveState === "error" ? "border-bad/50 text-bad" : "border-line-2 text-dim"
            }`}
          >
            {saveState === "saving" && (
              <>
                <Loader2 size={12} className="animate-spin" /> Guardando…
              </>
            )}
            {(saveState === "saved" || (saveState === "idle" && undo)) && (
              <>
                <CheckCircle2 size={12} className="text-ok" /> Guardado
              </>
            )}
            {undo && saveState !== "error" && saveState !== "saving" && (
              <button onClick={undoLast} className="ml-1 flex items-center gap-1 rounded-full border border-line-2 px-2 py-0.5 text-dim transition-colors hover:border-neon/40 hover:text-neon">
                <Undo2 size={11} /> Deshacer
              </button>
            )}
            {saveState === "error" && (
              <>
                No se pudo guardar: {saveError}
                <button onClick={() => set({})} className="ml-1 underline" title="Volver a intentarlo">
                  Reintentar
                </button>
              </>
            )}
          </span>
        </div>
      )}
    </div>
  );
}
