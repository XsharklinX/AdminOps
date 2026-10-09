// Probar periféricos: pantalla (píxeles muertos), teclado, altavoces izquierdo y
// derecho, micrófono y cámara. Para «no funciona el teclado», «no me oyen» o
// «tengo un punto en la pantalla». Nada se graba ni se guarda: el micrófono y la
// cámara se ven en vivo y se apagan al salir de la pestaña.
import { Camera, Keyboard, Mic, Monitor, RotateCcw, Volume2 } from "lucide-react";
import { useEffect, useRef, useState } from "react";
import { Button, Card } from "../components/ui";
import { usePageActive } from "../lib/pageActive";

// ---------- Pantalla ----------

const COLORS = [
  { c: "#ff0000", n: "Rojo" },
  { c: "#00ff00", n: "Verde" },
  { c: "#0000ff", n: "Azul" },
  { c: "#ffffff", n: "Blanco" },
  { c: "#000000", n: "Negro" },
];

function ScreenTest() {
  const [i, setI] = useState<number | null>(null);
  const box = useRef<HTMLDivElement>(null);
  useEffect(() => {
    if (i === null) return;
    const onKey = (e: KeyboardEvent) => {
      e.preventDefault();
      if (e.key === "Escape") setI(null);
      else setI((x) => (x === null ? null : (x + 1) % COLORS.length));
    };
    window.addEventListener("keydown", onKey);
    box.current?.requestFullscreen?.().catch(() => {});
    return () => {
      window.removeEventListener("keydown", onKey);
      if (document.fullscreenElement) void document.exitFullscreen().catch(() => {});
    };
  }, [i]);
  return (
    <Card title="Pantalla" icon={<Monitor size={14} />}>
      <p className="text-sm text-dim">La pantalla entera de un color, para ver píxeles muertos (un punto que no cambia), manchas o zonas con otro tono.</p>
      <div className="mt-3 flex flex-wrap items-center gap-2">
        <Button onClick={() => setI(0)}>Empezar</Button>
        <span className="text-xs text-mute">Clic o cualquier tecla: siguiente color · Esc: salir</span>
      </div>
      {i !== null && (
        <div
          ref={box}
          onClick={() => setI((x) => (x === null ? null : (x + 1) % COLORS.length))}
          className="fixed inset-0 z-[90] cursor-pointer"
          style={{ background: COLORS[i].c }}
          role="dialog"
          aria-label={`Prueba de pantalla: ${COLORS[i].n}`}
        >
          <span className="absolute bottom-4 left-1/2 -translate-x-1/2 rounded bg-black/50 px-3 py-1 text-xs text-white">
            {COLORS[i].n} · clic o tecla: siguiente · Esc: salir
          </span>
        </div>
      )}
    </Card>
  );
}

// ---------- Teclado ----------

/** Filas del teclado español (por la posición física de cada tecla). */
const ROWS: [string, string][][] = [
  [["Escape", "Esc"], ["F1", "F1"], ["F2", "F2"], ["F3", "F3"], ["F4", "F4"], ["F5", "F5"], ["F6", "F6"], ["F7", "F7"], ["F8", "F8"], ["F9", "F9"], ["F10", "F10"], ["F11", "F11"], ["F12", "F12"]],
  [["Backquote", "º"], ["Digit1", "1"], ["Digit2", "2"], ["Digit3", "3"], ["Digit4", "4"], ["Digit5", "5"], ["Digit6", "6"], ["Digit7", "7"], ["Digit8", "8"], ["Digit9", "9"], ["Digit0", "0"], ["Minus", "'"], ["Equal", "¡"], ["Backspace", "⌫"]],
  [["Tab", "Tab"], ["KeyQ", "Q"], ["KeyW", "W"], ["KeyE", "E"], ["KeyR", "R"], ["KeyT", "T"], ["KeyY", "Y"], ["KeyU", "U"], ["KeyI", "I"], ["KeyO", "O"], ["KeyP", "P"], ["BracketLeft", "`"], ["BracketRight", "+"], ["Enter", "Intro"]],
  [["CapsLock", "Bloq"], ["KeyA", "A"], ["KeyS", "S"], ["KeyD", "D"], ["KeyF", "F"], ["KeyG", "G"], ["KeyH", "H"], ["KeyJ", "J"], ["KeyK", "K"], ["KeyL", "L"], ["Semicolon", "Ñ"], ["Quote", "´"], ["Backslash", "Ç"]],
  [["ShiftLeft", "Mayús"], ["IntlBackslash", "<"], ["KeyZ", "Z"], ["KeyX", "X"], ["KeyC", "C"], ["KeyV", "V"], ["KeyB", "B"], ["KeyN", "N"], ["KeyM", "M"], ["Comma", ","], ["Period", "."], ["Slash", "-"], ["ShiftRight", "Mayús"]],
  [["ControlLeft", "Ctrl"], ["MetaLeft", "Win"], ["AltLeft", "Alt"], ["Space", "Espacio"], ["AltRight", "Alt Gr"], ["ContextMenu", "Menú"], ["ControlRight", "Ctrl"], ["ArrowLeft", "←"], ["ArrowUp", "↑"], ["ArrowDown", "↓"], ["ArrowRight", "→"]],
];

function KeyboardTest({ active }: { active: boolean }) {
  const [on, setOn] = useState(false);
  const [seen, setSeen] = useState<Set<string>>(new Set());
  const [down, setDown] = useState<string | null>(null);
  const [last, setLast] = useState("");
  useEffect(() => {
    if (!on || !active) return;
    const keydown = (e: KeyboardEvent) => {
      // Mientras se prueba, las teclas no hacen nada más en AdminOps (Esc sale).
      if (e.code === "Escape" && seen.has("Escape")) {
        setOn(false);
        return;
      }
      e.preventDefault();
      e.stopPropagation();
      setSeen((s) => new Set(s).add(e.code));
      setDown(e.code);
      setLast(`${e.code}${e.key && e.key.length === 1 ? ` («${e.key}»)` : ""}`);
    };
    const keyup = () => setDown(null);
    window.addEventListener("keydown", keydown, true);
    window.addEventListener("keyup", keyup, true);
    return () => {
      window.removeEventListener("keydown", keydown, true);
      window.removeEventListener("keyup", keyup, true);
    };
  }, [on, active, seen]);
  const total = ROWS.flat().length;
  const done = ROWS.flat().filter(([c]) => seen.has(c)).length;
  return (
    <Card title="Teclado" icon={<Keyboard size={14} />}>
      <p className="text-sm text-dim">Pulsa cada tecla: se marca en verde la que responde. La que no se marque no envía nada (o envía otra cosa: abajo se ve qué llegó).</p>
      <div className="mt-3 flex flex-wrap items-center gap-2">
        {on ? (
          <>
            <span className="text-xs text-ok">Probando: {done} de {total} teclas. Esc dos veces para terminar.</span>
            <Button kind="ghost" size="sm" onClick={() => setOn(false)}>
              Terminar
            </Button>
          </>
        ) : (
          <Button onClick={() => setOn(true)}>Empezar</Button>
        )}
        {seen.size > 0 && (
          <Button kind="ghost" size="sm" onClick={() => setSeen(new Set())}>
            <RotateCcw size={13} /> Empezar de cero
          </Button>
        )}
      </div>
      <div className={`mt-3 space-y-1 overflow-x-auto ${on ? "" : "opacity-60"}`}>
        {ROWS.map((row, r) => (
          <div key={r} className="flex gap-1">
            {row.map(([code, label]) => (
              <span
                key={code}
                className={`grid h-8 shrink-0 place-items-center rounded border px-1.5 font-mono text-[11px] ${code === "Space" ? "min-w-48" : "min-w-8"} ${
                  down === code ? "border-neon bg-neon/30 text-ink" : seen.has(code) ? "border-ok/50 bg-ok/15 text-ok" : "border-line-2 text-mute"
                }`}
              >
                {label}
              </span>
            ))}
          </div>
        ))}
      </div>
      {last && <p className="mt-2 font-mono text-[11.5px] text-mute">Última: {last}</p>}
    </Card>
  );
}

// ---------- Altavoces ----------

function SpeakerTest() {
  const ctx = useRef<AudioContext | null>(null);
  const [playing, setPlaying] = useState<string | null>(null);
  const play = (pan: number, name: string) => {
    ctx.current ??= new AudioContext();
    const a = ctx.current;
    const osc = a.createOscillator();
    const gain = a.createGain();
    const panner = a.createStereoPanner();
    osc.frequency.value = 440;
    panner.pan.value = pan;
    // Entra y sale suave: sin chasquido.
    gain.gain.setValueAtTime(0, a.currentTime);
    gain.gain.linearRampToValueAtTime(0.25, a.currentTime + 0.05);
    gain.gain.linearRampToValueAtTime(0, a.currentTime + 1.2);
    osc.connect(gain).connect(panner).connect(a.destination);
    osc.start();
    osc.stop(a.currentTime + 1.25);
    setPlaying(name);
    osc.onended = () => setPlaying(null);
  };
  useEffect(() => () => void ctx.current?.close().catch(() => {}), []);
  return (
    <Card title="Altavoces" icon={<Volume2 size={14} />}>
      <p className="text-sm text-dim">Un pitido por cada lado. Si suena por el lado contrario o por los dos, hay un cable o un ajuste de balance mal (Configuración de sonido).</p>
      <div className="mt-3 flex flex-wrap gap-2">
        {[
          [-1, "Izquierdo"],
          [0, "Los dos"],
          [1, "Derecho"],
        ].map(([pan, name]) => (
          <Button key={name} kind={playing === name ? "primary" : "secondary"} onClick={() => play(pan as number, name as string)}>
            {name}
          </Button>
        ))}
      </div>
    </Card>
  );
}

// ---------- Micrófono y cámara ----------

const deniedText = (e: unknown) =>
  String(e).includes("NotAllowed") || String(e).includes("Permission")
    ? "Windows o AdminOps no han dejado usarlo. Mira Configuración → Privacidad y seguridad → Micrófono/Cámara: tiene que estar permitido para las aplicaciones de escritorio."
    : String(e).includes("NotFound")
      ? "No hay ninguno conectado (o Windows no lo ve)."
      : String(e);

function MicTest({ active }: { active: boolean }) {
  const [level, setLevel] = useState(0);
  const [peak, setPeak] = useState(0);
  const [on, setOn] = useState(false);
  const [err, setErr] = useState<string | null>(null);
  useEffect(() => {
    if (!on || !active) return;
    let stream: MediaStream | null = null;
    let ctx: AudioContext | null = null;
    let raf = 0;
    let stop = false;
    navigator.mediaDevices
      .getUserMedia({ audio: true })
      .then((s) => {
        if (stop) return s.getTracks().forEach((t) => t.stop());
        stream = s;
        ctx = new AudioContext();
        const an = ctx.createAnalyser();
        an.fftSize = 512;
        ctx.createMediaStreamSource(s).connect(an);
        const buf = new Uint8Array(an.fftSize);
        const tick = () => {
          an.getByteTimeDomainData(buf);
          let max = 0;
          for (const v of buf) max = Math.max(max, Math.abs(v - 128));
          const l = Math.min(1, max / 100);
          setLevel(l);
          setPeak((p) => Math.max(p * 0.995, l));
          raf = requestAnimationFrame(tick);
        };
        tick();
        setErr(null);
      })
      .catch((e) => {
        setErr(deniedText(e));
        setOn(false);
      });
    return () => {
      stop = true;
      cancelAnimationFrame(raf);
      stream?.getTracks().forEach((t) => t.stop());
      void ctx?.close().catch(() => {});
      setLevel(0);
    };
  }, [on, active]);
  return (
    <Card title="Micrófono" icon={<Mic size={14} />}>
      <p className="text-sm text-dim">Habla o da una palmada: la barra se mueve si el micrófono llega. No se graba nada.</p>
      <div className="mt-3 flex items-center gap-3">
        <Button kind={on ? "ghost" : "primary"} onClick={() => setOn((x) => !x)}>
          {on ? "Parar" : "Empezar"}
        </Button>
        <div className="relative h-3 flex-1 overflow-hidden rounded-full bg-panel-2" aria-label="Nivel del micrófono">
          <div className="h-full rounded-full bg-ok transition-[width] duration-75" style={{ width: `${level * 100}%` }} />
          <div className="absolute top-0 h-full w-0.5 bg-ink/60" style={{ left: `${peak * 100}%` }} />
        </div>
      </div>
      {err && <p className="mt-2 text-xs text-warn">{err}</p>}
    </Card>
  );
}

function CameraTest({ active }: { active: boolean }) {
  const video = useRef<HTMLVideoElement>(null);
  const [on, setOn] = useState(false);
  const [err, setErr] = useState<string | null>(null);
  useEffect(() => {
    if (!on || !active) return;
    let stream: MediaStream | null = null;
    let stop = false;
    const el = video.current;
    navigator.mediaDevices
      .getUserMedia({ video: true })
      .then((s) => {
        if (stop) return s.getTracks().forEach((t) => t.stop());
        stream = s;
        if (el) el.srcObject = s;
        setErr(null);
      })
      .catch((e) => {
        setErr(deniedText(e));
        setOn(false);
      });
    return () => {
      stop = true;
      stream?.getTracks().forEach((t) => t.stop());
      if (el) el.srcObject = null;
    };
  }, [on, active]);
  return (
    <Card title="Cámara" icon={<Camera size={14} />}>
      <p className="text-sm text-dim">La imagen de la cámara en vivo. No se graba ni se guarda nada.</p>
      <div className="mt-3">
        <Button kind={on ? "ghost" : "primary"} onClick={() => setOn((x) => !x)}>
          {on ? "Apagar" : "Encender"}
        </Button>
      </div>
      {on && <video ref={video} autoPlay muted playsInline className="mt-3 aspect-video w-full max-w-md rounded-lg bg-black" />}
      {err && <p className="mt-2 text-xs text-warn">{err}</p>}
    </Card>
  );
}

export function Peripherals() {
  // El micrófono y la cámara se apagan al salir de la pestaña.
  const active = usePageActive();
  return (
    <div className="mx-auto grid max-w-(--page-max) gap-4 p-6 lg:grid-cols-2">
      <ScreenTest />
      <SpeakerTest />
      <div className="lg:col-span-2">
        <KeyboardTest active={active} />
      </div>
      <MicTest active={active} />
      <CameraTest active={active} />
    </div>
  );
}
