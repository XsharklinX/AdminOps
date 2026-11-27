import { listen } from "@tauri-apps/api/event";
import { Headphones, Mic, RefreshCw, ShieldCheck, Volume2 } from "lucide-react";
import { useCallback, useEffect, useRef, useState } from "react";
import { audioApi, AUDIO_LEVELS_EVENT, type AudioDev, type AudioView } from "../lib/api/audio";
import { advise, gadget, HEARD, privacyProblems } from "../lib/audio";
import { useToast } from "./feedback";
import { Button, Card } from "./ui";

const LISTEN_SECS = 12;

/**
 * «No se oye o no me oyen»: cada micrófono con su barra de voz en vivo, cuál te oye, y con un clic el
 * bueno para todo (predeterminado, nivel y permisos). Igual con altavoces y cascos.
 */
export function AudioCard() {
  const [view, setView] = useState<AudioView | null>(null);
  const [error, setError] = useState("");
  const [live, setLive] = useState<Record<string, number | null>>({});
  const [peaks, setPeaks] = useState<Record<string, number | null>>({});
  const [listening, setListening] = useState(false);
  const [busy, setBusy] = useState<string | null>(null);
  const [done, setDone] = useState<string[]>([]);
  const toast = useToast();
  const peaksRef = useRef<Record<string, number | null>>({});

  const load = useCallback(() => {
    audioApi
      .view()
      .then((v) => {
        setView(v);
        setError("");
      })
      .catch((e) => setError(String(e)));
  }, []);

  const start = useCallback(() => {
    peaksRef.current = {};
    setPeaks({});
    setLive({});
    setListening(true);
    audioApi.listen(LISTEN_SECS).catch((e) => {
      setListening(false);
      toast("error", String(e));
    });
  }, [toast]);

  useEffect(() => {
    load();
    const off = listen<[string, number | null][] | null>(AUDIO_LEVELS_EVENT, (e) => {
      if (!e.payload) {
        setListening(false);
        return;
      }
      const now: Record<string, number | null> = {};
      for (const [id, v] of e.payload) {
        now[id] = v;
        const prev = peaksRef.current[id];
        peaksRef.current[id] = v === null ? (prev ?? null) : Math.max(prev ?? 0, v);
      }
      setLive(now);
      setPeaks({ ...peaksRef.current });
    });
    start();
    return () => {
      void off.then((f) => f());
    };
  }, [load, start]);

  if (error) return <Card title="No se oye o no me oyen" icon={<Mic size={14} />} className="col-span-12"><p className="text-sm text-bad">{error}</p></Card>;
  if (!view) return null;

  const advice = advise(view, peaks);
  const perms = privacyProblems(view.privacy);
  const heardAny = Object.values(peaks).some((p) => (p ?? 0) >= HEARD);
  const fix = async () => {
    if (!advice.mic) return;
    setBusy("fix");
    try {
      const lines = await audioApi.use(advice.mic.id, true, 80);
      if (advice.speaker) lines.push(...(await audioApi.use(advice.speaker.id, false, 50)));
      setDone(lines.length ? lines : ["Ya estaba todo bien puesto"]);
      load();
    } catch (e) {
      toast("error", String(e));
    } finally {
      setBusy(null);
    }
  };
  const pickDevice = async (d: AudioDev, capture: boolean) => {
    setBusy(d.id);
    try {
      const lines = await audioApi.use(d.id, capture, capture ? 80 : 50);
      setDone(lines);
      load();
    } catch (e) {
      toast("error", String(e));
    } finally {
      setBusy(null);
    }
  };
  const allow = async () => {
    setBusy("perm");
    try {
      setDone(await audioApi.allowMic());
      load();
    } catch (e) {
      toast("error", String(e));
    } finally {
      setBusy(null);
    }
  };

  const problems = [advice.problem, ...perms].filter(Boolean);
  const target = advice.mic;
  const fixLines = target
    ? [
        advice.speaker ? "Micrófono y altavoz predeterminados, también para llamadas" : "Micrófono predeterminado, también para llamadas",
        target.volume < 80 || target.muted ? "Nivel del micrófono al 80 %" : null,
        perms.length ? "Permiso de micrófono para Teams y las aplicaciones de escritorio" : null,
      ].filter(Boolean)
    : [];

  return (
    <Card
      title="No se oye o no me oyen"
      icon={<Mic size={14} />}
      className="col-span-12"
      right={
        <button type="button" onClick={start} disabled={listening} className="flex items-center gap-1 text-[11px] text-mute hover:text-ink disabled:opacity-40">
          <RefreshCw size={11} className={listening ? "animate-spin" : ""} /> {listening ? "Escuchando… habla ahora" : "Escuchar otra vez"}
        </button>
      }
    >
      {listening && !heardAny && <p className="mb-2 text-sm text-neon">Habla ahora (o que hable el usuario): AdminOps escucha todos los micrófonos a la vez.</p>}
      {problems.length > 0 && (
        <div className="mb-3 rounded-lg border border-warn/40 bg-warn/5 px-3 py-2 text-sm text-ink">
          {problems.map((p) => (
            <p key={p}>{p}</p>
          ))}
        </div>
      )}
      <div className="mb-1 text-[11px] font-medium uppercase tracking-wide text-mute">Micrófonos</div>
      <div className="space-y-1.5">
        {view.capture.map((d) => (
          <DevRow key={d.id} d={d} level={live[d.id]} peak={peaks[d.id]} capture busy={busy === d.id} onUse={() => void pickDevice(d, true)} />
        ))}
      </div>
      {target && fixLines.length > 0 && (advice.problem || perms.length > 0) && (
        <div className="mt-3 rounded-lg border border-neon/40 bg-neon/5 p-3">
          <div className="flex items-center gap-2 text-sm font-medium text-ink">
            <Headphones size={15} className="text-neon" /> Usar «{gadget(target.name)}» para todo
          </div>
          <ul className="mt-1 list-inside list-disc text-[12.5px] text-dim">
            {fixLines.map((l) => (
              <li key={l as string}>{l}</li>
            ))}
          </ul>
          <div className="mt-2">
            <Button size="sm" disabled={busy !== null} onClick={fix}>
              {busy === "fix" ? "Arreglando…" : "Arreglarlo"}
            </Button>
          </div>
        </div>
      )}
      {!target && perms.length > 0 && (
        <div className="mt-3">
          <Button size="sm" disabled={busy !== null} onClick={allow}>
            <ShieldCheck size={13} /> Dar permiso de micrófono
          </Button>
        </div>
      )}
      {done.length > 0 && (
        <ul className="mt-3 space-y-0.5 text-[12.5px] text-ok">
          {done.map((l) => (
            <li key={l}>✓ {l}</li>
          ))}
        </ul>
      )}
      <EchoTest view={view} peaks={peaks} listening={listening} />
      <div className="mb-1 mt-4 text-[11px] font-medium uppercase tracking-wide text-mute">Altavoces y cascos</div>
      <div className="space-y-1.5">
        {view.render.map((d) => (
          <DevRow key={d.id} d={d} capture={false} busy={busy === d.id} onUse={() => void pickDevice(d, false)} onBeep={() => void audioApi.beep(d.id).catch((e) => toast("error", String(e)))} />
        ))}
      </div>
    </Card>
  );
}

function DevRow({ d, level, peak, capture, busy, onUse, onBeep }: { d: AudioDev; level?: number | null; peak?: number | null; capture: boolean; busy: boolean; onUse: () => void; onBeep?: () => void }) {
  const heard = (peak ?? 0) >= HEARD;
  const closed = capture && level === null;
  // La voz tiene picos bajos: se amplía para que la barra se mueva de verdad al hablar.
  const width = Math.min(100, Math.round(Math.sqrt(level ?? 0) * 140));
  return (
    <div className={`flex items-center gap-3 rounded-lg border px-3 py-2 ${heard ? "border-ok/50 bg-ok/5" : "border-line bg-panel-2/40"}`}>
      {capture ? <Mic size={14} className={heard ? "text-ok" : "text-mute"} /> : <Volume2 size={14} className="text-mute" />}
      <div className="min-w-0 flex-1">
        <div className="flex items-center gap-2 text-sm text-ink">
          <span className="truncate">{gadget(d.name)}</span>
          {d.default && <span className="shrink-0 text-[11px] text-mute">predeterminado</span>}
          <span className={`shrink-0 text-[11px] ${d.muted || d.volume < 50 ? "text-warn" : "text-mute"}`}>{d.muted ? "en silencio" : `${d.volume} %`}</span>
          {heard && <span className="shrink-0 text-[11px] font-medium text-ok">te oye</span>}
        </div>
        {capture && (
          <div className="mt-1 h-1.5 overflow-hidden rounded-full bg-void/70">
            <div className="h-full rounded-full bg-ok transition-[width] duration-100" style={{ width: `${width}%` }} />
          </div>
        )}
        {closed && <div className="mt-0.5 text-[11px] text-mute">No se pudo escuchar (permiso o en uso exclusivo por otro programa)</div>}
      </div>
      {onBeep && (
        <button type="button" onClick={onBeep} className="shrink-0 rounded-md px-2 py-1 text-[12px] text-dim hover:bg-panel-2 hover:text-ink">
          Probar
        </button>
      )}
      {!(d.default && d.defaultComm && !d.muted && d.volume >= 50) && (
        <Button size="sm" kind="ghost" disabled={busy} onClick={onUse}>
          {busy ? "…" : "Usar este"}
        </Button>
      )}
    </div>
  );
}

const ECHO_SECS = 5;

/**
 * «Escúchate como en una llamada»: graba unos segundos del micrófono elegido y los reproduce por los cascos
 * o altavoces elegidos. Así se oye el eco, la saturación o la distancia, que la barra de voz no dice.
 */
function EchoTest({ view, peaks, listening }: { view: AudioView; peaks: Record<string, number | null>; listening: boolean }) {
  // Por defecto, el micrófono que más te oyó (o el predeterminado) y su pareja de cascos si la hay.
  const best = [...view.capture].sort((a, b) => (peaks[b.id] ?? 0) - (peaks[a.id] ?? 0))[0];
  const defMic = (best && (peaks[best.id] ?? 0) >= HEARD ? best : view.capture.find((d) => d.default)) ?? view.capture[0];
  const pairSpk = defMic ? view.render.find((r) => gadget(r.name).toLowerCase() === gadget(defMic.name).toLowerCase()) : undefined;
  const [mic, setMic] = useState<string>("");
  const [spk, setSpk] = useState<string>("");
  const [phase, setPhase] = useState<"idle" | "rec" | "done">("idle");
  const [left, setLeft] = useState(0);
  const [result, setResult] = useState<{ ok: boolean; text: string } | null>(null);
  const micId = mic || defMic?.id || "";
  const spkId = spk || pairSpk?.id || view.render.find((d) => d.default)?.id || view.render[0]?.id || "";
  if (!micId || !spkId) return null;
  const nameOf = (list: AudioDev[], id: string) => gadget(list.find((d) => d.id === id)?.name ?? "");
  const run = async () => {
    setResult(null);
    setPhase("rec");
    setLeft(ECHO_SECS);
    const tick = window.setInterval(() => setLeft((s) => Math.max(0, s - 1)), 1000);
    try {
      const peak = await audioApi.echo(micId, spkId, ECHO_SECS);
      setResult(peak >= HEARD ? { ok: true, text: `Se te oyó (pico ${Math.round(peak * 100)} %). ¿Se entiende bien, sin eco ni ruido?` } : { ok: false, text: "No se oyó nada: ese micrófono no capta (apagado, tapado o con el nivel muy bajo)." });
    } catch (e) {
      setResult({ ok: false, text: String(e) });
    } finally {
      window.clearInterval(tick);
      setPhase("done");
    }
  };
  const sel = "max-w-[14rem] rounded border border-line bg-void/60 px-1.5 py-1 text-xs text-ink outline-none";
  return (
    <div className="mt-3 rounded-lg border border-line bg-void/40 p-3">
      <div className="mb-1.5 text-[11px] font-medium uppercase tracking-wide text-mute">Escúchate como en una llamada</div>
      <div className="flex flex-wrap items-center gap-2 text-sm text-dim">
        Grabar con
        <select aria-label="Micrófono para la prueba" value={micId} onChange={(e) => setMic(e.target.value)} className={sel} disabled={phase === "rec"}>
          {view.capture.map((d) => (
            <option key={d.id} value={d.id}>
              {gadget(d.name)}
            </option>
          ))}
        </select>
        y oírlo por
        <select aria-label="Altavoz o cascos para la prueba" value={spkId} onChange={(e) => setSpk(e.target.value)} className={sel} disabled={phase === "rec"}>
          {view.render.map((d) => (
            <option key={d.id} value={d.id}>
              {gadget(d.name)}
            </option>
          ))}
        </select>
        <Button size="sm" disabled={phase === "rec" || listening} onClick={run}>
          {phase === "rec" ? (left > 0 ? `● Habla ahora… ${left} s` : `▶ Escuchándote por ${nameOf(view.render, spkId)}`) : `Grabar ${ECHO_SECS} s y escucharte`}
        </Button>
      </div>
      {result && <p className={`mt-1.5 text-[12.5px] ${result.ok ? "text-ok" : "text-warn"}`}>{result.text}</p>}
      {listening && <p className="mt-1.5 text-[11px] text-mute">Cuando termine de escuchar los micrófonos.</p>}
    </div>
  );
}
