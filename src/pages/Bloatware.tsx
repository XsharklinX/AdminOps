import { Download, Loader2, PackageX, RefreshCw, Search, TriangleAlert } from "lucide-react";
import { useCallback, useEffect, useMemo, useState } from "react";
import { useConfirm, useToast } from "../components/feedback";
import { TaskStatus } from "../components/TaskStatus";
import { RP_FAILED, systemApi, type Advice, type AppView } from "../lib/api";

const ADVICE: Record<Advice, { title: string; hint: string; cls: string }> = {
  remove: { title: "Recomendado quitar", hint: "Promocionales o retiradas por Microsoft.", cls: "text-ok" },
  optional: { title: "Depende del usuario", hint: "Útiles para algunos; pregunta antes de quitar.", cls: "text-warn" },
  keep: { title: "Mejor conservar", hint: "Funciones del sistema que el usuario espera tener.", cls: "text-bad" },
};

export function Bloatware({ isAdmin }: { isAdmin: boolean }) {
  const [apps, setApps] = useState<AppView[] | null>(null);
  const [loading, setLoading] = useState(false);
  const [selected, setSelected] = useState<Set<string>>(new Set());
  const [query, setQuery] = useState("");
  const [removing, setRemoving] = useState(false);
  const [reinstalling, setReinstalling] = useState<string | null>(null);
  const toast = useToast();
  const { confirm, dialog } = useConfirm();

  const load = useCallback(async () => {
    setLoading(true);
    try {
      setApps(await systemApi.listApps());
    } catch (e) {
      toast("error", String(e));
    } finally {
      setLoading(false);
    }
  }, [toast]);

  useEffect(() => {
    load();
  }, [load]);

  const filtered = useMemo(() => {
    const q = query.trim().toLowerCase();
    return (apps ?? []).filter(
      (a) => !q || a.name.toLowerCase().includes(q) || a.package.toLowerCase().includes(q),
    );
  }, [apps, query]);

  const installed = filtered.filter((a) => a.installed);
  const groups = (["remove", "optional", "keep"] as Advice[]).map((adv) => ({
    advice: adv,
    apps: installed.filter((a) => a.advice === adv),
  }));
  const others = installed.filter((a) => a.advice === null);
  const missing = filtered.filter((a) => !a.installed);

  const toggle = (pkg: string) =>
    setSelected((s) => {
      const n = new Set(s);
      if (n.has(pkg)) n.delete(pkg);
      else n.add(pkg);
      return n;
    });

  const selectRecommended = () =>
    setSelected(new Set((apps ?? []).filter((a) => a.installed && a.advice === "remove").map((a) => a.package)));

  const remove = async (skipRestorePoint = false): Promise<void> => {
    const chosen = (apps ?? []).filter((a) => selected.has(a.package));
    if (!skipRestorePoint) {
      const risky = chosen.filter((a) => a.advice !== "remove");
      const noUndo = chosen.filter((a) => !a.reinstallable);
      const ok = await confirm({
        title: `¿Quitar ${chosen.length} ${chosen.length === 1 ? "app" : "apps"}?`,
        danger: risky.length > 0,
        confirmLabel: "Quitar",
        body: (
          <>
            <ul className="mb-3 max-h-40 overflow-y-auto rounded-md border border-line bg-void/50 px-3 py-2 text-xs">
              {chosen.map((a) => (
                <li key={a.package} className="flex justify-between gap-2 py-0.5">
                  <span className="text-ink">{a.name}</span>
                  {!a.reinstallable && <span className="text-warn">sin reinstalación</span>}
                </li>
              ))}
            </ul>
            <p>
              Se quitan para <b className="text-ink">todos los usuarios</b> y para las cuentas nuevas. Antes se creará un
              punto de restauración.
            </p>
            {noUndo.length > 0 && (
              <p className="mt-2 text-warn">
                {noUndo.length} no se {noUndo.length === 1 ? "puede" : "pueden"} reinstalar automáticamente.
              </p>
            )}
          </>
        ),
      });
      if (!ok) return;
    }

    setRemoving(true);
    try {
      const r = await systemApi.removeApps([...selected], skipRestorePoint);
      const failed = r.results.filter((x) => !x.ok);
      const done = r.results.length - failed.length;
      if (done) toast("ok", `${done} ${done === 1 ? "app quitada" : "apps quitadas"}.${r.restorePointCreated ? " Se creó un punto de restauración." : ""}`);
      failed.forEach((f) => toast("error", `${f.package}: ${f.message}`));
      setSelected(new Set(failed.map((f) => f.package)));
    } catch (e) {
      const err = String(e);
      if (err.startsWith(RP_FAILED)) {
        setRemoving(false);
        const go = await confirm({
          title: "No se pudo crear el punto de restauración",
          body: <p className="font-mono text-xs break-words text-bad">{err.slice(RP_FAILED.length)}</p>,
          confirmLabel: "Quitar sin punto",
        });
        if (go) return remove(true);
      } else {
        toast("error", err);
      }
    } finally {
      setRemoving(false);
      load();
    }
  };

  const reinstall = async (a: AppView) => {
    setReinstalling(a.package);
    try {
      await systemApi.reinstallApp(a.package);
      toast("ok", `${a.name} reinstalada.`);
    } catch (e) {
      toast("error", String(e));
    } finally {
      setReinstalling(null);
      load();
    }
  };

  if (!apps) return <p className="p-8 font-mono text-sm text-mute">Leyendo apps instaladas…</p>;

  const row = (a: AppView) => (
    <label
      key={a.package}
      className={`flex cursor-pointer items-start gap-3 rounded-lg border px-3.5 py-2.5 transition-colors ${
        selected.has(a.package) ? "border-neon/40 bg-neon/5" : "border-line bg-panel hover:border-line-2"
      } ${!isAdmin ? "cursor-default" : ""}`}
    >
      <input
        type="checkbox"
        checked={selected.has(a.package)}
        disabled={!isAdmin || removing}
        onChange={() => toggle(a.package)}
        className="mt-1 size-4 accent-[var(--color-neon)]"
      />
      <div className="min-w-0 flex-1">
        <div className="flex flex-wrap items-baseline gap-x-2">
          <span className="text-sm font-medium text-ink">{a.name}</span>
          <span className="truncate font-mono text-[11px] text-mute select-text">{a.package}</span>
        </div>
        {a.description && <p className="text-xs text-dim">{a.description}</p>}
        {a.note && (
          <p className="mt-1 flex gap-1.5 text-xs text-warn/90">
            <TriangleAlert size={12} className="mt-px shrink-0" /> {a.note}
          </p>
        )}
        {!a.description && a.publisher && <p className="text-xs text-dim">{a.publisher}</p>}
      </div>
      <div className="flex shrink-0 flex-col items-end gap-1 text-[11px] text-mute">
        {a.version && <span className="font-mono">{a.version}</span>}
        {!a.reinstallable && a.advice && <span className="text-warn/80">sin reinstalación</span>}
      </div>
    </label>
  );

  return (
    <div className="mx-auto max-w-4xl p-6 pb-24">
      {!isAdmin && (
        <p className="mb-4 rounded-lg border border-warn/30 bg-warn/5 px-3.5 py-2.5 text-sm text-warn">
          Sin administrador solo se ven las apps de tu usuario y no se pueden quitar.
        </p>
      )}

      <div className="mb-5 flex items-center gap-2">
        <div className="relative flex-1">
          <Search size={14} className="absolute top-1/2 left-3 -translate-y-1/2 text-mute" />
          <input
            value={query}
            onChange={(e) => setQuery(e.target.value)}
            placeholder="Buscar app o paquete…"
            className="w-full rounded-md border border-line bg-panel py-1.5 pr-3 pl-8 text-sm text-ink outline-none placeholder:text-mute focus:border-neon/50"
          />
        </div>
        <button
          onClick={selectRecommended}
          disabled={!isAdmin}
          className="rounded-md border border-line-2 px-3 py-1.5 text-xs text-dim transition-colors hover:border-neon/40 hover:text-neon disabled:opacity-40"
        >
          Seleccionar recomendadas
        </button>
        <button
          onClick={load}
          disabled={loading}
          className="rounded-md p-1.5 text-dim transition-colors hover:bg-panel-2 hover:text-ink"
          title="Volver a leer"
        >
          <RefreshCw size={14} className={loading ? "animate-spin" : ""} />
        </button>
      </div>

      {groups.map(
        (g) =>
          g.apps.length > 0 && (
            <section key={g.advice} className="mb-6">
              <h2 className="mb-2 flex items-baseline gap-2">
                <span className={`text-xs font-semibold ${ADVICE[g.advice].cls}`}>
                  {ADVICE[g.advice].title}
                </span>
                <span className="text-xs text-mute">{ADVICE[g.advice].hint}</span>
              </h2>
              <div className="space-y-1.5">{g.apps.map(row)}</div>
            </section>
          ),
      )}

      {others.length > 0 && (
        <section className="mb-6">
          <h2 className="mb-2 flex items-baseline gap-2">
            <span className="text-xs font-semibold text-dim">Otras apps</span>
            <span className="text-xs text-mute">
              No catalogadas: instaladas por el usuario o el fabricante. Revisa antes de quitar.
            </span>
          </h2>
          <div className="space-y-1.5">{others.map(row)}</div>
        </section>
      )}

      {missing.length > 0 && (
        <section className="mb-6">
          <h2 className="mb-2 flex items-baseline gap-2">
            <span className="text-xs font-semibold text-mute">No instaladas</span>
            <span className="text-xs text-mute">Se pueden reinstalar desde Microsoft Store.</span>
          </h2>
          <div className="grid grid-cols-2 gap-1.5">
            {missing.map((a) => (
              <div key={a.package} className="flex items-center gap-3 rounded-lg border border-line bg-panel/50 px-3.5 py-2">
                <span className="min-w-0 flex-1 truncate text-sm text-dim">{a.name}</span>
                <button
                  onClick={() => reinstall(a)}
                  disabled={reinstalling !== null}
                  className="flex items-center gap-1 text-xs text-mute transition-colors hover:text-neon disabled:opacity-40"
                >
                  {reinstalling === a.package ? <Loader2 size={12} className="animate-spin" /> : <Download size={12} />}
                  Reinstalar
                </button>
              </div>
            ))}
          </div>
        </section>
      )}

      {selected.size > 0 && (
        <div className="fixed right-0 bottom-0 left-60 z-30 border-t border-line bg-panel px-6 py-3">
          <div className="mx-auto flex max-w-4xl items-center justify-between">
            {removing ? (
              <TaskStatus task="apps" active={removing} fallback="Quitando apps…" />
            ) : (
            <span className="text-sm text-dim">
              <span className="font-mono text-neon">{selected.size}</span> seleccionadas
              <button onClick={() => setSelected(new Set())} className="ml-3 text-xs text-mute hover:text-ink">
                Limpiar
              </button>
            </span>
            )}
            <button
              onClick={() => remove()}
              disabled={removing}
              className="flex items-center gap-1.5 rounded-md border border-bad/50 px-3.5 py-1.5 text-sm font-medium text-bad transition-colors hover:bg-bad/10 disabled:opacity-50"
            >
              {removing ? <Loader2 size={14} className="animate-spin" /> : <PackageX size={14} />}
              {removing ? "Quitando…" : "Quitar seleccionadas"}
            </button>
          </div>
        </div>
      )}
      {dialog}
    </div>
  );
}
