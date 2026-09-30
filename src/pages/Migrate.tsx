import { listen } from "@tauri-apps/api/event";
import { ArchiveRestore, CheckCircle2, FolderOpen, HardDriveDownload, Info, Loader2, TriangleAlert } from "lucide-react";
import { useEffect, useMemo, useState } from "react";
import { useToast } from "../components/feedback";
import { TaskStatus } from "../components/TaskStatus";
import { Bar, Button, Card } from "../components/ui";
import { bytes } from "../lib/format";
import { migrateApi, type BackupManifest, type MigrateEstimate, type MigrateSummary } from "../lib/api";
import { useLiveEffect } from "../lib/useLiveEffect";

/** Carpetas que se marcan por defecto (Descargas, Música y Vídeos suelen ser enormes y prescindibles). */
const DEFAULT_ON = new Set(["desktop", "documents", "pictures", "browsers", "wifi"]);

function useMigrateProgress(active: boolean) {
  const [p, setP] = useState<{ item: string; doneBytes: number; totalBytes: number; files: number } | null>(null);
  useEffect(() => {
    if (!active) return;
    setP(null);
    const un = listen<{ item: string; doneBytes: number; totalBytes: number; files: number }>("migrate-progress", (e) => setP(e.payload));
    return () => {
      void un.then((f) => f());
    };
  }, [active]);
  return p;
}

export function Migrate() {
  const [mode, setMode] = useState<"backup" | "restore">("backup");
  return (
    <div className="mx-auto max-w-5xl p-6">
      <div className="mb-4 grid grid-cols-2 gap-3">
        {(
          [
            ["backup", HardDriveDownload, "Hacer una copia", "Del equipo antiguo (o antes de formatear) a un USB o disco."],
            ["restore", ArchiveRestore, "Restaurar una copia", "En el equipo nuevo, sobre el usuario con la sesión abierta."],
          ] as const
        ).map(([id, Icon, title, sub]) => (
          <button
            key={id}
            onClick={() => setMode(id)}
            className={`flex items-start gap-3 rounded-xl border p-4 text-left transition-colors ${mode === id ? "border-neon/60 bg-neon/5" : "border-line bg-panel hover:border-line-2"}`}
          >
            <Icon size={20} className={mode === id ? "text-neon" : "text-dim"} />
            <div>
              <div className={`text-sm font-medium ${mode === id ? "text-neon" : "text-ink"}`}>{title}</div>
              <div className="text-xs text-mute">{sub}</div>
            </div>
          </button>
        ))}
      </div>
      {mode === "backup" ? <Backup /> : <Restore />}
    </div>
  );
}

function Progress({ active }: { active: boolean }) {
  const p = useMigrateProgress(active);
  if (!active) return null;
  const pct = p && p.totalBytes ? (p.doneBytes / p.totalBytes) * 100 : 0;
  return (
    <div className="space-y-2">
      <TaskStatus task="migrate" active={active} fallback="Preparando…" />
      {p && p.totalBytes > 0 && (
        <>
          <Bar value={pct} color="var(--color-neon)" />
          <p className="font-mono text-xs text-mute">
            {p.item} · {bytes(p.doneBytes)} de {bytes(p.totalBytes)} · {p.files.toLocaleString("es")} archivos
          </p>
        </>
      )}
    </div>
  );
}

function Summary({ s, restore }: { s: MigrateSummary; restore?: boolean }) {
  return (
    <Card title={s.cancelled ? "Cancelado" : restore ? "Restauración terminada" : "Copia terminada"} icon={s.cancelled ? <TriangleAlert size={14} /> : <CheckCircle2 size={14} />}>
      <p className="text-sm text-dim">
        <span className="font-mono text-neon">{s.files.toLocaleString("es")}</span> archivos copiados
        {restore && s.unchanged > 0 && <> · {s.unchanged.toLocaleString("es")} ya estaban igual</>}
        {restore && s.renamed > 0 && <> · {s.renamed.toLocaleString("es")} guardados como «(AdminOps)» para no sobrescribir</>}
        {s.errors.length > 0 && <span className="text-warn"> · {s.errors.length} con error</span>}
      </p>
      {!restore && <p className="mt-1 truncate font-mono text-xs text-mute" title={s.folder}>{s.folder}</p>}
      {s.notes.map((n) => (
        <p key={n} className="mt-2 flex items-start gap-1.5 text-xs text-dim">
          <Info size={12} className="mt-0.5 shrink-0 text-neon" /> {n}
        </p>
      ))}
      {s.errors.length > 0 && (
        <details className="mt-2 text-xs">
          <summary className="cursor-pointer text-warn">Ver archivos que no se pudieron copiar</summary>
          <ul className="mt-1 pane-sm overflow-y-auto font-mono text-[11px] text-mute">
            {s.errors.map((e) => (
              <li key={e} className="truncate" title={e}>
                {e}
              </li>
            ))}
          </ul>
        </details>
      )}
    </Card>
  );
}

function Backup() {
  const [profiles, setProfiles] = useState<{ sid: string; name: string; isTarget: boolean }[]>([]);
  const [sid, setSid] = useState<string>("");
  const [items, setItems] = useState<MigrateEstimate[] | null>(null);
  const [selected, setSelected] = useState<Set<string>>(new Set());
  const [dest, setDest] = useState<string | null>(null);
  const [running, setRunning] = useState(false);
  const [summary, setSummary] = useState<MigrateSummary | null>(null);
  const toast = useToast();

  useEffect(() => {
    migrateApi.profiles().then((p) => {
      setProfiles(p);
      setSid(p[0]?.sid ?? "");
    });
  }, []);

  // Al cambiar de usuario, lo que tarde en llegar del anterior se descarta:
  // si no, se verían los datos de un perfil que ya no está seleccionado.
  useLiveEffect(
    (vigente) => {
      if (!sid) return;
      setItems(null);
      migrateApi
        .estimate(sid)
        .then((e) => {
          if (!vigente()) return;
          setItems(e);
          setSelected(new Set(e.filter((i) => i.available && DEFAULT_ON.has(i.id)).map((i) => i.id)));
        })
        .catch((e) => vigente() && toast("error", String(e)));
    },
    [sid, toast],
  );

  const total = useMemo(() => (items ?? []).filter((i) => selected.has(i.id)).reduce((a, i) => a + i.bytes, 0), [items, selected]);

  const start = async () => {
    if (!dest) return;
    setRunning(true);
    setSummary(null);
    try {
      const s = await migrateApi.backup(sid, [...selected], dest);
      setSummary(s);
      if (!s.cancelled) toast(s.errors.length ? "info" : "ok", `Copia terminada: ${s.files.toLocaleString("es")} archivos.`);
    } catch (e) {
      toast("error", String(e));
    } finally {
      setRunning(false);
    }
  };

  const toggle = (id: string) =>
    setSelected((s) => {
      const n = new Set(s);
      if (n.has(id)) n.delete(id);
      else n.add(id);
      return n;
    });

  return (
    <div className="space-y-4">
      <Card title="1 · Qué copiar">
        {profiles.length > 1 && (
          <label className="mb-3 flex items-center gap-2 text-sm text-dim">
            Usuario:
            <select
              value={sid}
              onChange={(e) => setSid(e.target.value)}
              disabled={running}
              className="rounded-md border border-line bg-void/60 px-2 py-1 text-sm text-ink outline-none"
            >
              {profiles.map((p) => (
                <option key={p.sid} value={p.sid}>
                  {p.name}
                  {p.isTarget ? " (sesión abierta)" : ""}
                </option>
              ))}
            </select>
          </label>
        )}
        {!items ? (
          <p className="flex items-center gap-2 text-sm text-mute">
            <Loader2 size={13} className="animate-spin" /> Calculando el tamaño de cada carpeta…
          </p>
        ) : (
          <div className="divide-y divide-line/60">
            {items.map((i) => (
              <label key={i.id} className={`flex items-center gap-3 py-2 ${i.available ? "cursor-pointer" : "opacity-45"}`}>
                <input
                  type="checkbox"
                  checked={selected.has(i.id)}
                  disabled={!i.available || running}
                  onChange={() => toggle(i.id)}
                  className="size-4 accent-[var(--color-neon)]"
                />
                <div className="min-w-0 flex-1">
                  <div className="text-sm text-ink">{i.label}</div>
                  {i.note && <div className="truncate text-[11px] text-mute">{i.note}</div>}
                </div>
                <div className="text-right font-mono text-xs text-dim">
                  {i.bytes > 0 && <div>{bytes(i.bytes)}</div>}
                  <div className="text-mute">
                    {i.files.toLocaleString("es")} {i.id === "wifi" ? "redes" : i.id === "browsers" ? "perfiles" : "archivos"}
                  </div>
                </div>
              </label>
            ))}
          </div>
        )}
      </Card>

      <Card title="2 · Dónde guardarla">
        <div className="flex items-center gap-3">
          <Button kind="ghost" onClick={() => migrateApi.pickFolder().then((f) => f && setDest(f))} disabled={running}>
            <FolderOpen size={14} /> Elegir USB o carpeta…
          </Button>
          <span className="min-w-0 flex-1 truncate font-mono text-xs text-dim" title={dest ?? undefined}>
            {dest ?? "Ninguna carpeta elegida"}
          </span>
        </div>
        <p className="mt-2 text-[11px] text-mute">Se crea dentro una carpeta «AdminOps - Copia de…» con la fecha. No se borra ni modifica nada del equipo.</p>
      </Card>

      <div className="flex items-center gap-4">
        <div className="min-w-0 flex-1">
          {running ? <Progress active={running} /> : <p className="text-sm text-dim">Total aproximado: <span className="font-mono text-neon">{bytes(total)}</span></p>}
        </div>
        <Button onClick={start} disabled={running || !dest || !selected.size}>
          <HardDriveDownload size={14} /> Hacer la copia
        </Button>
      </div>
      {summary && <Summary s={summary} />}
    </div>
  );
}

function Restore() {
  const [folder, setFolder] = useState<string | null>(null);
  const [manifest, setManifest] = useState<BackupManifest | null>(null);
  const [selected, setSelected] = useState<Set<string>>(new Set());
  const [target, setTarget] = useState<string | null>(null);
  const [running, setRunning] = useState(false);
  const [summary, setSummary] = useState<MigrateSummary | null>(null);
  const toast = useToast();

  useEffect(() => {
    void migrateApi.profiles().then((p) => setTarget(p.find((x) => x.isTarget)?.name ?? null));
  }, []);

  const pick = async () => {
    const f = await migrateApi.pickFolder();
    if (!f) return;
    try {
      const m = await migrateApi.readBackup(f);
      setFolder(f);
      setManifest(m);
      setSelected(new Set(m.items.map((i) => i.id)));
      setSummary(null);
    } catch (e) {
      toast("error", String(e));
    }
  };

  const start = async () => {
    if (!folder) return;
    setRunning(true);
    setSummary(null);
    try {
      const s = await migrateApi.restore(folder, [...selected]);
      setSummary(s);
      if (!s.cancelled) toast(s.errors.length ? "info" : "ok", `Restauración terminada: ${s.files.toLocaleString("es")} archivos.`);
    } catch (e) {
      toast("error", String(e));
    } finally {
      setRunning(false);
    }
  };

  return (
    <div className="space-y-4">
      <Card title="1 · Elegir la copia">
        <div className="flex items-center gap-3">
          <Button kind="ghost" onClick={pick} disabled={running}>
            <FolderOpen size={14} /> Elegir la carpeta «AdminOps - Copia de…»
          </Button>
          <span className="min-w-0 flex-1 truncate font-mono text-xs text-dim" title={folder ?? undefined}>
            {folder ?? "Ninguna copia elegida"}
          </span>
        </div>
        {manifest && (
          <>
            <p className="mt-3 text-sm text-dim">
              Copia de <span className="text-ink">{manifest.user}</span> en <span className="text-ink">{manifest.computer}</span>, hecha el{" "}
              {new Date(manifest.created).toLocaleString("es", { dateStyle: "long", timeStyle: "short" })}.
            </p>
            <div className="mt-2 divide-y divide-line/60">
              {manifest.items.map((i) => (
                <label key={i.id} className="flex cursor-pointer items-center gap-3 py-2">
                  <input
                    type="checkbox"
                    checked={selected.has(i.id)}
                    disabled={running}
                    onChange={() =>
                      setSelected((s) => {
                        const n = new Set(s);
                        if (n.has(i.id)) n.delete(i.id);
                        else n.add(i.id);
                        return n;
                      })
                    }
                    className="size-4 accent-[var(--color-neon)]"
                  />
                  <span className="flex-1 text-sm text-ink">{i.label}</span>
                  <span className="font-mono text-xs text-dim">
                    {i.bytes > 0 && `${bytes(i.bytes)} · `}
                    {i.files.toLocaleString("es")}
                  </span>
                </label>
              ))}
            </div>
          </>
        )}
      </Card>

      <Card title="2 · Restaurar">
        <p className="text-sm text-dim">
          Se restaurará en el usuario con la sesión abierta: <span className="text-ink">{target ?? "…"}</span>. Los archivos se añaden a sus carpetas
          (Escritorio, Documentos…): si ya existe uno distinto con el mismo nombre, se guarda como «nombre (AdminOps)» y nunca se sobrescribe.
        </p>
        <p className="mt-2 text-xs text-mute">Cierra los navegadores antes de restaurar los marcadores. Las redes Wi-Fi requieren administrador.</p>
      </Card>

      <div className="flex items-center gap-4">
        <div className="min-w-0 flex-1">
          <Progress active={running} />
        </div>
        <Button onClick={start} disabled={running || !manifest || !selected.size}>
          <ArchiveRestore size={14} /> Restaurar
        </Button>
      </div>
      {summary && <Summary s={summary} restore />}
    </div>
  );
}
