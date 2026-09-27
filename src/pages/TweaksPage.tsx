import { RefreshCw } from "lucide-react";
import { useCallback, useEffect, useState } from "react";
import { useConfirm, useToast } from "../components/feedback";
import { TweakCard } from "../components/TweakCard";
import { RP_FAILED, tweaksApi, type OpResult, type TweakView } from "../lib/api";
import { ScheduleCard } from "../components/Maintenance";
import { useOnJournalChange } from "../lib/journalEvents";

const CANCELLED = "Cancelado por el usuario.";

export function TweaksPage({ category, isAdmin, focus }: { category: string; isAdmin: boolean; focus?: string | null }) {
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

  const load = useCallback(async () => {
    setLoading(true);
    try {
      setTweaks(await tweaksApi.list(category));
    } catch (e) {
      toast("error", String(e));
    } finally {
      setLoading(false);
    }
  }, [category, toast]);

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

  const toggles = tweaks.filter((t) => t.kind === "toggle");
  const applied = toggles.filter((t) => t.status === "applied").length;

  return (
    <div className="mx-auto max-w-4xl p-6">
      {category === "cleanup" && <ScheduleCard isAdmin={isAdmin} />}
      <div className="mb-4 flex items-center justify-between">
        <p className="text-sm text-dim">
          {toggles.length > 0 ? (
            <>
              <span className="font-mono text-neon">{applied}</span> de {toggles.length} ajustes aplicados
            </>
          ) : (
            `${tweaks.length} tareas disponibles`
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
      <div className="space-y-3">
        {tweaks.map((t) => (
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
        ))}
      </div>
      {dialog}
    </div>
  );
}
