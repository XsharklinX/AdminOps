import {
  Briefcase,
  CheckCircle2,
  CircleDashed,
  EyeOff,
  Gamepad2,
  Layers,
  Loader2,
  MinusCircle,
  Play,
  RotateCcw,
  Turtle,
  XCircle,
  type LucideIcon,
} from "lucide-react";
import { useCallback, useEffect, useState } from "react";
import { useConfirm, useToast } from "../components/feedback";
import { RP_FAILED, profilesApi, type ProfileResult, type ProfileView } from "../lib/api";

const ICONS: Record<string, LucideIcon> = {
  briefcase: Briefcase,
  gamepad: Gamepad2,
  turtle: Turtle,
  "eye-off": EyeOff,
};

const OUTCOME = {
  applied: { label: "Aplicado", cls: "text-neon", icon: CheckCircle2 },
  reverted: { label: "Deshecho", cls: "text-neon", icon: RotateCcw },
  ran: { label: "Ejecutado", cls: "text-neon", icon: CheckCircle2 },
  skipped: { label: "Omitido", cls: "text-mute", icon: MinusCircle },
  failed: { label: "Error", cls: "text-bad", icon: XCircle },
};

export function Profiles({ isAdmin }: { isAdmin: boolean }) {
  const [profiles, setProfiles] = useState<ProfileView[] | null>(null);
  const [busy, setBusy] = useState<string | null>(null);
  const [result, setResult] = useState<{ profile: string; title: string; r: ProfileResult } | null>(null);
  const toast = useToast();
  const { confirm, dialog } = useConfirm();

  const load = useCallback(async () => {
    try {
      setProfiles(await profilesApi.list());
    } catch (e) {
      toast("error", String(e));
    }
  }, [toast]);

  useEffect(() => {
    load();
  }, [load]);

  const apply = async (p: ProfileView, skipRestorePoint = false): Promise<void> => {
    const pending = p.items.filter((i) => i.supported && (i.kind === "action" || i.status !== "applied"));
    if (!skipRestorePoint) {
      const ok = await confirm({
        title: `¿Aplicar el perfil "${p.name}"?`,
        confirmLabel: "Aplicar perfil",
        body: (
          <>
            <p className="mb-2">Se aplicarán {pending.length} cambios. Antes se creará un punto de restauración.</p>
            <ul className="max-h-48 overflow-y-auto rounded-md border border-line bg-void/50 px-3 py-2 text-xs">
              {pending.map((i) => (
                <li key={i.id} className="flex justify-between gap-2 py-0.5">
                  <span className="text-ink">{i.name}</span>
                  {i.risk !== "low" && <span className={i.risk === "high" ? "text-bad" : "text-warn"}>riesgo {i.risk === "high" ? "alto" : "medio"}</span>}
                </li>
              ))}
            </ul>
            <p className="mt-2">Podrás deshacerlo entero con "Deshacer perfil" o ajuste por ajuste desde el Historial.</p>
          </>
        ),
      });
      if (!ok) return;
    }
    setBusy(p.id);
    try {
      const r = await profilesApi.apply(p.id, skipRestorePoint);
      setResult({ profile: p.id, title: `Perfil "${p.name}" aplicado`, r });
    } catch (e) {
      const err = String(e);
      if (err.startsWith(RP_FAILED)) {
        setBusy(null);
        const go = await confirm({
          title: "No se pudo crear el punto de restauración",
          body: <p className="font-mono text-xs break-words text-bad">{err.slice(RP_FAILED.length)}</p>,
          confirmLabel: "Aplicar sin punto",
        });
        if (go) return apply(p, true);
      } else {
        toast("error", err);
      }
    } finally {
      setBusy(null);
      load();
    }
  };

  const revert = async (p: ProfileView) => {
    const ok = await confirm({
      title: `¿Deshacer el perfil "${p.name}"?`,
      confirmLabel: "Deshacer",
      body: <p>Se restaurará el estado anterior de los ajustes que AdminOps aplicó con este perfil. Los que ya venían aplicados no se tocan.</p>,
    });
    if (!ok) return;
    setBusy(p.id);
    try {
      const r = await profilesApi.revert(p.id);
      setResult({ profile: p.id, title: `Perfil "${p.name}" deshecho`, r });
    } catch (e) {
      toast("error", String(e));
    } finally {
      setBusy(null);
      load();
    }
  };

  if (!profiles) return <p className="p-8 font-mono text-sm text-mute">Detectando estado de los perfiles…</p>;

  return (
    <div className="mx-auto max-w-6xl p-6">
      {!isAdmin && (
        <p className="mb-4 rounded-lg border border-warn/30 bg-warn/5 px-3.5 py-2.5 text-sm text-warn">
          Los perfiles modifican el registro y los servicios: requieren ejecutar AdminOps como administrador.
        </p>
      )}

      {result && (
        <div className="mb-4 rounded-xl border border-neon/30 bg-neon/5 p-4">
          <div className="mb-2 flex items-center justify-between">
            <h3 className="font-medium text-ink">{result.title}</h3>
            <button onClick={() => setResult(null)} className="text-xs text-mute hover:text-ink">
              Cerrar
            </button>
          </div>
          {(result.r.restorePointCreated || result.r.reboot) && (
            <p className="mb-2 text-xs text-dim">
              {result.r.restorePointCreated && "Se creó un punto de restauración. "}
              {result.r.reboot && <span className="text-warn">Reinicia el equipo para completar algunos cambios.</span>}
            </p>
          )}
          <ul className="grid grid-cols-2 gap-x-6 gap-y-1">
            {result.r.results.map((x) => {
              const O = OUTCOME[x.outcome];
              return (
                <li key={x.id} className="flex min-w-0 items-start gap-2 text-xs" title={x.message ?? undefined}>
                  <O.icon size={13} className={`mt-px shrink-0 ${O.cls}`} />
                  <span className="truncate text-ink">{x.name}</span>
                  <span className={`ml-auto shrink-0 ${O.cls}`}>{O.label}</span>
                </li>
              );
            })}
          </ul>
        </div>
      )}

      <div className="grid grid-cols-1 gap-4 lg:grid-cols-2">
        {profiles.map((p) => {
          const Icon = ICONS[p.icon] ?? Layers;
          const toggles = p.items.filter((i) => i.kind === "toggle");
          const applied = toggles.filter((i) => i.status === "applied").length;
          const canRevert = p.items.some((i) => i.hasBackup);
          const isBusy = busy === p.id;
          return (
            <article key={p.id} className="flex flex-col rounded-xl border border-line bg-panel p-5">
              <div className="mb-3 flex items-start gap-3">
                <div className="grid size-10 shrink-0 place-items-center rounded-lg bg-neon/10 text-neon glow-neon">
                  <Icon size={20} />
                </div>
                <div className="min-w-0 flex-1">
                  <h2 className="text-lg font-semibold">{p.name}</h2>
                  <p className="text-sm text-dim">{p.description}</p>
                </div>
              </div>

              <div className="mb-3">
                <div className="mb-1 flex justify-between text-xs text-dim">
                  <span>Ajustes aplicados</span>
                  <span className="font-mono">
                    {applied}/{toggles.length}
                  </span>
                </div>
                <div className="h-1.5 overflow-hidden rounded-full bg-line">
                  <div
                    className="h-full rounded-full bg-neon shadow-[0_0_8px_var(--color-neon)] transition-all"
                    style={{ width: `${toggles.length ? (applied / toggles.length) * 100 : 0}%` }}
                  />
                </div>
              </div>

              <ul className="mb-4 flex-1 space-y-1">
                {p.items.map((i) => (
                  <li key={i.id} className="flex items-center gap-2 text-xs">
                    {i.kind === "action" ? (
                      <Play size={11} className="shrink-0 text-mute" />
                    ) : i.status === "applied" ? (
                      <CheckCircle2 size={12} className="shrink-0 text-neon" />
                    ) : (
                      <CircleDashed size={12} className="shrink-0 text-mute" />
                    )}
                    <span className={i.supported ? "text-dim" : "text-mute line-through"}>{i.name}</span>
                    {i.kind === "action" && <span className="text-mute">(tarea)</span>}
                  </li>
                ))}
              </ul>

              <div className="flex gap-2">
                <button
                  onClick={() => apply(p)}
                  disabled={!isAdmin || busy !== null}
                  className="flex flex-1 items-center justify-center gap-1.5 rounded-md border border-neon/50 bg-neon/10 py-2 text-sm font-medium text-neon transition-colors hover:bg-neon/20 disabled:cursor-not-allowed disabled:opacity-40"
                >
                  {isBusy ? <Loader2 size={14} className="animate-spin" /> : <Play size={14} />}
                  {isBusy ? "Aplicando…" : "Aplicar perfil"}
                </button>
                {canRevert && (
                  <button
                    onClick={() => revert(p)}
                    disabled={!isAdmin || busy !== null}
                    className="flex items-center gap-1.5 rounded-md border border-line-2 px-3 py-2 text-sm text-dim transition-colors hover:border-neon/40 hover:text-neon disabled:opacity-40"
                  >
                    <RotateCcw size={14} /> Deshacer perfil
                  </button>
                )}
              </div>
            </article>
          );
        })}
      </div>
      {dialog}
    </div>
  );
}
