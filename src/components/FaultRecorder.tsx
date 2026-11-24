import { convertFileSrc } from "@tauri-apps/api/core";
import { listen } from "@tauri-apps/api/event";
import { AppWindow, ArrowLeft, Check, ChevronRight, Circle, Eye, FileText, Film, FolderOutput, Mic, MousePointerClick, RefreshCw, Scissors, SquareDashed, StickyNote, Video } from "lucide-react";
import { useCallback, useEffect, useRef, useState, type ReactNode } from "react";
import { casesApi, faultRecApi, FAULTREC_AUTO_EVENT, FAULTREC_DONE_EVENT, FAULTREC_REGION_EVENT, type FaultAutoSaved, FAULTREC_TICK_EVENT, type FaultInput, type FaultMark, type FaultRecDone, type FaultRecMonitor, type FaultRecStatus, type FaultRecWindow, type FaultRecording, type FaultRecordingDetail, type FaultRegion, type FaultRegionPicked } from "../lib/api";
import { logQuietly } from "../lib/api/core";
import { CASE_CHANGED_EVENT, caseChanged } from "../lib/currentCase";
import { areaPath, axisTicks, bucketMax, caseNote, clock, eraseWhen, getRecOptions, inputLine, inputOverlay, KIND_STYLE, lengthLabel, markLine, markPercent, marksCount, MAX_SECS, monitorLabel, originLabel, pickRecording, rangeLabel, recSummary, regionLabel, remaining, seekTime, setRecOptions, sortedMarks, tracksOf, videoBox, whenLabel, type RecOptions, type RecTarget } from "../lib/faultrec";
import { usePageActive } from "../lib/pageActive";
import { useConfirm, useToast } from "./feedback";
import { Button, Card, ErrorState, Intro, Loading, inputClass, Toggle } from "./ui";
import { FaultWatch } from "./FaultWatch";
import { timeOfDay } from "../lib/format";

const NO_VIDEO_IN_DEMO = "En la demo no hay vídeo; las marcas sí funcionan";

/** Una fila de la pantalla de preparar: lo que se va a grabar y lo que pasa con ello. */
function Row({ label, children }: { label: string; children: ReactNode }) {
  return (
    <div className="flex flex-wrap items-center gap-x-4 gap-y-1 border-t border-line/60 py-2.5 text-[13px]">
      <dt className="w-24 shrink-0 text-mute">{label}</dt>
      <dd className="min-w-0 flex-1 text-ink">{children}</dd>
    </div>
  );
}

/** Los clics (un círculo que se abre) y las teclas (abajo) del momento `ms`, encima de la imagen del vídeo. */
function InputLayer({ video, events, ms }: { video: HTMLVideoElement | null; events: FaultInput[]; ms: number }) {
  if (!video || !events.length) return null;
  const box = videoBox(video.clientWidth, video.clientHeight, video.videoWidth, video.videoHeight);
  const { clicks, keys } = inputOverlay(events, ms);
  return (
    <div aria-hidden className="pointer-events-none absolute overflow-hidden" style={{ left: box.left, top: box.top, width: box.width, height: box.height }}>
      {clicks.map(({ e, age }) => (
        <span
          key={e.t}
          className={`absolute rounded-full border-2 ${e.label === "Clic derecho" ? "border-warn" : "border-neon"}`}
          style={{ left: `${e.x * 100}%`, top: `${e.y * 100}%`, width: 30, height: 30, opacity: 1 - age, transform: `translate(-50%, -50%) scale(${0.5 + age})` }}
        />
      ))}
      {keys && (
        <span className="absolute bottom-[6%] left-1/2 -translate-x-1/2 rounded-lg bg-black/75 px-3 py-1.5 font-mono text-[15px] font-medium tracking-wide whitespace-nowrap text-white shadow-lg">
          {keys.label}
        </span>
      )}
    </div>
  );
}

const LABEL_W = 92;
/** Colores de cada pista (clases enteras para que Tailwind las encuentre). */
const TRACK_FILL: Record<string, string> = { cpu: "fill-neon/35", disk: "fill-warn/35", mem: "fill-ok/30", net: "fill-ink/25" };

function TrackRow({ label, children, title }: { label: string; children: ReactNode; title?: string }) {
  return (
    <div className="flex items-center" title={title}>
      <span className="shrink-0 pr-2 text-[10.5px] text-mute" style={{ width: LABEL_W }}>
        {label}
      </span>
      <div className="relative h-6 min-w-0 flex-1 rounded-sm bg-panel-2/50">{children}</div>
    </div>
  );
}

/**
 * Las pistas bajo el vídeo (1.4, idea 12), como un monitor de rendimiento: procesador, disco, memoria y red de
 * cada segundo, los errores, los clics y teclas y la voz, todas alineadas con el vídeo. Un clic lleva a ese segundo;
 * arrastrar marca un trozo para recortar (idea 14).
 */
function TrackPanel({ rec, pos, sel, onSeek, onMark, onSelect }: { rec: FaultRecording; pos: number; sel: [number, number] | null; onSeek: (secs: number) => void; onMark: (m: FaultMark) => void; onSelect: (r: [number, number] | null) => void }) {
  const secs = Math.max(1, rec.secs);
  const tracks = tracksOf(rec.samples ?? [], secs);
  const marks = sortedMarks(rec.marks);
  const input = rec.input ?? [];
  // La voz va por décimas: en una grabación de 2 minutos son 1200 barras, que no caben; se juntan en 160.
  const voice = bucketMax(rec.voice ?? [], 160);
  const area = useRef<HTMLDivElement>(null);
  const drag = useRef<{ from: number; moved: boolean } | null>(null);
  const timeAt = (x: number) => {
    const r = area.current?.getBoundingClientRect();
    if (!r || r.width <= 0) return 0;
    return Math.min(secs, Math.max(0, ((x - r.left) / r.width) * secs));
  };
  const now = Math.min(secs - 1, Math.max(0, Math.floor(pos)));
  const pctOf = (t: number) => `${markPercent(t, secs)}%`;
  return (
    <div className="select-none">
      <div className="relative space-y-1">
        {tracks.map((t) => (
          <TrackRow key={t.key} label={t.label} title={`${t.label} en ${clock(now)}: ${t.values[now] == null ? "sin dato" : t.format(t.values[now]!)}`}>
            <svg viewBox={`0 0 ${t.values.length} 24`} preserveAspectRatio="none" className="absolute inset-0 h-full w-full" aria-hidden>
              <path d={areaPath(t.values, t.max, 24)} className={TRACK_FILL[t.key]} />
            </svg>
          </TrackRow>
        ))}
        <TrackRow label="Errores">
          {marks.map((m, i) => (
            <button
              key={`${m.t}-${i}`}
              type="button"
              onClick={() => onMark(m)}
              title={markLine(m)}
              aria-label={`${markLine(m)}. Ir a ese momento del vídeo`}
              className={`absolute top-1/2 z-20 h-3.5 w-3.5 -translate-x-1/2 -translate-y-1/2 rounded-full border-2 border-panel ${KIND_STYLE[m.kind].dot} ${m.probable ? "ring-2 ring-warn/60" : ""}`}
              style={{ left: pctOf(m.t) }}
            />
          ))}
        </TrackRow>
        {input.length > 0 && (
          <TrackRow label="Clics y teclas">
            {input.map((e, i) => (
              <span key={`${e.t}-${i}`} title={inputLine(e)} className={`absolute top-1 h-4 w-0.5 -translate-x-1/2 rounded-full ${e.kind === "click" ? "bg-neon/70" : "bg-ink/50"}`} style={{ left: pctOf(e.t / 1000) }} />
            ))}
          </TrackRow>
        )}
        {voice.length > 0 && (
          <TrackRow label="Voz">
            <div className="absolute inset-0 flex items-center gap-px overflow-hidden px-px">
              {voice.map((v, i) => (
                <span key={i} className="min-w-px flex-1 rounded-full bg-ok/60" style={{ height: `${Math.max(8, Math.min(100, v))}%` }} />
              ))}
            </div>
          </TrackRow>
        )}
        {/* Encima de todas las pistas: el cabezal, el trozo elegido y el ratón (clic: ir; arrastrar: recortar). */}
        <div
          ref={area}
          className="absolute inset-y-0 right-0 z-10 cursor-crosshair"
          style={{ left: LABEL_W }}
          onPointerDown={(e) => {
            if (e.button !== 0) return;
            e.currentTarget.setPointerCapture(e.pointerId);
            drag.current = { from: timeAt(e.clientX), moved: false };
          }}
          onPointerMove={(e) => {
            const d = drag.current;
            if (!d) return;
            const t = timeAt(e.clientX);
            if (Math.abs(t - d.from) * (area.current?.clientWidth ?? 0) / secs < 4 && !d.moved) return;
            d.moved = true;
            onSelect([Math.min(d.from, t), Math.max(d.from, t)]);
          }}
          onPointerUp={(e) => {
            const d = drag.current;
            drag.current = null;
            if (d && !d.moved) onSeek(timeAt(e.clientX));
          }}
        >
          {sel && (
            <div className="pointer-events-none absolute inset-y-0 border-x-2 border-neon bg-neon/10" style={{ left: pctOf(sel[0]), width: `${markPercent(sel[1], secs) - markPercent(sel[0], secs)}%` }} />
          )}
          <div className="pointer-events-none absolute inset-y-0 w-px bg-ink/80" style={{ left: pctOf(pos) }} />
        </div>
      </div>
      <div className="relative mt-0.5 h-4" style={{ marginLeft: LABEL_W }} aria-hidden>
        {axisTicks(rec.secs).map((t) => (
          <span key={t.t} className="absolute top-0 -translate-x-1/2 text-[10px] tabular text-mute" style={{ left: `${t.pct}%` }}>
            {t.label}
          </span>
        ))}
      </div>
      {tracks.length > 0 && (
        <p className="mt-1 text-[11px] tabular text-dim" style={{ marginLeft: LABEL_W }}>
          {clock(now)} · {tracks.map((t) => `${t.label} ${t.values[now] == null ? "—" : t.format(t.values[now]!)}`).join(" · ")}
        </p>
      )}
    </div>
  );
}

/** El trozo elegido y lo que se hace con él: al informe, al caso o a un MP4 (idea 14). */
function ClipBar({ detail, sel, onClear }: { detail: FaultRecordingDetail; sel: [number, number] | null; onClear: () => void }) {
  const rec = detail.meta;
  const toast = useToast();
  const [inReport, setInReport] = useState<[number, number] | null>(detail.clip ?? null);
  const [busy, setBusy] = useState<"report" | "case" | "export" | null>(null);
  const [a, b] = sel ?? [0, rec.secs];
  const run = async (kind: "report" | "case" | "export", fn: () => Promise<void>) => {
    setBusy(kind);
    try {
      await fn();
    } catch (e) {
      toast("error", String(e));
    } finally {
      setBusy(null);
    }
  };
  const toReport = () =>
    run("report", async () => {
      await faultRecApi.toReport(rec.id, a * 1000, b * 1000);
      setInReport([a * 1000, b * 1000]);
      toast("ok", "Va en el informe del cliente: tres fotogramas del trozo y sus marcas. El vídeo no sale del equipo.");
    });
  const outOfReport = () =>
    run("report", async () => {
      await faultRecApi.reportRemove(rec.id);
      setInReport(null);
    });
  const toCase = () =>
    run("case", async () => {
      const c = await casesApi.current();
      if (!c) {
        toast("info", "No hay ningún caso abierto: ábrelo y vuelve a pulsar «Al caso».");
        return;
      }
      const line = caseNote(rec, a, b);
      await casesApi.update({ ...c, notes: c.notes.trim() ? `${c.notes.trimEnd()}\n${line}` : line });
      caseChanged();
      toast("ok", "Añadido a las notas del caso.");
    });
  const exportMp4 = () =>
    run("export", async () => {
      const r = await faultRecApi.exportClip(rec.id, a * 1000, b * 1000);
      // Solo el nombre del archivo: la ruta lleva el usuario de Windows.
      if (r) toast("ok", `Guardado como «${r.path.split(/[\\/]/).pop() ?? "grabación.mp4"}».`);
    });
  return (
    <div className="mt-3 flex flex-wrap items-center gap-x-3 gap-y-2 rounded-lg border border-line/70 bg-panel-2/40 px-3 py-2 text-[13px]">
      <span className="flex items-center gap-1.5 text-ink tabular">
        <Scissors size={13} className="text-mute" />
        {sel ? rangeLabel(a, b) : `Toda la grabación · ${lengthLabel(rec.secs)}`}
      </span>
      {sel ? (
        <button type="button" onClick={onClear} className="text-xs text-neon hover:underline">
          Quitar el recorte
        </button>
      ) : (
        <span className="text-xs text-mute">Arrastra sobre las pistas para quedarte con un trozo</span>
      )}
      <span className="ml-auto flex flex-wrap items-center gap-2">
        {inReport ? (
          <span className="flex items-center gap-1.5 text-xs text-ok">
            <Check size={12} /> En el informe ({rangeLabel(inReport[0] / 1000, inReport[1] / 1000)})
            <button type="button" onClick={() => void outOfReport()} disabled={busy !== null} className="text-mute hover:text-ink hover:underline">
              Quitar
            </button>
            <button type="button" onClick={() => void toReport()} disabled={busy !== null} className="text-neon hover:underline">
              Cambiar por este trozo
            </button>
          </span>
        ) : (
          <Button kind="secondary" size="sm" onClick={() => void toReport()} disabled={busy !== null}>
            <FileText size={13} /> {busy === "report" ? "Preparando…" : "Al informe"}
          </Button>
        )}
        <Button kind="secondary" size="sm" onClick={() => void toCase()} disabled={busy !== null}>
          <StickyNote size={13} /> Al caso
        </Button>
        <Button kind="secondary" size="sm" onClick={() => void exportMp4()} disabled={busy !== null}>
          <FolderOutput size={13} /> {busy === "export" ? "Exportando…" : "Exportar MP4"}
        </Button>
      </span>
    </div>
  );
}

/** La miniatura de una grabación (se pide una vez; si no hay, un icono). */
function Thumb({ id }: { id: string }) {
  const [src, setSrc] = useState<string | null>(null);
  useEffect(() => {
    let alive = true;
    faultRecApi.thumb(id).then(
      (s) => alive && typeof s === "string" && setSrc(s),
      () => {},
    );
    return () => {
      alive = false;
    };
  }, [id]);
  return src ? <img src={src} alt="" className="h-full w-full object-cover" /> : <Film size={18} className="text-mute" aria-hidden />;
}

/** La pantalla como lista (idea 16): cada grabación con su miniatura, lo que dura, qué pasó y la causa probable. */
function RecList({ list, hasCase, onOpen }: { list: FaultRecording[]; hasCase: boolean; onOpen: (id: string) => void }) {
  return (
    <div className="mt-4">
      <ul className="space-y-1.5">
        {list.map((r) => {
          const sum = recSummary(r);
          const origin = originLabel(r);
          return (
            <li key={r.id}>
              <button type="button" onClick={() => onOpen(r.id)} className="flex w-full items-center gap-3 rounded-lg border border-line/70 p-2 text-left transition-colors hover:border-line-2 hover:bg-panel-2/40">
                <span className="relative grid aspect-video w-28 shrink-0 place-items-center overflow-hidden rounded-md bg-void">
                  <Thumb id={r.id} />
                  <span className="absolute right-1 bottom-1 rounded bg-black/70 px-1 font-mono text-[10px] text-white">{clock(r.secs)}</span>
                </span>
                <span className="min-w-0 flex-1">
                  <span className="block text-[13px] font-medium text-ink">{sum.what}</span>
                  <span className="block text-xs text-dim">
                    {whenLabel(r.started)}
                    {sum.probable ? ` · ${sum.probable}` : r.marks.length ? ` · ${marksCount(r.marks.length)}` : ""}
                  </span>
                  {origin && <span className={`mt-0.5 block text-[11px] ${r.origin === "auto" ? "text-warn" : "text-mute"}`}>{origin}</span>}
                </span>
                <ChevronRight size={15} className="shrink-0 text-mute" />
              </button>
            </li>
          );
        })}
      </ul>
      <p className="mt-2 text-xs text-mute">
        {list.length === 1 ? "1 grabación" : `${list.length} grabaciones`} {hasCase ? "de este caso · se borran al cerrarlo" : "· se borran al cerrar AdminOps"}
      </p>
    </div>
  );
}

/** Una grabación con su vídeo, su línea de tiempo y sus marcas. */
function Result({ detail, onAgain, onDelete, onBack }: { detail: FaultRecordingDetail; onAgain: () => void; onDelete: () => Promise<void>; onBack: () => void }) {
  const rec = detail.meta;
  const toast = useToast();
  const [ignored, setIgnored] = useState(false);
  const ignore = () => {
    if (!rec.triggerKey) return;
    faultRecApi.autoIgnore(rec.triggerKey, rec.trigger ?? rec.triggerKey).then(
      () => {
        setIgnored(true);
        toast("ok", "Ya no se graba sola por esto. Se puede volver a activar en Vigilar.");
      },
      (e) => toast("error", String(e)),
    );
  };
  const video = useRef<HTMLVideoElement>(null);
  const [pos, setPos] = useState(0);
  const [sel, setSel] = useState<[number, number] | null>(null);
  const [videoError, setVideoError] = useState(false);
  const active = usePageActive();
  const hasVideo = detail.path !== "";
  const marks = sortedMarks(rec.marks);
  const input = rec.input ?? [];
  // El milisegundo exacto, para los clics y las teclas: «timeupdate» llega cuatro veces por segundo, poco para un clic.
  const [ms, setMs] = useState(0);
  const [playing, setPlaying] = useState(false);
  useEffect(() => {
    if (!playing || !input.length) return;
    let raf = 0;
    const step = () => {
      if (video.current) setMs(video.current.currentTime * 1000);
      raf = requestAnimationFrame(step);
    };
    raf = requestAnimationFrame(step);
    return () => cancelAnimationFrame(raf);
  }, [playing, input.length]);

  // Al salir de la pestaña el vídeo se pausa.
  useEffect(() => {
    if (!active) video.current?.pause();
  }, [active]);

  const seek = (m: FaultMark) => seekTo(seekTime(m.t));
  const seekTo = (to: number) => {
    setPos(to);
    setMs(to * 1000);
    const v = video.current;
    if (!v) return;
    v.currentTime = to;
    v.play().catch(logQuietly("FaultRecorder.play"));
  };

  return (
    <Card
      title={`Grabación · ${lengthLabel(rec.secs)}`}
      icon={<Video size={14} />}
      right={
        <button type="button" onClick={onBack} className="flex items-center gap-1 text-xs text-mute hover:text-ink">
          <ArrowLeft size={12} /> Todas las grabaciones
        </button>
      }
    >
      <p className="-mt-2 mb-3 text-xs text-mute">
        {whenLabel(rec.started)}
        {originLabel(rec) ? ` · ${originLabel(rec)}` : ""}
        {rec.target ? ` · ${rec.target}` : ""}
        {rec.monitor && !rec.target?.startsWith("Ventana") ? ` · ${rec.monitor}` : ""}
        {rec.mic ? " · con voz" : ""}
      </p>
      {hasVideo ? (
        videoError ? (
          <div className="grid aspect-video max-h-[50vh] w-full place-items-center rounded-lg bg-void px-4 text-center text-sm text-dim">No se pudo reproducir el vídeo. Las marcas siguen aquí debajo.</div>
        ) : (
          <div className="relative">
            <video
              ref={video}
              src={convertFileSrc(detail.path)}
              controls
              preload="metadata"
              className="max-h-[50vh] w-full rounded-lg bg-void"
              onTimeUpdate={(e) => {
                setPos(e.currentTarget.currentTime);
                if (e.currentTarget.paused) setMs(e.currentTarget.currentTime * 1000);
              }}
              onPlay={() => setPlaying(true)}
              onPause={() => setPlaying(false)}
              onEnded={() => setPlaying(false)}
              onSeeked={(e) => setMs(e.currentTarget.currentTime * 1000)}
              onError={() => setVideoError(true)}
            >
              <track kind="captions" />
            </video>
            <InputLayer video={video.current} events={input} ms={ms} />
          </div>
        )
      ) : (
        <div className="grid aspect-video max-h-[50vh] w-full place-items-center rounded-lg bg-void px-4 text-center text-sm text-dim">{NO_VIDEO_IN_DEMO}</div>
      )}

      <div className="mt-3">
        <TrackPanel rec={rec} pos={pos} sel={sel} onSeek={(t) => seekTo(t)} onMark={seek} onSelect={setSel} />
        <ClipBar detail={detail} sel={sel} onClear={() => setSel(null)} />
      </div>

      {rec.note && <p className="mt-2 text-xs text-warn">{rec.note}</p>}

      {marks.length === 0 ? (
        <p className="mt-3 text-[13px] text-dim">No se apuntó nada raro mientras se grababa: ni errores, ni programas colgados, ni picos de procesador o de disco.</p>
      ) : (
        <ul className="mt-3 divide-y divide-line/60">
          {marks.map((m, i) => (
            <li key={`${m.t}-${i}`}>
              <button type="button" onClick={() => seek(m)} className="flex w-full items-start gap-3 rounded-md px-1 py-2 text-left hover:bg-panel-2/60">
                <span className={`mt-1.5 h-2 w-2 shrink-0 rounded-full ${KIND_STYLE[m.kind].dot}`} aria-hidden />
                <span className="min-w-0 flex-1">
                  <span className="block text-[13px] text-ink">{markLine(m)}</span>
                  {m.detail && <span className="block text-xs text-dim">{m.detail}</span>}
                </span>
                {m.probable && <span className="shrink-0 rounded-md bg-warn/15 px-1.5 py-0.5 text-[11px] font-medium text-warn">Causa probable</span>}
              </button>
            </li>
          ))}
        </ul>
      )}

      <div className="mt-4 flex flex-wrap items-center gap-2">
        <Button kt="G" onClick={onAgain}>
          Grabar otra vez
        </Button>
        <Button kt="B" kind="secondary" onClick={onDelete}>
          Borrar grabación
        </Button>
        {rec.origin === "auto" && rec.triggerKey && (
          <Button kind="ghost" onClick={ignore} disabled={ignored}>
            {ignored ? "Ya no se graba sola por esto" : "No volver a grabar por esto"}
          </Button>
        )}
      </div>
    </Card>
  );
}

const TARGETS: { id: RecTarget; label: string; icon: ReactNode }[] = [
  { id: "screen", label: "Pantalla entera", icon: <Video size={13} /> },
  { id: "window", label: "Una ventana", icon: <AppWindow size={13} /> },
  { id: "region", label: "Una zona", icon: <SquareDashed size={13} /> },
];

/** Lo que se eligió para grabar y lo que hace falta para empezar. */
interface Choice {
  opts: RecOptions;
  monitorId: string;
  window: number | null;
  region: { monitor: string | null; region: FaultRegion } | null;
}

/** Se puede empezar: con ventana, una elegida; con zona, una dibujada. */
const ready = (c: Choice) => (c.opts.target === "window" ? c.window !== null : c.opts.target === "region" ? c.region !== null : true);

/** Pantalla entera, una ventana o una zona (idea 11); clics y teclas (13); la voz (15). */
function Setup({ monitors, choice, onChange }: { monitors: FaultRecMonitor[]; choice: Choice; onChange: (c: Choice) => void }) {
  const [windows, setWindows] = useState<FaultRecWindow[] | null>(null);
  const [picking, setPicking] = useState(false);
  const toast = useToast();
  const { opts } = choice;
  const setOpts = (o: Partial<RecOptions>) => {
    const next = { ...opts, ...o };
    setRecOptions(next);
    onChange({ ...choice, opts: next });
  };

  const loadWindows = useCallback(() => {
    faultRecApi.windows().then(setWindows, (e) => {
      setWindows([]);
      logQuietly("FaultRecorder.windows")(e);
    });
  }, []);
  useEffect(() => {
    if (opts.target === "window" && windows === null) loadWindows();
  }, [opts.target, windows, loadWindows]);
  // La ventana elegida se cerró: se quita.
  useEffect(() => {
    if (windows && choice.window !== null && !windows.some((w) => w.id === choice.window)) onChange({ ...choice, window: null });
  }, [windows, choice, onChange]);

  // La capa de la zona responde con un aviso.
  const choiceRef = useRef(choice);
  choiceRef.current = choice;
  useEffect(() => {
    const off = listen<FaultRegionPicked>(FAULTREC_REGION_EVENT, (e) => {
      setPicking(false);
      if (e.payload.region) onChange({ ...choiceRef.current, region: { monitor: e.payload.monitor, region: e.payload.region } });
    });
    return () => void off.then((f) => f());
  }, [onChange]);

  const drawRegion = () => {
    setPicking(true);
    faultRecApi.pickRegion(choice.monitorId || null).catch((e) => {
      setPicking(false);
      toast("error", String(e));
    });
  };

  const monitorSelect = monitors.length > 1 && (
    <select aria-label="Pantalla" className={`${inputClass} max-w-sm`} value={choice.monitorId} onChange={(e) => onChange({ ...choice, monitorId: e.target.value, region: null })}>
      <option value="">{monitorLabel(monitors.find((m) => m.primary) ?? monitors[0], monitors.length)}</option>
      {monitors
        .filter((m) => !m.primary)
        .map((m) => (
          <option key={m.id} value={m.id}>
            {monitorLabel(m, monitors.length)}
          </option>
        ))}
    </select>
  );

  return (
    <>
      <Row label="Qué grabar">
        <div className="inline-flex rounded-lg border border-line bg-void p-0.5" role="radiogroup" aria-label="Qué grabar">
          {TARGETS.map((t) => (
            <button
              key={t.id}
              type="button"
              role="radio"
              aria-checked={opts.target === t.id}
              onClick={() => setOpts({ target: t.id })}
              className={`flex items-center gap-1.5 rounded-md px-3 py-1.5 text-[12.5px] transition-colors ${opts.target === t.id ? "bg-panel-2 font-medium text-ink" : "text-dim hover:text-ink"}`}
            >
              {t.icon}
              {t.label}
            </button>
          ))}
        </div>
      </Row>

      {opts.target === "screen" && <Row label="Pantalla">{monitorSelect || "Monitor principal"}</Row>}

      {opts.target === "window" && (
        <Row label="Ventana">
          <div className="space-y-1.5">
            {windows === null ? (
              <Loading text="Buscando las ventanas abiertas…" />
            ) : windows.length === 0 ? (
              <p className="text-dim">No hay ninguna ventana abierta que se pueda grabar. Abre el programa que falla y actualiza la lista.</p>
            ) : (
              <ul className="max-h-56 max-w-xl space-y-1 overflow-y-auto pr-1" role="radiogroup" aria-label="Ventana que se graba">
                {windows.map((w) => {
                  const on = choice.window === w.id;
                  return (
                    <li key={w.id}>
                      <button
                        type="button"
                        role="radio"
                        aria-checked={on}
                        onClick={() => onChange({ ...choice, window: w.id })}
                        className={`flex w-full items-center gap-2.5 rounded-lg border px-3 py-2 text-left transition-colors ${on ? "border-neon/50 bg-neon/10" : "border-line hover:border-line-2"}`}
                      >
                        <span className={`grid size-3.5 shrink-0 place-items-center rounded-full border ${on ? "border-neon" : "border-line-2"}`}>{on && <span className="size-1.5 rounded-full bg-neon" />}</span>
                        <span className="min-w-0 flex-1">
                          <span className="block truncate text-[13px] text-ink">{w.app}</span>
                          <span className="block truncate text-xs text-mute">{w.title}</span>
                        </span>
                        {w.minimized && <span className="shrink-0 text-[11px] text-mute">minimizada · se abre al grabar</span>}
                      </button>
                    </li>
                  );
                })}
              </ul>
            )}
            <button type="button" onClick={loadWindows} className="inline-flex items-center gap-1.5 text-xs text-neon hover:underline">
              <RefreshCw size={11} /> Actualizar la lista
            </button>
            <p className="text-xs text-mute">Solo sale esa ventana, aunque la muevas o tenga otra encima. El título no se guarda.</p>
          </div>
        </Row>
      )}

      {opts.target === "region" && (
        <Row label="Zona">
          <div className="flex flex-wrap items-center gap-2">
            {monitorSelect}
            {choice.region ? <span className="text-ink">{regionLabel(choice.region.region)}</span> : <span className="text-dim">Sin dibujar</span>}
            <Button kind="ghost" size="sm" onClick={drawRegion} disabled={picking}>
              <SquareDashed size={13} /> {picking ? "Dibujando en la pantalla…" : choice.region ? "Dibujarla otra vez" : "Dibujar la zona"}
            </Button>
          </div>
        </Row>
      )}

      <Row label="Clics y teclas">
        <span className="flex items-center gap-3">
          <Toggle checked={opts.input} onChange={(v) => setOpts({ input: v })} label="Apuntar los clics y las teclas" />
          <span className="text-dim">
            <MousePointerClick size={12} className="mr-1 inline" />
            Un círculo en cada clic y los atajos («Ctrl + P», «Intro»). Lo que se escribe nunca se guarda: sale «•••».
          </span>
        </span>
      </Row>
      <Row label="Voz">
        <span className="flex items-center gap-3">
          <Toggle checked={opts.mic} onChange={(v) => setOpts({ mic: v })} label="Grabar el micrófono" />
          <span className="text-dim">
            <Mic size={12} className="mr-1 inline" />
            {opts.mic ? "El usuario cuenta lo que hace mientras graba. Se apaga solo al parar." : "Apagado: el vídeo va sin sonido."}
          </span>
        </span>
      </Row>
    </>
  );
}

/** Estado del equipo → Grabar el fallo: grabar la pantalla con lo que pasa en Windows apuntado en una línea de tiempo. */
export function FaultRecorder() {
  const [status, setStatus] = useState<FaultRecStatus | null>(null);
  const [monitors, setMonitors] = useState<FaultRecMonitor[]>([]);
  const [list, setList] = useState<FaultRecording[]>([]);
  const [hasCase, setHasCase] = useState(false);
  const [choice, setChoice] = useState<Choice>(() => ({ opts: getRecOptions(), monitorId: "", window: null, region: null }));
  const [selected, setSelected] = useState<string | null>(null);
  const [detail, setDetail] = useState<FaultRecordingDetail | null>(null);
  // Se pidió grabar otra vez: se enseña la pantalla de preparar aunque haya grabaciones.
  const [preparing, setPreparing] = useState(false);
  // La pantalla abre con la lista (idea 16); al terminar una grabación o abrir una, se ve esa.
  const [browsing, setBrowsing] = useState(true);
  // El último error ya enseñado: llega por el aviso de «terminó» y también por la consulta de respaldo.
  const shownError = useRef<string | null>(null);
  const [error, setError] = useState<string | null>(null);
  const { confirm, dialog } = useConfirm();
  const toast = useToast();
  const showError = useCallback(
    (msg: string) => {
      if (msg === shownError.current) return;
      shownError.current = msg;
      toast("error", msg);
    },
    [toast],
  );
  const active = usePageActive();

  const loadList = useCallback(async (prefer?: string | null) => {
    const l = await faultRecApi.list();
    setList(l);
    setSelected((cur) => pickRecording(l, prefer ?? cur)?.id ?? null);
    return l;
  }, []);

  const loadCase = useCallback(() => {
    void casesApi
      .current()
      .then((c) => setHasCase(!!c))
      .catch(logQuietly("FaultRecorder.case"));
  }, []);

  const load = useCallback(async () => {
    try {
      const [s, m] = await Promise.all([faultRecApi.status(), faultRecApi.monitors()]);
      setStatus(s);
      setMonitors(m);
      if (s.error) showError(s.error);
      await loadList();
      setError(null);
    } catch (e) {
      setError(String(e));
    }
  }, [loadList, showError]);

  useEffect(() => {
    void load();
    loadCase();
  }, [load, loadCase]);

  // Al volver a la pestaña (o a AdminOps después de grabar) todo puede haber cambiado.
  useEffect(() => {
    if (active) {
      void load();
      loadCase();
    }
  }, [active, load, loadCase]);

  useEffect(() => {
    const onCase = () => {
      loadCase();
      // Cerrar el caso borra sus grabaciones.
      void loadList().catch(logQuietly("FaultRecorder.list"));
    };
    window.addEventListener(CASE_CHANGED_EVENT, onCase);
    const offs = [
      listen("case-changed", onCase),
      listen<number>(FAULTREC_TICK_EVENT, (e) => setStatus((s) => (s ? { ...s, recording: true, secs: e.payload } : s))),
      listen<FaultAutoSaved>(FAULTREC_AUTO_EVENT, (e) => {
        toast("info", `${e.payload.trigger} a las ${timeOfDay(e.payload.at)}: se guardó sola. Está en la lista de grabaciones.`);
        void loadList().catch(logQuietly("FaultRecorder.list"));
      }),
      listen<FaultRecDone>(FAULTREC_DONE_EVENT, (e) => {
        setStatus((s) => (s ? { ...s, recording: false, finishing: false } : s));
        if (e.payload.error) {
          showError(e.payload.error);
          // El mismo error queda guardado para la consulta de respaldo: se recoge ya para que no vuelva a salir.
          void faultRecApi.status().catch(logQuietly("FaultRecorder.status"));
        }
        // Lo guardado con Ctrl+Alt+G solo entra en la lista: lo que se estuviera haciendo aquí no cambia.
        if (e.payload.id && !e.payload.replay) {
          setPreparing(false);
          setBrowsing(false);
        }
        void loadList(e.payload.replay ? null : e.payload.id).catch(logQuietly("FaultRecorder.list"));
      }),
    ];
    return () => {
      window.removeEventListener(CASE_CHANGED_EVENT, onCase);
      for (const off of offs) void off.then((f) => f());
    };
  }, [loadCase, loadList, showError, toast]);

  // Mientras se graba se pregunta cada segundo: si el aviso de «terminó» se pierde (o es la demo, que no tiene avisos),
  // la pantalla igualmente pasa a la grabación.
  const busy = status?.recording === true || status?.finishing === true;
  useEffect(() => {
    if (!busy) return;
    const t = window.setInterval(() => {
      faultRecApi.status().then(
        (s) => {
          setStatus(s);
          if (s.error) showError(s.error);
          if (!s.recording && !s.finishing) {
            setPreparing(false);
            setBrowsing(false);
            void loadList(s.id).catch(logQuietly("FaultRecorder.list"));
          }
        },
        logQuietly("FaultRecorder.status"),
      );
    }, 1000);
    return () => window.clearInterval(t);
  }, [busy, loadList, showError]);

  // La grabación elegida, con la ruta de su vídeo.
  useEffect(() => {
    if (!selected) {
      setDetail(null);
      return;
    }
    let alive = true;
    faultRecApi.get(selected).then(
      (d) => alive && setDetail(d),
      (e) => {
        if (!alive) return;
        setDetail(null);
        toast("error", String(e));
        void loadList().catch(logQuietly("FaultRecorder.list"));
      },
    );
    return () => {
      alive = false;
    };
  }, [selected, toast, loadList]);

  const start = async () => {
    try {
      shownError.current = null;
      const { opts } = choice;
      const monitor = opts.target === "region" ? (choice.region?.monitor ?? choice.monitorId) : choice.monitorId;
      const s = await faultRecApi.start(monitor || null, {
        window: opts.target === "window" ? choice.window : null,
        region: opts.target === "region" ? (choice.region?.region ?? null) : null,
        input: opts.input,
        mic: opts.mic,
      });
      setStatus(s);
      setPreparing(false);
    } catch (e) {
      const msg = String(e);
      toast(msg.includes("antes de empezar a grabar") ? "info" : "error", msg);
    }
  };

  const stop = async () => {
    try {
      const rec = await faultRecApi.stop();
      setStatus((s) => (s ? { ...s, recording: false, finishing: false } : s));
      setPreparing(false);
      await loadList(rec.id);
    } catch (e) {
      showError(String(e));
    }
  };

  const remove = async () => {
    if (!detail) return;
    const ok = await confirm({ title: "¿Borrar la grabación?", body: "Se borran el vídeo y sus marcas de este equipo. No se puede deshacer.", confirmLabel: "Borrar grabación", danger: true });
    if (!ok) return;
    try {
      await faultRecApi.remove(detail.meta.id);
      setDetail(null);
      setSelected(null);
      const l = await loadList();
      setBrowsing(true);
      if (l.length === 0) setPreparing(false);
    } catch (e) {
      toast("error", String(e));
    }
  };

  if (error && !status) return <ErrorState message={error} onRetry={() => void load()} where="Estado del equipo · Grabar el fallo" />;
  if (!status) return <Loading text="Preparando la grabación de pantalla…" page />;

  // Grabando (la ventana se minimiza al empezar; esto se ve si se vuelve a AdminOps antes de que acabe).
  if (status.recording) {
    return (
      <div className="mx-auto max-w-(--page-max) space-y-4 p-6">
        <Card title="Grabando" icon={<Circle size={14} className="text-bad" fill="currentColor" />}>
          <p className="text-2xl font-semibold tabular text-ink">{clock(status.secs)}</p>
          <p className="mt-1 text-[13px] text-dim">Para sola a los {clock(MAX_SECS)}. Quedan {lengthLabel(remaining(status.secs, status.maxSecs || MAX_SECS))}.</p>
          <div className="mt-4">
            <Button kt="P" kind="danger" onClick={stop}>
              Parar
            </Button>
          </div>
        </Card>
        {dialog}
      </div>
    );
  }

  if (status.finishing) {
    return (
      <div className="mx-auto max-w-(--page-max) space-y-4 p-6">
        <Loading text="Preparando las marcas de la grabación…" />
      </div>
    );
  }

  const viewing = !!selected && !preparing && !browsing;
  const home = !preparing && !viewing && list.length > 0;
  return (
    <div className="mx-auto max-w-(--page-max) space-y-4 p-6">
      {viewing ? (
        detail ? (
          <Result key={detail.meta.id} detail={detail} onAgain={() => setPreparing(true)} onDelete={remove} onBack={() => setBrowsing(true)} />
        ) : (
          <Loading text="Abriendo la grabación…" />
        )
      ) : home ? (
        <Card title="Grabar el fallo" icon={<Video size={14} />}>
          <p className="-mt-2 mb-3 text-xs text-mute">Hasta 2 minutos · se queda en este equipo</p>
          <div className="flex flex-wrap items-center gap-2">
            <Button kt="G" kind="danger" onClick={() => setPreparing(true)}>
              <Circle size={10} fill="currentColor" /> Grabar
            </Button>
            <Button kt="V" kind="secondary" onClick={() => document.getElementById("fault-watch")?.scrollIntoView({ behavior: "smooth", block: "start" })}>
              <Eye size={13} /> Vigilar
            </Button>
          </div>
          <RecList
            list={list}
            hasCase={hasCase}
            onOpen={(id) => {
              setSelected(id);
              setBrowsing(false);
            }}
          />
        </Card>
      ) : (
        <Card title="Grabar el fallo" icon={<Video size={14} />}>
          <p className="-mt-2 mb-3 text-xs text-mute">Hasta 2 minutos · se queda en este equipo</p>
          <Intro id="machine:recorder">Pide al usuario que haga lo de siempre hasta que falle. AdminOps graba la pantalla y apunta lo que pasa en Windows a la vez.</Intro>
          <dl className="mt-3">
            <Setup monitors={monitors} choice={choice} onChange={setChoice} />
            <Row label="Se borra">{eraseWhen(hasCase)}</Row>
          </dl>
          <div className="mt-4 flex flex-wrap items-center gap-2">
            <Button kt="E" kind="danger" onClick={start} disabled={!ready(choice)}>
              <Circle size={10} fill="currentColor" /> Empezar a grabar
            </Button>
            {list.length > 0 && (
              <Button
                kind="ghost"
                onClick={() => {
                  setPreparing(false);
                  setBrowsing(true);
                }}
              >
                Ver las grabaciones
              </Button>
            )}
          </div>
        </Card>
      )}

      <FaultWatch
        monitorId={choice.monitorId}
        input={choice.opts.input}
        onSaved={(id) => {
          setPreparing(false);
          setBrowsing(false);
          void loadList(id).catch(logQuietly("FaultRecorder.list"));
        }}
      />
      {dialog}
    </div>
  );
}
