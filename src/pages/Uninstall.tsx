import { ArrowDownUp, CheckCircle2, FileWarning, Loader2, PackageX, RefreshCw, Search, Trash2 } from "lucide-react";
import { useCallback, useEffect, useMemo, useState } from "react";
import { useConfirm, useToast } from "../components/feedback";
import { TaskStatus } from "../components/TaskStatus";
import { Button, Modal } from "../components/ui";
import { bytes } from "../lib/format";
import { programsApi, type InstalledProgram, type Leftover } from "../lib/api";

type Sort = "name" | "size" | "date";

const norm = (s: string) => s.normalize("NFD").replace(/[̀-ͯ]/g, "").toLowerCase();

export function Uninstall({ isAdmin }: { isAdmin: boolean }) {
  const [list, setList] = useState<InstalledProgram[] | null>(null);
  const [loading, setLoading] = useState(false);
  const [query, setQuery] = useState("");
  const [sort, setSort] = useState<Sort>("name");
  const [hideMicrosoft, setHideMicrosoft] = useState(false);
  const [target, setTarget] = useState<InstalledProgram | null>(null);
  const [running, setRunning] = useState<string | null>(null);
  const [leftovers, setLeftovers] = useState<{ program: InstalledProgram; items: Leftover[] } | null>(null);
  const toast = useToast();
  const { confirm, dialog } = useConfirm();

  const load = useCallback(async () => {
    setLoading(true);
    try {
      setList(await programsApi.list());
    } catch (e) {
      toast("error", String(e));
    } finally {
      setLoading(false);
    }
  }, [toast]);

  useEffect(() => {
    load();
  }, [load]);

  const visible = useMemo(() => {
    const q = norm(query.trim());
    const v = (list ?? []).filter(
      (p) => (!hideMicrosoft || !p.publisher?.startsWith("Microsoft")) && (!q || norm(`${p.name} ${p.publisher ?? ""}`).includes(q)),
    );
    if (sort === "size") v.sort((a, b) => (b.size ?? 0) - (a.size ?? 0));
    else if (sort === "date") v.sort((a, b) => (b.installed ?? "").localeCompare(a.installed ?? ""));
    return v;
  }, [list, query, sort, hideMicrosoft]);

  const uninstall = async (p: InstalledProgram, silent: boolean) => {
    setTarget(null);
    setRunning(p.id);
    try {
      const r = await programsApi.uninstall(p.id, silent);
      if (r.removed) {
        toast("ok", `${p.name}: ${r.message}`);
        if (r.leftovers.length) setLeftovers({ program: p, items: r.leftovers });
      } else toast("info", `${p.name}: ${r.message}`);
      await load();
    } catch (e) {
      toast("error", `${p.name}: ${e}`);
    } finally {
      setRunning(null);
    }
  };

  const removeOrphan = async (p: InstalledProgram) => {
    const ok = await confirm({
      title: "Quitar de la lista",
      body: `El desinstalador de «${p.name}» ya no existe, así que el programa probablemente ya no está. Se quitará su entrada (se guarda una copia .reg por si acaso).`,
      confirmLabel: "Quitar",
    });
    if (!ok) return;
    try {
      await programsApi.removeOrphan(p.id);
      toast("ok", `«${p.name}» quitado de la lista.`);
      load();
    } catch (e) {
      toast("error", String(e));
    }
  };

  if (!list) return <p className="p-8 font-mono text-sm text-mute">Leyendo programas instalados…</p>;

  const orphans = list.filter((p) => p.orphan).length;
  const busy = running !== null;

  return (
    <div className="mx-auto max-w-6xl p-6">
      <div className="mb-4 flex flex-wrap items-center gap-3">
        <p className="text-sm text-dim">
          <span className="font-mono text-neon">{list.length}</span> programas
          {orphans > 0 && <span className="text-warn"> · {orphans} entradas huérfanas</span>}
        </p>
        <div className="relative ml-auto w-72">
          <Search size={14} className="absolute top-1/2 left-3 -translate-y-1/2 text-mute" />
          <input
            value={query}
            onChange={(e) => setQuery(e.target.value)}
            placeholder="Buscar programa o editor…"
            className="w-full rounded-md border border-line bg-panel py-1.5 pr-3 pl-8 text-sm text-ink outline-none placeholder:text-mute focus:border-neon/50"
          />
        </div>
        <label className="flex items-center gap-1.5 text-xs text-dim">
          <ArrowDownUp size={12} />
          <select value={sort} onChange={(e) => setSort(e.target.value as Sort)} className="rounded border border-line bg-panel px-1.5 py-1 text-xs text-ink outline-none">
            <option value="name">Nombre</option>
            <option value="size">Tamaño</option>
            <option value="date">Más recientes</option>
          </select>
        </label>
        <label className="flex items-center gap-1.5 text-xs text-dim">
          <input type="checkbox" checked={hideMicrosoft} onChange={(e) => setHideMicrosoft(e.target.checked)} className="accent-[var(--color-neon)]" />
          Ocultar Microsoft
        </label>
        <button onClick={load} disabled={loading || busy} className="rounded-md p-1.5 text-dim hover:bg-panel-2 hover:text-ink" title="Volver a leer">
          <RefreshCw size={14} className={loading ? "animate-spin" : ""} />
        </button>
      </div>

      {running && (
        <div className="mb-3 rounded-lg border border-neon/30 bg-neon/5 px-4 py-2">
          <TaskStatus task={`uninstall:${running}`} active fallback="Desinstalando…" />
        </div>
      )}

      <div className="overflow-hidden rounded-xl border border-line bg-panel">
        {visible.map((p, i) => (
          <div key={p.id} className={`flex items-center gap-4 px-4 py-2 ${i ? "border-t border-line/60" : ""} ${p.orphan ? "opacity-70" : ""}`}>
            <div className="min-w-0 flex-1">
              <div className="flex items-center gap-1.5">
                <span className="truncate text-sm text-ink">{p.name}</span>
                {p.orphan && (
                  <span className="flex shrink-0 items-center gap-1 text-[10px] text-warn" title="El desinstalador ya no existe">
                    <FileWarning size={10} /> huérfana
                  </span>
                )}
                {p.perUser && <span className="shrink-0 rounded border border-line px-1 text-[9px] text-mute">solo este usuario</span>}
              </div>
              <div className="truncate text-[11px] text-mute">
                {[p.publisher, p.version && `v${p.version}`, p.installed].filter(Boolean).join(" · ") || "—"}
              </div>
            </div>
            <div className="w-20 text-right font-mono text-xs text-dim">{p.size ? bytes(p.size) : ""}</div>
            <div className="w-32 text-right">
              {running === p.id ? (
                <Loader2 size={15} className="ml-auto animate-spin text-neon" />
              ) : p.orphan ? (
                <Button kind="ghost" onClick={() => removeOrphan(p)} disabled={busy}>
                  Quitar de la lista
                </Button>
              ) : (
                <Button
                  kind="ghost"
                  onClick={() => setTarget(p)}
                  disabled={busy || (!p.perUser && !isAdmin)}
                  title={!p.perUser && !isAdmin ? "Requiere administrador" : undefined}
                >
                  <Trash2 size={13} /> Desinstalar
                </Button>
              )}
            </div>
          </div>
        ))}
        {visible.length === 0 && <p className="px-4 py-8 text-center text-sm text-mute">Sin resultados.</p>}
      </div>
      <p className="mt-3 text-xs text-mute">
        Las apps de Microsoft Store se quitan desde <span className="text-ink">Bloatware</span>. Tras desinstalar, AdminOps busca carpetas que hayan
        quedado y te deja enviarlas a la papelera.
      </p>

      {target && (
        <Modal
          title={`Desinstalar ${target.name}`}
          onClose={() => setTarget(null)}
          footer={
            <>
              <Button kind="ghost" onClick={() => setTarget(null)}>
                Cancelar
              </Button>
              {target.silent && (
                <Button kind="danger" onClick={() => uninstall(target, true)}>
                  Desinstalar en silencio
                </Button>
              )}
              <Button kind={target.silent ? "ghost" : "danger"} onClick={() => uninstall(target, false)}>
                {target.silent ? "Con su asistente" : "Desinstalar"}
              </Button>
            </>
          }
        >
          <p className="text-sm text-dim">
            {target.silent
              ? "Se puede desinstalar en silencio, sin ventanas. Si prefieres elegir opciones (conservar configuración, etc.), usa su asistente."
              : "Se abrirá el asistente de desinstalación del programa: sigue sus pasos. AdminOps espera a que termine."}
          </p>
          {target.size && <p className="mt-2 text-xs text-mute">Ocupa unos {bytes(target.size)}.</p>}
        </Modal>
      )}

      {leftovers && (
        <LeftoversDialog
          program={leftovers.program}
          items={leftovers.items}
          onClose={() => setLeftovers(null)}
          onDone={(n) => {
            setLeftovers(null);
            toast("ok", `${n} carpetas enviadas a la papelera.`);
          }}
        />
      )}
      {dialog}
    </div>
  );
}

function LeftoversDialog({ program, items, onClose, onDone }: { program: InstalledProgram; items: Leftover[]; onClose: () => void; onDone: (n: number) => void }) {
  const [selected, setSelected] = useState<Set<string>>(new Set(items.map((i) => i.path)));
  const [busy, setBusy] = useState(false);
  const [error, setError] = useState<string | null>(null);
  const total = items.filter((i) => selected.has(i.path)).reduce((a, i) => a + i.size, 0);

  const remove = async () => {
    setBusy(true);
    setError(null);
    try {
      onDone(await programsApi.removeLeftovers(program.id, [...selected]));
    } catch (e) {
      setError(String(e));
      setBusy(false);
    }
  };

  return (
    <Modal
      title="Restos del programa"
      onClose={busy ? () => {} : onClose}
      width="w-[620px]"
      footer={
        <>
          {error && <p className="mr-auto max-w-72 text-xs text-bad">{error}</p>}
          <Button kind="ghost" onClick={onClose} disabled={busy}>
            Dejarlos
          </Button>
          <Button kind="danger" onClick={remove} disabled={busy || !selected.size}>
            {busy ? <Loader2 size={14} className="animate-spin" /> : <PackageX size={14} />} Enviar a la papelera ({bytes(total)})
          </Button>
        </>
      }
    >
      <p className="mb-3 flex items-center gap-2 text-sm text-dim">
        <CheckCircle2 size={14} className="text-ok" /> {program.name} se desinstaló, pero quedaron estas carpetas:
      </p>
      <div className="space-y-1">
        {items.map((i) => (
          <label key={i.path} className="flex cursor-pointer items-center gap-3 rounded-md px-2 py-1.5 hover:bg-panel-2">
            <input
              type="checkbox"
              checked={selected.has(i.path)}
              onChange={() =>
                setSelected((s) => {
                  const n = new Set(s);
                  if (n.has(i.path)) n.delete(i.path);
                  else n.add(i.path);
                  return n;
                })
              }
              className="accent-[var(--color-neon)]"
            />
            <span className="min-w-0 flex-1 truncate font-mono text-xs text-ink" title={i.path}>
              {i.path}
            </span>
            <span className="shrink-0 font-mono text-xs text-dim">{bytes(i.size)}</span>
          </label>
        ))}
      </div>
      <p className="mt-3 text-[11px] text-mute">Van a la papelera: si algo hacía falta, se puede recuperar desde allí.</p>
    </Modal>
  );
}
