import { CheckCircle2, Loader2, Search, X } from "lucide-react";
import { useCallback, useEffect, useRef, useState } from "react";
import { type PageId } from "../components/Sidebar";
import { ErrorState, Loading } from "../components/ui";
import { PerfPanel } from "../components/PerfPanel";
import { type AppInfo, type Settings, workApi } from "../lib/api";
import { PortalSettings } from "./settings/Portals";
import { findSettings, SECTIONS, type SettingsSection } from "./settings/catalog";
import { LockSettings } from "./settings/LockSettings";
import { NavEditor } from "./settings/NavEditor";
import { ShortcutEditor } from "./settings/ShortcutEditor";
import { About } from "./settings/About";
import { Appearance } from "./settings/Appearance";
import { General } from "./settings/General";
import { Reports } from "./settings/Reports";
import { HighlightCtx, Row } from "./settings/shared";

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
    if (focus && SECTIONS.some((x) => x.id === focus)) setTab(focus as Tab);
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
          <ul className="no-scrollbar flex gap-1 overflow-x-auto lg:flex-col lg:overflow-visible">
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
                onImported={loadSettings}
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

      {saveState !== "idle" && (
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
            {saveState === "saved" && (
              <>
                <CheckCircle2 size={12} className="text-ok" /> Guardado
              </>
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
