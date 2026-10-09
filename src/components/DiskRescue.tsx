// Rescate a fondo: recuperar archivos por su firma (discos formateados, tarjetas,
// sistemas de archivos dañados, imágenes) y clonar o hacer una imagen de un disco
// que falla. Lo que se hacía con PhotoRec y con ddrescue.
import { Camera, ChevronDown, ChevronRight, CopyCheck, FileSearch, FolderOutput, Loader2, Play, Square } from "lucide-react";
import { useEffect, useMemo, useRef, useState } from "react";
import { useToast } from "./feedback";
import { SurfaceMap } from "./DiskDeep";
import { TypedConfirm } from "./DiskPartitions";
import { TaskStatus } from "./TaskStatus";
import { Button, Card, inputClass } from "./ui";
import { appApi, carveApi, carveTask, cloneApi, disksApi, type CarveItem, type CarveSource, type CloneLive, type CloneResult, type DiskReport, type RecoverSummary } from "../lib/api";
import { bytes } from "../lib/format";

const GROUPS: [string, string][] = [
  ["photos", "Fotos"],
  ["documents", "Documentos"],
  ["videos", "Vídeos"],
  ["music", "Audio"],
  ["archives", "Comprimidos"],
  ["databases", "Bases de datos"],
];
const PHOTO_EXT = ["jpg", "png", "gif", "bmp", "webp"];
const PAGE = 200;

const diskLabel = (d: DiskReport) => `Disco ${d.number} · ${d.model || d.bus} · ${bytes(d.size, 0)}${d.system ? " · Windows" : ""}`;

/** Miniatura de una foto: solo se pide al disco cuando la fila se ve en pantalla. */
function Thumb({ source, item }: { source: CarveSource; item: CarveItem }) {
  const [src, setSrc] = useState<string | null>(null);
  const box = useRef<HTMLDivElement>(null);
  useEffect(() => {
    const el = box.current;
    if (!el || !PHOTO_EXT.includes(item.ext) || typeof IntersectionObserver === "undefined") return;
    let live = true;
    const io = new IntersectionObserver((entries) => {
      if (entries.some((e) => e.isIntersecting)) {
        io.disconnect();
        carveApi
          .preview(source, item.id)
          .then((s) => live && setSrc(s))
          .catch(() => undefined);
      }
    });
    io.observe(el);
    return () => {
      live = false;
      io.disconnect();
    };
  }, [source, item.id, item.ext]);
  return (
    <div ref={box} className="flex size-10 shrink-0 items-center justify-center overflow-hidden rounded-md bg-panel-2 text-[9px] font-mono uppercase text-mute">
      {src ? <img src={src} alt="" className="size-full object-cover" /> : item.ext}
    </div>
  );
}

/** Buscar archivos por su firma en un disco o en una imagen. `fixed`: la fuente ya está elegida. */
export function CarveCard({ isAdmin, fixed }: { isAdmin: boolean; fixed?: CarveSource }) {
  const toast = useToast();
  const [disks, setDisks] = useState<DiskReport[]>([]);
  const [kind, setKind] = useState<"disk" | "image">("disk");
  const [number, setNumber] = useState<number | null>(null);
  const [image, setImage] = useState("");
  const [groups, setGroups] = useState<string[]>(["photos", "documents"]);
  const [items, setItems] = useState<CarveItem[] | null>(null);
  const [show, setShow] = useState<string>("all");
  const [picked, setPicked] = useState<Set<number>>(new Set());
  const [limit, setLimit] = useState(PAGE);
  const [dest, setDest] = useState("");
  const [busy, setBusy] = useState<"scan" | "copy" | null>(null);
  const [done, setDone] = useState<RecoverSummary | null>(null);

  useEffect(() => {
    if (fixed || !isAdmin) return;
    disksApi
      .status()
      .then((d) => {
        setDisks(d);
        setNumber((n) => n ?? d.find((x) => !x.system)?.number ?? d[0]?.number ?? null);
      })
      .catch(() => undefined);
  }, [fixed, isAdmin]);

  const source: CarveSource | null = useMemo(() => {
    if (fixed) return fixed;
    if (kind === "image") return image.trim() ? { kind: "image", number: 0, path: image.trim(), offset: 0, length: 0 } : null;
    return number === null ? null : { kind: "disk", number, path: "", offset: 0, length: 0 };
  }, [fixed, kind, number, image]);

  const scan = async () => {
    if (!source) return;
    setBusy("scan");
    setItems(null);
    setPicked(new Set());
    setDone(null);
    setLimit(PAGE);
    try {
      const r = await carveApi.scan(source, groups);
      setItems(r);
      setPicked(new Set(r.filter((i) => i.quality === "full").map((i) => i.id)));
    } catch (e) {
      toast("error", String(e));
    } finally {
      setBusy(null);
    }
  };

  const recover = async () => {
    if (!source || !dest.trim()) return;
    setBusy("copy");
    try {
      const r = await carveApi.recover(source, [...picked], dest);
      setDone(r);
      toast("ok", `${r.copied} archivos copiados.`);
    } catch (e) {
      toast("error", String(e));
    } finally {
      setBusy(null);
    }
  };

  const counts = useMemo(() => {
    const m = new Map<string, number>();
    items?.forEach((i) => m.set(i.group, (m.get(i.group) ?? 0) + 1));
    return m;
  }, [items]);
  const visible = (items ?? []).filter((i) => show === "all" || i.group === show);
  const total = [...picked].reduce((s, id) => s + (items?.find((i) => i.id === id)?.length ?? 0), 0);

  return (
    <Card title="Recuperar por firma (a fondo)" icon={<FileSearch size={14} />}>
      <div className="space-y-3 p-4">
        <p className="text-xs leading-relaxed text-dim">
          Para discos formateados, tarjetas de cámara o sistemas de archivos dañados, donde Windows File Recovery ya no encuentra nada. Recorre el disco buscando el principio de cada archivo conocido y mide dónde acaba. Los que están
          <b className="text-ink"> completos</b> se abren con normalidad; los <b className="text-ink">parciales</b> se cortan o estaban troceados en el disco. Solo lee: lo recuperado se copia a otro disco.
        </p>
        {!isAdmin && !fixed && <p className="text-xs text-warn">Leer un disco entero necesita administrador.</p>}
        {!fixed && (
          <div className="grid gap-3 sm:grid-cols-2">
            <div className="space-y-1.5">
              <span className="block text-xs text-dim">Dónde buscar</span>
              <div className="flex gap-4 text-xs">
                <label className="flex items-center gap-1.5">
                  <input type="radio" checked={kind === "disk"} onChange={() => setKind("disk")} className="accent-[var(--color-neon)]" /> Un disco
                </label>
                <label className="flex items-center gap-1.5">
                  <input type="radio" checked={kind === "image"} onChange={() => setKind("image")} className="accent-[var(--color-neon)]" /> Una imagen (.img)
                </label>
              </div>
              {kind === "disk" ? (
                <select value={number ?? ""} onChange={(e) => setNumber(Number(e.target.value))} className={inputClass} aria-label="Disco">
                  {disks.map((d) => (
                    <option key={d.number} value={d.number}>
                      {diskLabel(d)}
                    </option>
                  ))}
                </select>
              ) : (
                <input value={image} onChange={(e) => setImage(e.target.value)} placeholder="E:\Imagenes\cliente.img" className={inputClass} aria-label="Ruta de la imagen" />
              )}
            </div>
            <div className="space-y-1.5">
              <span className="block text-xs text-dim">Qué buscar</span>
              <div className="flex flex-wrap gap-x-4 gap-y-1">
                {GROUPS.map(([g, label]) => (
                  <label key={g} className="flex items-center gap-1.5 text-xs text-ink">
                    <input type="checkbox" checked={groups.includes(g)} onChange={() => setGroups(groups.includes(g) ? groups.filter((x) => x !== g) : [...groups, g])} className="accent-[var(--color-neon)]" />
                    {label}
                  </label>
                ))}
              </div>
            </div>
          </div>
        )}
        <div className="flex flex-wrap items-center gap-2">
          <Button onClick={() => void scan()} disabled={busy !== null || !source || (!fixed && !isAdmin && kind === "disk") || groups.length === 0}>
            {busy === "scan" ? <Loader2 size={14} className="animate-spin" /> : <FileSearch size={14} />} Buscar archivos
          </Button>
          {source && busy === "scan" && (
            <Button kind="danger" size="sm" onClick={() => void appApi.cancelTask(carveTask(source)).catch(() => undefined)}>
              <Square size={12} /> Parar
            </Button>
          )}
        </div>
        {source && <TaskStatus task={carveTask(source)} active={busy === "scan"} cancellable={false} fallback="Recorriendo el disco… (puede tardar horas en discos grandes)" />}

        {items && source && (
          <>
            {items.length === 0 ? (
              <p className="text-sm text-dim">No se encontró nada de lo que se pidió. Si el disco se sobrescribió o es un SSD con TRIM, lo borrado ya no se puede recuperar.</p>
            ) : (
              <>
                <div className="flex flex-wrap items-center gap-1.5">
                  <button type="button" onClick={() => setShow("all")} className={`rounded-full border px-2.5 py-0.5 text-xs ${show === "all" ? "border-neon/50 bg-neon/10 text-neon" : "border-line text-dim"}`}>
                    Todo {items.length}
                  </button>
                  {GROUPS.filter(([g]) => counts.has(g)).map(([g, label]) => (
                    <button key={g} type="button" onClick={() => setShow(g)} className={`rounded-full border px-2.5 py-0.5 text-xs ${show === g ? "border-neon/50 bg-neon/10 text-neon" : "border-line text-dim"}`}>
                      {label} {counts.get(g)}
                    </button>
                  ))}
                  <span className="ml-auto flex gap-2 text-[11px]">
                    <button type="button" className="text-neon hover:underline" onClick={() => setPicked(new Set(visible.filter((i) => i.quality === "full").map((i) => i.id)))}>
                      Elegir los completos
                    </button>
                    <button type="button" className="text-neon hover:underline" onClick={() => setPicked(new Set())}>
                      Ninguno
                    </button>
                  </span>
                </div>
                <ul className="max-h-96 divide-y divide-line/60 overflow-y-auto rounded-lg border border-line">
                  {visible.slice(0, limit).map((i) => (
                    <li key={i.id} className="flex items-center gap-3 px-3 py-1.5 text-xs">
                      <input
                        type="checkbox"
                        checked={picked.has(i.id)}
                        aria-label={`Elegir ${i.label} ${i.id}`}
                        onChange={() => {
                          const n = new Set(picked);
                          if (n.has(i.id)) n.delete(i.id);
                          else n.add(i.id);
                          setPicked(n);
                        }}
                        className="accent-[var(--color-neon)]"
                      />
                      <Thumb source={source} item={i} />
                      <div className="min-w-0 flex-1">
                        <div className="truncate text-ink">{i.label}</div>
                        <div className="font-mono text-[10px] text-mute">
                          {bytes(i.length)} · en {bytes(i.offset)}
                        </div>
                      </div>
                      <span className={`rounded-full border px-2 py-px text-[10px] ${i.quality === "full" ? "border-ok/40 bg-ok/10 text-ok" : "border-warn/40 bg-warn/10 text-warn"}`}>{i.quality === "full" ? "Completo" : "Parcial"}</span>
                    </li>
                  ))}
                </ul>
                {visible.length > limit && (
                  <button type="button" className="text-xs text-neon hover:underline" onClick={() => setLimit(limit + PAGE)}>
                    Mostrar {Math.min(PAGE, visible.length - limit)} más (de {visible.length - limit} sin mostrar)
                  </button>
                )}
                <div className="flex flex-wrap items-end gap-2">
                  <label className="min-w-0 flex-1 text-xs text-dim">
                    Guardar en (otro disco)
                    <input value={dest} onChange={(e) => setDest(e.target.value)} placeholder="E:\Recuperado" className={`mt-1 ${inputClass}`} />
                  </label>
                  <Button kind="secondary" onClick={() => void disksApi.pickFolder().then((p) => p && setDest(p))}>
                    <FolderOutput size={14} /> Elegir carpeta
                  </Button>
                  <Button onClick={() => void recover()} disabled={busy !== null || picked.size === 0 || !dest.trim()}>
                    {busy === "copy" ? <Loader2 size={14} className="animate-spin" /> : <CopyCheck size={14} />} Recuperar {picked.size} ({bytes(total, 0)})
                  </Button>
                </div>
                <TaskStatus task="carve-recover" active={busy === "copy"} fallback="Copiando…" />
                {done && (
                  <p className="rounded-md bg-ok/10 px-3 py-2 text-sm text-ink">
                    {done.copied} archivos copiados ({bytes(done.bytes, 0)}) en {done.folder}.{done.failed > 0 && ` ${done.failed} no se pudieron copiar.`}
                  </p>
                )}
              </>
            )}
          </>
        )}
      </div>
    </Card>
  );
}

// ---------- Clonar ----------

const eta = (s: number) => (s >= 3600 ? `${Math.floor(s / 3600)} h ${Math.floor((s % 3600) / 60)} min` : `${Math.max(1, Math.round(s / 60))} min`);

/** Copiar un disco que falla a una imagen o a otro disco, sin forzarlo. */
export function CloneCard({ isAdmin }: { isAdmin: boolean }) {
  const toast = useToast();
  const [disks, setDisks] = useState<DiskReport[]>([]);
  const [src, setSrc] = useState<number | null>(null);
  const [mode, setMode] = useState<"image" | "disk">("image");
  const [path, setPath] = useState("");
  const [dst, setDst] = useState<number | null>(null);
  const [retries, setRetries] = useState(3);
  const [busy, setBusy] = useState(false);
  const [live, setLive] = useState<CloneLive | null>(null);
  const [last, setLast] = useState<CloneResult | null>(null);
  const [typed, setTyped] = useState(false);
  const [open, setOpen] = useState(false);

  useEffect(() => {
    if (!isAdmin) return;
    disksApi
      .status()
      .then((d) => {
        setDisks(d);
        setSrc((s) => s ?? d.find((x) => !x.system)?.number ?? d[0]?.number ?? null);
      })
      .catch(() => undefined);
  }, [isAdmin]);
  useEffect(() => {
    if (src === null) return;
    cloneApi
      .last(src)
      .then(setLast)
      .catch(() => undefined);
  }, [src]);
  useEffect(() => {
    if (!busy || src === null) return;
    const t = window.setInterval(() => void cloneApi.live(src).then((l) => l && setLive(l)).catch(() => undefined), 1000);
    return () => {
      window.clearInterval(t);
      setLive(null);
    };
  }, [busy, src]);

  const others = disks.filter((d) => d.number !== src && !d.system);
  const target = mode === "image" ? { kind: "image" as const, path: path.trim(), number: 0 } : { kind: "disk" as const, path: "", number: dst ?? -1 };
  const ready = src !== null && (mode === "image" ? path.trim() !== "" : dst !== null);
  const partial = last && !last.finished && last.target === (mode === "image" ? path.trim() : `disco ${dst}`);

  const go = async (resume: boolean) => {
    if (src === null) return;
    setBusy(true);
    try {
      setLast(await cloneApi.start(src, target, retries, resume));
    } catch (e) {
      toast("error", String(e));
    } finally {
      setBusy(false);
    }
  };

  const start = (resume: boolean) => {
    if (mode === "disk" && !resume) setTyped(true);
    else void go(resume);
  };

  return (
    <Card title="Clonar o hacer una imagen de un disco que falla" icon={<Camera size={14} />}>
      <div className="space-y-3 p-4">
        <p className="text-xs leading-relaxed text-dim">
          Con un disco que falla, lo primero es copiarlo entero, y de forma que no lo empeore: primero copia todo lo que se lee de corrido, salta lo que da error, vuelve a ello con calma y al final intenta sector a sector. Lo que no se pueda leer
          queda a ceros y apuntado. El origen solo se lee, y se puede parar y seguir otro día. Después puedes sacar los archivos de la copia sin seguir forzando el disco enfermo.
        </p>
        {!isAdmin && <p className="text-xs text-warn">Leer un disco entero necesita administrador.</p>}
        <div className="grid gap-3 sm:grid-cols-2">
          <label className="block text-xs text-dim">
            Disco que falla (origen, solo lectura)
            <select value={src ?? ""} onChange={(e) => setSrc(Number(e.target.value))} className={`mt-1 ${inputClass}`}>
              {disks.map((d) => (
                <option key={d.number} value={d.number}>
                  {diskLabel(d)}
                </option>
              ))}
            </select>
          </label>
          <div className="space-y-1.5">
            <span className="block text-xs text-dim">Copiar a</span>
            <div className="flex gap-4 text-xs">
              <label className="flex items-center gap-1.5">
                <input type="radio" checked={mode === "image"} onChange={() => setMode("image")} className="accent-[var(--color-neon)]" /> Un archivo de imagen
              </label>
              <label className="flex items-center gap-1.5">
                <input type="radio" checked={mode === "disk"} onChange={() => setMode("disk")} className="accent-[var(--color-neon)]" /> Otro disco
              </label>
            </div>
            {mode === "image" ? (
              <input value={path} onChange={(e) => setPath(e.target.value)} placeholder="E:\Imagenes\cliente.img" className={inputClass} aria-label="Archivo de imagen" />
            ) : (
              <select value={dst ?? ""} onChange={(e) => setDst(Number(e.target.value))} className={inputClass} aria-label="Disco de destino">
                <option value="">Elige el disco de destino…</option>
                {others.map((d) => (
                  <option key={d.number} value={d.number}>
                    {diskLabel(d)}
                  </option>
                ))}
              </select>
            )}
          </div>
        </div>
        <div className="flex flex-wrap items-center gap-2">
          <label className="flex items-center gap-2 text-xs text-dim">
            Intentos por sector malo
            <input type="number" min={1} max={20} value={retries} onChange={(e) => setRetries(Number(e.target.value))} className="h-8 w-16 rounded-md border border-line-2 bg-void px-2 text-right text-xs text-ink" />
          </label>
          <span className="flex-1" />
          {partial && !busy && (
            <Button kind="secondary" onClick={() => start(true)} disabled={!ready}>
              <Play size={13} /> Seguir donde se quedó
            </Button>
          )}
          {!busy ? (
            <Button onClick={() => start(false)} disabled={!ready || !isAdmin}>
              <Camera size={14} /> {mode === "image" ? "Hacer la imagen" : "Clonar al disco"}
            </Button>
          ) : (
            <Button kind="danger" onClick={() => src !== null && void appApi.cancelTask(`disk-clone:${src}`).catch(() => undefined)}>
              <Square size={12} /> Parar
            </Button>
          )}
        </div>
        {src !== null && <TaskStatus task={`disk-clone:${src}`} active={busy} cancellable={false} fallback="Empezando…" />}
        {busy && live && (
          <p className="text-[11px] text-mute">
            Pasada {Math.min(live.pass, 3)} de 3 · {live.percent} % · {live.mbps.toFixed(0)} MB/s · quedan {eta(live.etaSecs)} · copiado {bytes(live.good, 0)} · ilegible {bytes(live.lost, 0)}
          </p>
        )}
        {(busy && live ? live.cells : last?.cells) && (
          <div className="rounded-lg border border-line bg-panel-2 p-3">
            <SurfaceMap cells={(busy && live ? live.cells : last?.cells) ?? ""} />
          </div>
        )}
        {last && !busy && (
          <div className={`rounded-lg border p-3 text-xs ${last.level === "warn" ? "border-warn/40 bg-warn/10" : "border-ok/40 bg-ok/10"}`}>
            <p className={last.level === "warn" ? "text-warn" : "text-ok"}>{last.text}</p>
            <p className="mt-1 text-[11px] text-mute">
              Destino: {last.target} · {bytes(last.size)} · copiado {bytes(last.good, 0)} · ilegible {bytes(last.lost, 0)} en {last.lostRanges} zonas
            </p>
            {last.finished && last.target.includes("\\") && (
              <button type="button" className="mt-2 flex items-center gap-1 text-neon hover:underline" onClick={() => setOpen(!open)}>
                {open ? <ChevronDown size={12} /> : <ChevronRight size={12} />} Sacar archivos de la imagen
              </button>
            )}
          </div>
        )}
        {open && last?.finished && <CarveCard isAdmin={isAdmin} fixed={{ kind: "image", number: 0, path: last.target, offset: 0, length: 0 }} />}
      </div>
      {typed && dst !== null && (
        <TypedConfirm
          title={`Clonar al disco ${dst}`}
          body={`Se BORRARÁ todo lo que hay en el disco ${dst} (${disks.find((d) => d.number === dst)?.model ?? ""}) para dejar una copia exacta del disco ${src}. Comprueba que es el disco de destino correcto: no se puede deshacer.`}
          word={`BORRAR DISCO ${dst}`}
          confirmLabel="Borrar y clonar"
          onClose={() => setTyped(false)}
          onConfirm={() => {
            setTyped(false);
            void go(false);
          }}
        />
      )}
    </Card>
  );
}
