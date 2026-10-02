// Discos: lo de la 1.1.8. Tendencia de cada disco, velocidad, BitLocker de
// cada volumen, capacidad real de un pendrive, expulsar y formatear.
import { Copy, Eraser, Eye, Gauge, KeyRound, Loader2, LogOut, ShieldCheck, TestTube } from "lucide-react";
import { useState } from "react";
import { useConfirm, useToast } from "./feedback";
import { TaskStatus } from "./TaskStatus";
import { Button, inputClass, Modal } from "./ui";
import { disksApi, type DiskCapacity, type DiskPoint, type DiskReport, type DiskSpeed, type DiskVolume } from "../lib/api";
import { bytes } from "../lib/format";

const TEXT = { ok: "text-ok", warn: "text-warn", bad: "text-bad" };

// ---------- Tendencia ----------

/** Línea pequeña de una cifra a lo largo de los días. */
function Spark({ points, pick, label }: { points: DiskPoint[]; pick: (p: DiskPoint) => number | null; label: string }) {
  const vals = points.map(pick);
  const known = vals.filter((v): v is number => v !== null && v >= 0);
  if (known.length < 2) return null;
  const first = known[0];
  const last = known[known.length - 1];
  const max = Math.max(...known, 1);
  const w = 120;
  const h = 26;
  const step = w / Math.max(vals.length - 1, 1);
  const path = vals
    .map((v, i) => (v === null || v < 0 ? null : `${i * step},${h - 2 - (v / max) * (h - 4)}`))
    .filter(Boolean)
    .join(" ");
  const worse = last > first;
  return (
    <div className="flex items-center gap-2 text-[11px]">
      <span className="w-36 shrink-0 text-mute">{label}</span>
      <svg width={w} height={h} className="shrink-0" aria-hidden>
        <polyline points={path} fill="none" strokeWidth={1.6} className={worse ? "stroke-bad" : "stroke-ok"} />
      </svg>
      <span className={`font-mono ${worse ? "text-bad" : "text-dim"}`}>
        {first === last ? last : `${first} → ${last}`}
      </span>
    </div>
  );
}

/** Cómo han evolucionado las cifras de desgaste (si hay más de un día apuntado). */
export function DiskTrend({ d }: { d: DiskReport }) {
  if (d.trend.length < 2) {
    return d.trend.length === 1 ? <p className="px-4 pb-3 text-[11px] text-mute">Desde hoy se apunta una foto al día de sus cifras: en unos días verás si va a peor.</p> : null;
  }
  const since = new Date(d.trend[0].day * 86_400_000).toLocaleDateString("es", { day: "numeric", month: "short" });
  return (
    <div className="mx-4 mb-3 rounded-lg border border-line bg-panel-2 px-3 py-2">
      <div className="mb-1 text-[11px] text-dim">Evolución desde el {since} ({d.trend.length} días apuntados)</div>
      <Spark points={d.trend} pick={(p) => p.reallocated} label="Sectores apartados" />
      <Spark points={d.trend} pick={(p) => p.pending} label="Sectores pendientes" />
      <Spark points={d.trend} pick={(p) => p.readErrors} label="Errores de lectura" />
      <Spark points={d.trend} pick={(p) => p.crc} label="Errores de conexión" />
      <Spark points={d.trend} pick={(p) => p.wear} label="Desgaste (%)" />
    </div>
  );
}

// ---------- Velocidad ----------

export function SpeedTest({ d }: { d: DiskReport }) {
  const [busy, setBusy] = useState(false);
  const [r, setR] = useState<DiskSpeed | null>(null);
  const toast = useToast();
  // Se mide en el volumen con más espacio libre del disco.
  const vol = [...d.volumes].sort((a, b) => b.free - a.free)[0];
  if (!vol) return null;
  const run = async () => {
    setBusy(true);
    setR(null);
    try {
      setR(await disksApi.speed(vol.letter, d.media, d.bus));
    } catch (e) {
      toast("error", String(e));
    } finally {
      setBusy(false);
    }
  };
  return (
    <div className="px-4 pb-3">
      <div className="flex flex-wrap items-center gap-2">
        <Button kind="ghost" onClick={() => void run()} disabled={busy} title={`Escribe y lee 256 MB en ${vol.letter}: (se borran al acabar)`}>
          {busy ? <Loader2 size={13} className="animate-spin" /> : <Gauge size={13} />} Medir velocidad
        </Button>
        {busy && <TaskStatus task={`disk-speed:${vol.letter}`} active fallback="Midiendo…" />}
        {r && (
          <span className="flex flex-wrap gap-x-4 text-xs text-dim">
            <span>
              Escritura <b className="font-mono text-ink">{r.writeMbps.toFixed(0)} MB/s</b>
            </span>
            <span>
              Lectura <b className="font-mono text-ink">{r.readMbps.toFixed(0)} MB/s</b>
            </span>
            <span>
              Aleatorio <b className="font-mono text-ink">{Math.round(r.randomIops).toLocaleString("es")}</b> lecturas/s
            </span>
          </span>
        )}
      </div>
      {r && <p className={`mt-1 text-xs ${TEXT[r.level]}`}>{r.text}</p>}
    </div>
  );
}

// ---------- Por volumen: BitLocker, capacidad real, expulsar, formatear ----------

const BL: Record<string, { label: string; tone: string }> = {
  on: { label: "BitLocker activo", tone: "border-ok/40 text-ok" },
  suspended: { label: "BitLocker en pausa", tone: "border-warn/50 text-warn" },
  encrypting: { label: "Cifrando", tone: "border-neon/50 text-neon" },
  decrypting: { label: "Descifrando", tone: "border-warn/50 text-warn" },
};

export function BitLockerChip({ v, isAdmin }: { v: DiskVolume; isAdmin: boolean }) {
  const [keys, setKeys] = useState<{ id: string; password: string }[] | null>(null);
  const [busy, setBusy] = useState(false);
  const toast = useToast();
  const b = BL[v.bitlocker];
  if (!b) return null;
  const show = async () => {
    setBusy(true);
    try {
      setKeys(await disksApi.bitlockerKey(v.letter));
    } catch (e) {
      toast("error", String(e));
    } finally {
      setBusy(false);
    }
  };
  return (
    <>
      <span className={`flex items-center gap-1 rounded border px-1 text-[10px] ${b.tone}`}>
        <ShieldCheck size={10} /> {b.label}
        {(v.bitlocker === "encrypting" || v.bitlocker === "decrypting") && v.bitlockerPercent >= 0 && ` ${v.bitlockerPercent} %`}
      </span>
      {isAdmin && (
        <button onClick={() => void show()} disabled={busy} className="flex items-center gap-1 text-[10px] text-mute hover:text-neon" title="Clave de recuperación (queda anotado en el diario)">
          {busy ? <Loader2 size={10} className="animate-spin" /> : <KeyRound size={10} />} clave
        </button>
      )}
      {keys && (
        <Modal title={`Clave de recuperación de ${v.letter}:`} onClose={() => setKeys(null)} footer={<Button onClick={() => setKeys(null)}>Cerrar</Button>}>
          <p className="mb-3 text-xs text-dim">
            La pide Windows si cambia el hardware, la BIOS o se mueve el disco a otro equipo. Guárdala fuera de este equipo. Queda anotado en el diario que se
            consultó.
          </p>
          <ul className="space-y-2">
            {keys.map((k) => (
              <li key={k.id} className="rounded-lg border border-line bg-panel-2 p-3">
                <div className="text-[10px] text-mute">Id. {k.id.slice(0, 8)}</div>
                <div className="flex items-center gap-2">
                  <span className="min-w-0 flex-1 font-mono text-sm break-all text-ink select-text">{k.password}</span>
                  <button onClick={() => void navigator.clipboard.writeText(k.password).then(() => toast("ok", "Clave copiada."), () => {})} className="text-mute hover:text-neon" title="Copiar">
                    <Copy size={14} />
                  </button>
                </div>
              </li>
            ))}
          </ul>
        </Modal>
      )}
    </>
  );
}

/** Acciones de una unidad extraíble o externa: capacidad real, expulsar y formatear. */
export function RemovableActions({ v, size, onChanged }: { v: DiskVolume; size: number; onChanged: () => void }) {
  const [busy, setBusy] = useState<"capacity" | "eject" | null>(null);
  const [cap, setCap] = useState<DiskCapacity | null>(null);
  const [formatting, setFormatting] = useState(false);
  const toast = useToast();
  const { confirm, dialog } = useConfirm();

  const capacity = async () => {
    const ok = await confirm({
      title: `¿Capacidad real de ${v.letter}:?`,
      body: `Llena los ${bytes(v.free)} libres con datos de prueba y los vuelve a leer, para saber si el pendrive tiene de verdad la capacidad que dice. No toca lo que ya hay y borra la prueba al acabar. En pendrives lentos puede tardar mucho (se puede cancelar).`,
      confirmLabel: "Empezar",
    });
    if (!ok) return;
    setBusy("capacity");
    setCap(null);
    try {
      setCap(await disksApi.capacity(v.letter));
    } catch (e) {
      toast("error", String(e));
    } finally {
      setBusy(null);
    }
  };

  const eject = async () => {
    setBusy("eject");
    try {
      await disksApi.eject(v.letter);
      toast("ok", `${v.letter}: expulsado. Ya se puede quitar.`);
      onChanged();
    } catch (e) {
      toast("error", String(e));
    } finally {
      setBusy(null);
    }
  };

  return (
    <div className="mt-2">
      <div className="flex flex-wrap gap-1.5">
        <Button kind="ghost" onClick={() => void capacity()} disabled={busy !== null} title="¿Pendrive falso? Comprueba la capacidad real">
          {busy === "capacity" ? <Loader2 size={13} className="animate-spin" /> : <TestTube size={13} />} ¿Capacidad real?
        </Button>
        <Button kind="ghost" onClick={() => void eject()} disabled={busy !== null} title="Expulsar para quitarlo sin dañar nada">
          {busy === "eject" ? <Loader2 size={13} className="animate-spin" /> : <LogOut size={13} />} Expulsar
        </Button>
        <Button kind="ghost" onClick={() => setFormatting(true)} disabled={busy !== null}>
          <Eraser size={13} /> Formatear
        </Button>
      </div>
      {busy === "capacity" && <TaskStatus task={`disk-capacity:${v.letter}`} active className="mt-2" fallback="Escribiendo datos de prueba…" />}
      {cap && (
        <div className={`mt-2 rounded-lg border p-2 text-xs ${cap.fake ? "border-bad/40 bg-bad/10" : "border-ok/40 bg-ok/10"}`}>
          <p className={cap.fake ? "text-bad" : "text-ok"}>{cap.text}</p>
          <p className="mt-1 text-mute">
            Escritura {cap.writeMbps.toFixed(0)} MB/s · lectura {cap.readMbps.toFixed(0)} MB/s
          </p>
        </div>
      )}
      {formatting && (
        <FormatDialog
          v={v}
          size={size}
          onClose={() => setFormatting(false)}
          onDone={() => {
            setFormatting(false);
            onChanged();
          }}
        />
      )}
      {dialog}
    </div>
  );
}

function FormatDialog({ v, size, onClose, onDone }: { v: DiskVolume; size: number; onClose: () => void; onDone: () => void }) {
  // exFAT sirve en Windows, Mac, televisores y coches modernos y admite archivos grandes.
  const canFat32 = size <= 32 * 1024 ** 3;
  const [fs, setFs] = useState("exFAT");
  const [label, setLabel] = useState(v.label);
  const [typed, setTyped] = useState("");
  const [busy, setBusy] = useState(false);
  const toast = useToast();
  const ready = typed.trim().toUpperCase().replace(":", "") === v.letter.toUpperCase();

  const go = async () => {
    setBusy(true);
    try {
      await disksApi.format(v.letter, fs, label);
      toast("ok", `${v.letter}: formateado como ${fs}.`);
      onDone();
    } catch (e) {
      toast("error", String(e));
      setBusy(false);
    }
  };

  const options = [
    { id: "exFAT", text: "Recomendado. Funciona en Windows, Mac, televisores y la mayoría de coches, y admite archivos de más de 4 GB." },
    { id: "FAT32", text: canFat32 ? "Para aparatos antiguos (radios de coche, algunas TV). No admite archivos de más de 4 GB." : "Windows solo lo permite en unidades de hasta 32 GB." },
    { id: "NTFS", text: "Solo para usar en Windows (en Mac se lee pero no se escribe). Más robusto si se desconecta sin expulsar." },
  ];

  return (
    <Modal
      title={`Formatear ${v.letter}:`}
      onClose={onClose}
      footer={
        <>
          <Button kind="ghost" onClick={onClose}>
            Cancelar
          </Button>
          <Button kind="danger" onClick={() => void go()} disabled={!ready || busy}>
            {busy ? <Loader2 size={14} className="animate-spin" /> : <Eraser size={14} />} Borrar todo y formatear
          </Button>
        </>
      }
    >
      <p className="mb-3 rounded-lg border border-bad/40 bg-bad/10 px-3 py-2 text-xs text-bad">
        Se borra todo lo que hay en {v.letter}: ({v.label || "sin nombre"}, {bytes(v.size - v.free)} ocupados). No se puede deshacer.
      </p>
      <div className="space-y-2">
        {options.map((o) => (
          <label key={o.id} className={`flex items-start gap-2 rounded-lg border p-2 text-xs ${fs === o.id ? "border-neon/50 bg-neon/5" : "border-line"} ${o.id === "FAT32" && !canFat32 ? "opacity-50" : ""}`}>
            <input type="radio" name="fs" checked={fs === o.id} disabled={o.id === "FAT32" && !canFat32} onChange={() => setFs(o.id)} className="mt-0.5 accent-[var(--color-neon)]" />
            <span>
              <b className="text-ink">{o.id}</b> <span className="text-dim">{o.text}</span>
            </span>
          </label>
        ))}
      </div>
      <label className="mt-3 block text-xs text-dim">
        Nombre de la unidad
        <input value={label} onChange={(e) => setLabel(e.target.value.slice(0, 32))} className={`${inputClass} mt-1`} />
      </label>
      <label className="mt-3 block text-xs text-dim">
        Para confirmar, escribe la letra de la unidad ({v.letter})
        <input value={typed} onChange={(e) => setTyped(e.target.value)} className={`${inputClass} mt-1 font-mono`} autoFocus />
      </label>
      <p className="mt-2 flex items-center gap-1 text-[11px] text-mute">
        <Eye size={11} /> Queda anotado en el diario.
      </p>
    </Modal>
  );
}
