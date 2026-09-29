import { RefreshCw } from "lucide-react";
import { useCallback, useEffect, useState } from "react";
import { useConfirm, useToast } from "../components/feedback";
import { TweakCard } from "../components/TweakCard";
import { RP_FAILED, tweaksApi, type OpResult, type TweakView } from "../lib/api";
import { ScheduleCard } from "../components/Maintenance";
import { useOnJournalChange } from "../lib/journalEvents";

const CANCELLED = "Cancelado por el usuario.";

/** Nombre de cada categoría (encabezados al buscar en varias a la vez). */
export const TWEAK_CATEGORY_LABEL: Record<string, string> = {
  cleanup: "Limpieza",
  performance: "Rendimiento",
  privacy: "Privacidad",
  services: "Servicios",
  repair: "Reparaciones",
};

/** Sin tildes ni mayúsculas, para buscar «telemetria» y encontrar «Telemetría». */
const fold = (s: string) => s.normalize("NFD").replace(/[\u0300-\u036f]/g, "").toLowerCase();

export function matchesTweak(t: TweakView, query: string): boolean {
  const words = fold(query).split(/\s+/).filter(Boolean);
  const hay = fold([t.name, t.description, t.note ?? "", ...t.changes].join(" "));
  return words.every((w) => hay.includes(w));
}

export function TweaksPage({
  category,
  isAdmin,
  focus,
  query = "",
}: {
  /** Una categoría, o varias (se muestran agrupadas). */
  category: string | string[];
  isAdmin: boolean;
  focus?: string | null;
  /** Filtra por nombre, descripción o lo que cambia. */
  query?: string;
}) {
  const [tweaks, setTweaks] = useState<TweakView[] | null>(null);
  const [busy, setBusy] = useState<Record<string, string>>({});
  const [messages, setMessages] = useState<Record<string, string>>({});
  const [loading, setLoading] = useState(false);
  const [highlight, setHighlight] = useState<string | null>(null);
  const toast = useToast();
  const { confirm, dialog } = useConfirm();

  const fail = (t: TweakView, e: unknown) => {
    const err = String(e);
    if (err === CANCELLED) toast("info", `${t.name}: cancelado. Lo que ya se hubiera cambiado se deshizo.`);
    else toast("error", `${t.name}: ${err}`);
  };

  // Llegada desde un hallazgo del diagnóstico: ir al ajuste y resaltarlo.
  const loaded = tweaks !== null;
  useEffect(() => {
    if (!focus || !loaded) return;
    const el = document.getElementById(`focus-${focus}`);
    if (!el) return;
    el.scrollIntoView({ behavior: "smooth", block: "center" });
    setHighlight(focus);
    const t = window.setTimeout(() => setHighlight(null), 2500);
    return () => window.clearTimeout(t);
  }, [focus, loaded]);

  const catKey = Array.isArray(category) ? category.join(",") : category;
  const load = useCallback(async () => {
    setLoading(true);
    try {
      const cats = catKey.split(",");
      setTweaks(cats.length > 1 ? (await tweaksApi.list()).filter((t) => cats.includes(t.category)) : await tweaksApi.list(catKey));
    } catch (e) {
      toast("error", String(e));
    } finally {
      setLoading(false);
    }
  }, [catKey, toast]);

  useOnJournalChange(load);
  useEffect(() => {
    setTweaks(null);
    setMessages({});
    load();
  }, [load]);

  const setBusyFor = (id: string, label?: string) =>
    setBusy((b) => {
      const n = { ...b };
      if (label) n[id] = label;
      else delete n[id];
      return n;
    });

  const done = (t: TweakView, r: OpResult) => {
    setMessages((m) => ({ ...m, [t.id]: r.message }));
    toast("ok", `${t.name}: ${r.message}${r.restorePointCreated ? " Se creó un punto de restauración." : ""}`);
  };

  const apply = async (t: TweakView, skipRestorePoint = false) => {
    setBusyFor(t.id, t.risk !== "low" && !skipRestorePoint ? "Creando punto de restauración y aplicando…" : "Aplicando…");
    try {
      done(t, await tweaksApi.apply(t.id, skipRestorePoint));
    } catch (e) {
      const err = String(e);
      if (err.startsWith(RP_FAILED)) {
        setBusyFor(t.id);
        const go = await confirm({
          title: "No se pudo crear el punto de restauración",
          body: (
            <>
              <p className="mb-2 font-mono text-xs break-words text-bad">{err.slice(RP_FAILED.length)}</p>
              <p>
                Puedes aplicar el ajuste igualmente: AdminOps guarda el estado anterior y podrás deshacerlo desde aquí o
                desde el Historial.
              </p>
            </>
          ),
          confirmLabel: "Aplicar sin punto",
        });
        if (go) return apply(t, true);
      } else {
        fail(t, err);
      }
    } finally {
      setBusyFor(t.id);
      load();
    }
  };

  const toggle = async (t: TweakView) => {
    if (t.status === "applied") {
      setBusyFor(t.id, "Deshaciendo…");
      try {
        done(t, await tweaksApi.revert(t.id));
      } catch (e) {
        fail(t, e);
      } finally {
        setBusyFor(t.id);
        load();
      }
      return;
    }
    if (t.risk === "high") {
      const go = await confirm({
        title: `¿Aplicar "${t.name}"?`,
        body: (
          <>
            <p className="mb-2">{t.note ?? t.description}</p>
            <p>Se creará un punto de restauración antes de aplicarlo.</p>
          </>
        ),
        confirmLabel: "Aplicar",
        danger: true,
      });
      if (!go) return;
    }
    apply(t);
  };

  const run = async (t: TweakView) => {
    if (t.risk !== "low") {
      const go = await confirm({
        title: `¿Ejecutar "${t.name}"?`,
        body: <p>{t.note ?? t.description}</p>,
        confirmLabel: "Ejecutar",
        danger: t.risk === "high",
      });
      if (!go) return;
    }
    setBusyFor(t.id, "Ejecutando…");
    try {
      done(t, await tweaksApi.run(t.id));
    } catch (e) {
      fail(t, e);
    } finally {
      setBusyFor(t.id);
    }
  };

  if (!tweaks) return <p className="p-8 font-mono text-sm text-mute">Detectando estado actual…</p>;

  const shown = query.trim() ? tweaks.filter((t) => matchesTweak(t, query)) : tweaks;
  const toggles = shown.filter((t) => t.kind === "toggle");
  const applied = toggles.filter((t) => t.status === "applied").length;
  const grouped = catKey.includes(",");
  const groups = grouped
    ? catKey
        .split(",")
        .map((c) => ({ c, items: shown.filter((t) => t.category === c) }))
        .filter((g) => g.items.length)
    : [{ c: catKey, items: shown }];

  const card = (t: TweakView) => (
    <TweakCard
      key={t.id}
      tweak={t}
      isAdmin={isAdmin}
      busy={!!busy[t.id]}
      busyLabel={busy[t.id]}
      lastMessage={messages[t.id]}
      highlighted={highlight === t.id}
      onToggle={() => toggle(t)}
      onRun={() => run(t)}
    />
  );

  return (
    <div className="mx-auto max-w-4xl p-6">
      {catKey === "cleanup" && !query && <ScheduleCard isAdmin={isAdmin} />}
      <div className="mb-4 flex items-center justify-between">
        <p className="text-sm text-dim">
          {toggles.length > 0 ? (
            <>
              <span className="font-mono text-neon">{applied}</span> de {toggles.length} ajustes aplicados
            </>
          ) : (
            `${shown.length} tareas disponibles`
          )}
        </p>
        <button
          onClick={load}
          disabled={loading}
          className="flex items-center gap-1.5 rounded-md px-2.5 py-1 text-xs text-dim transition-colors hover:bg-panel-2 hover:text-ink"
        >
          <RefreshCw size={12} className={loading ? "animate-spin" : ""} /> Volver a detectar
        </button>
      </div>
      {shown.length === 0 && <p className="py-10 text-center text-sm text-mute">Ningún ajuste coincide con «{query}».</p>}
      {groups.map((g) => (
        <section key={g.c} className="mb-6">
          {grouped && <h3 className="mb-2 text-xs font-medium tracking-wide text-mute uppercase">{TWEAK_CATEGORY_LABEL[g.c] ?? g.c}</h3>}
          <div className="space-y-3">{g.items.map(card)}</div>
        </section>
      ))}
      {dialog}
    </div>
  );
}
