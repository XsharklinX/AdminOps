import { useRef, type ReactNode } from "react";
import { createPortal } from "react-dom";
import { Pin } from "lucide-react";
import { SlideMark } from "./SlideMark";
import { PageActiveContext, usePageActive } from "../lib/pageActive";
import { setSidebar, usePrefs } from "../lib/prefs";
import { tabLetters } from "../lib/keytips";
import { usePageId } from "../lib/sectionState";
import { useTabsSlot } from "../lib/tabsSlot";
import { navKey } from "../lib/sections";
import { useNewMarks } from "../lib/whatsNew";

export interface PageTab<T extends string> {
  id: T;
  label: string;
  icon?: ReactNode;
  /** Qué hace esta pestaña (al pasar el ratón). */
  help?: string;
  /** Letra fija para Alt (si no, la inicial del nombre). */
  kt?: string;
  /** Nombre corto, para las píldoras dentro de un grupo (Estado del equipo). */
  short?: string;
}

/** Fijar o quitar de la barra lateral una sección («page:sección»). */
export function useSectionPin() {
  const favorites = usePrefs().sidebar.favorites;
  return {
    pinned: (k: string) => favorites.includes(k),
    toggle: (k: string) => setSidebar({ favorites: favorites.includes(k) ? favorites.filter((x) => x !== k) : [...favorites, k] }),
  };
}

/** Chincheta que sale al pasar el ratón por una pestaña (o se queda si ya está fijada). */
export function TabPin({ k, label, at = "-top-1 -right-1.5" }: { k: string; label: string; /** Dónde va, dentro de la pestaña (que lleva `relative`). */ at?: string }) {
  const pin = useSectionPin();
  const on = pin.pinned(k);
  return (
    <span
      role="button"
      tabIndex={-1}
      onClick={(e) => {
        e.stopPropagation();
        pin.toggle(k);
      }}
      title={on ? "Quitar de la barra lateral" : "Fijar en la barra lateral"}
      aria-label={on ? `Quitar ${label} de fijados` : `Fijar ${label} en la barra lateral`}
      // En la esquina y por encima: no ocupa sitio en la pestaña ni la ensancha.
      className={`absolute ${at} z-10 grid size-[18px] place-items-center rounded-full border border-line-2 bg-panel opacity-0 shadow transition-opacity group-hover/tab:opacity-100 focus-visible:opacity-100 ${on ? "text-neon" : "text-mute hover:text-ink"}`}
    >
      <Pin size={10} fill={on ? "currentColor" : "none"} />
    </span>
  );
}

/**
 * Pestañas de una página que agrupa varias vistas. En la pantalla a la vista van
 * en la cabecera, en la misma línea que el título (tabsSlot); si no hay hueco
 * (la mitad derecha de la vista dividida), van encima del contenido.
 */
export function PageTabs<T extends string>({
  tabs,
  value,
  onChange,
  pinnable = true,
}: {
  tabs: PageTab<T>[];
  value: T;
  onChange: (t: T) => void;
  /** Cada pestaña se puede fijar en la barra lateral (no en los grupos de Estado del equipo). */
  pinnable?: boolean;
}) {
  const row = useRef<HTMLDivElement>(null);
  const page = usePageId();
  const slot = useTabsSlot(page);
  const fresh = useNewMarks();
  const letters = tabLetters(tabs.map((t) => t.label));
  const inHead = !!slot;

  const strip = (
    <div
      ref={row}
      role="tablist"
      className={
        inHead
          ? "no-scrollbar relative isolate flex min-w-0 flex-1 gap-0.5 self-stretch overflow-x-auto"
          : "no-scrollbar relative isolate mx-auto flex w-full max-w-(--page-max) shrink-0 gap-1 overflow-x-auto border-b border-line px-6 pt-4"
      }
    >
      <SlideMark within={row} kind="underline" />
      {tabs.map((t, i) => {
        const k = page ? navKey(page, t.id) : "";
        return (
          <button
            key={t.id}
            role="tab"
            data-kt={t.kt ?? letters[i]}
            aria-selected={value === t.id}
            onClick={() => onChange(t.id)}
            title={t.help}
            aria-current={value === t.id ? "page" : undefined}
            className={`group/tab relative -mb-px flex shrink-0 items-center gap-1.5 border-b-2 border-transparent whitespace-nowrap transition-colors ${
              inHead ? "px-3 text-[13px]" : "px-3.5 py-2 text-sm"
            } ${value === t.id ? "font-medium text-ink" : "text-dim hover:text-ink"}`}
          >
            {!inHead && t.icon}
            {t.label}
            {k && fresh.includes(k) && <span className="size-1.5 rounded-full bg-ok" title="Ha cambiado en esta versión" />}
            {pinnable && k && <TabPin k={k} label={t.label} at={inHead ? "top-1 right-0" : "top-0 right-0"} />}
          </button>
        );
      })}
    </div>
  );

  return inHead ? createPortal(strip, slot) : strip;
}

/**
 * Mantiene montadas las pestañas ya visitadas (conservan sus datos al cambiar),
 * igual que las páginas de la app.
 */
export function TabPanels<T extends string>({ value, visited, render }: { value: T; visited: T[]; render: (t: T) => ReactNode }) {
  // La página puede estar visible y la pestaña no: lo que la pestaña tenga en
  // marcha (temperaturas, métricas, vistas web) debe pararse igual que al
  // cambiar de página. Por eso cada pestaña recibe su propio «estoy a la vista».
  const pageActive = usePageActive();
  return (
    <>
      {visited.map((t) => (
        // Ocupa el alto que queda: una web incrustada (portal) necesita su área completa.
        <div key={t} hidden={t !== value} className="min-h-0 flex-1 overflow-y-auto">
          <PageActiveContext.Provider value={pageActive && t === value}>{render(t)}</PageActiveContext.Provider>
        </div>
      ))}
    </>
  );
}
