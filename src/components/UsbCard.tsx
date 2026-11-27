import { Check, Clock, History, Loader2, Usb, X } from "lucide-react";
import { useCallback, useEffect, useState } from "react";
import { casesApi } from "../lib/api";
import { usbApi, type HiddenUsb, type UsbMode, type UsbSeen, type UsbState } from "../lib/api/usb";
import { caseChanged } from "../lib/currentCase";
import { appendNote } from "../lib/sendTo";
import { useToast } from "./feedback";
import { Button, Card, Notice } from "./ui";

const MODES: { id: UsbMode; label: string }[] = [
  { id: "allowed", label: "Permitidas" },
  { id: "readonly", label: "Solo lectura" },
  { id: "blocked", label: "Bloqueadas" },
];

/** Lo que se puede hacer con una memoria en cada modo, como en la maqueta aprobada. */
const CAN: Record<UsbMode, [boolean, boolean]> = {
  allowed: [true, true],
  readonly: [true, false],
  blocked: [false, false],
};

/** El número de serie abreviado («60A4…E1»): entero no cabe y no hace falta para reconocer la memoria. */
export function shortSerial(s: string): string {
  return s.length > 8 ? `${s.slice(0, 4)}…${s.slice(-2)}` : s;
}

/**
 * Memorias USB: permitidas, solo lectura o bloqueadas, en este equipo. Teclado, ratón, impresora y
 * lector de huella siguen funcionando. Para usar una memoria un rato, «Permitir 15 minutos».
 */
export function UsbCard({ isAdmin }: { isAdmin: boolean }) {
  const [st, setSt] = useState<UsbState | null>(null);
  const [error, setError] = useState("");
  const [busy, setBusy] = useState(false);
  // Las que se han conectado alguna vez: se piden al pulsar, porque tarda unos segundos.
  const [seen, setSeen] = useState<UsbSeen[] | null>(null);
  const [seenBusy, setSeenBusy] = useState(false);
  // «Meto la memoria y no aparece»: sin letra, o con la letra de una unidad de red que la tapa.
  const [hidden, setHidden] = useState<HiddenUsb[] | null>(null);
  const [hiddenBusy, setHiddenBusy] = useState(false);
  const [fixed, setFixed] = useState<string[] | null>(null);
  const toast = useToast();

  const load = useCallback(() => {
    usbApi
      .state()
      .then((s) => {
        setSt(s);
        setError("");
      })
      .catch((e) => setError(String(e)));
  }, []);
  useEffect(load, [load]);

  if (error) return <Card title="Memorias USB" icon={<Usb size={14} />}><p className="text-sm text-bad">{error}</p></Card>;
  if (!st) return null;

  const run = async (p: Promise<UsbState>, ok: string) => {
    setBusy(true);
    try {
      setSt(await p);
      toast("ok", ok);
    } catch (e) {
      toast("error", String(e));
    } finally {
      setBusy(false);
    }
  };
  // «Meto la memoria y no aparece»: sin letra, o con la letra de una unidad de red que la tapa.
  const checkHidden = async () => {
    setHiddenBusy(true);
    setFixed(null);
    try {
      setHidden(await usbApi.hidden());
    } catch (e) {
      toast("error", String(e));
    } finally {
      setHiddenBusy(false);
    }
  };
  const fixHidden = async () => {
    setHiddenBusy(true);
    try {
      setFixed(await usbApi.fixLetters());
      setHidden(await usbApi.hidden());
    } catch (e) {
      toast("error", String(e));
    } finally {
      setHiddenBusy(false);
    }
  };
  const loadSeen = async () => {
    setSeenBusy(true);
    try {
      setSeen(await usbApi.history());
    } catch (e) {
      toast("error", String(e));
    } finally {
      setSeenBusy(false);
    }
  };
  const noteSeen = async () => {
    try {
      const c = await casesApi.current();
      if (!c) {
        toast("info", "No hay ningún caso abierto: ábrelo y vuelve a pulsar «Apuntar en el caso».");
        return;
      }
      const text = (seen ?? []).map((u) => `${u.name} · serie ${u.serial} · ${u.last ?? "fecha desconocida"}`).join("\n");
      await casesApi.update({ ...c, notes: appendNote(c.notes, "Memorias USB conectadas a este equipo", text) });
      caseChanged();
      toast("ok", "Añadido a las notas del caso.");
    } catch (e) {
      toast("error", String(e));
    }
  };
  const [open, copy] = CAN[st.mode];
  const disabled = !isAdmin || busy;
  const line = (ok: boolean, text: string, value: string) => (
    <div className="flex items-center justify-between gap-3 py-1.5 text-sm">
      <span className="text-dim">{text}</span>
      <span className={`flex shrink-0 items-center gap-1 font-medium ${ok ? "text-ok" : "text-bad"}`}>
        {ok ? <Check size={13} /> : <X size={13} />}
        {value}
      </span>
    </div>
  );

  return (
    <Card title="Memorias USB" icon={<Usb size={14} />} right={st.until ? <span className="flex items-center gap-1 text-[11px] text-warn"><Clock size={11} /> Permitidas hasta las {st.until}</span> : undefined}>
      <div role="radiogroup" aria-label="Memorias USB en este equipo" className="mb-3 grid grid-cols-3 gap-1 rounded-lg border border-line bg-void/50 p-1">
        {MODES.map((m) => (
          <button
            key={m.id}
            type="button"
            role="radio"
            aria-checked={st.mode === m.id}
            disabled={disabled || (!!st.portableDrive && m.id !== "allowed")}
            onClick={() => st.mode !== m.id && void run(usbApi.set(m.id), m.id === "allowed" ? "Memorias USB permitidas." : m.id === "readonly" ? "Memorias USB en solo lectura." : "Memorias USB bloqueadas.")}
            className={`rounded-md px-2 py-1.5 text-sm transition-colors disabled:opacity-60 ${st.mode === m.id ? (m.id === "allowed" ? "bg-panel-2 text-ink" : m.id === "readonly" ? "bg-warn/15 text-warn" : "bg-bad/15 text-bad") : "text-dim hover:text-ink"}`}
          >
            {m.label}
          </button>
        ))}
      </div>
      <div className="divide-y divide-line/60 rounded-lg border border-line px-3">
        {line(open, "Abrir archivos de una memoria", open ? "sí" : "no")}
        {line(copy, "Copiar o guardar en una memoria", copy ? "sí" : "no")}
        {line(true, "Teclado, ratón, impresora, lector de huella", "funcionan")}
        {st.mode !== "allowed" && line(false, "Discos USB externos (también los de copias de seguridad)", st.mode === "readonly" ? "solo lectura" : "bloqueados")}
      </div>
      <div className="mt-3 flex flex-wrap items-center gap-3">
        {st.mode !== "allowed" && !st.until && (
          <Button size="sm" kind="secondary" disabled={disabled} onClick={() => run(usbApi.allowFor(15), "Memorias USB permitidas 15 minutos; luego vuelven solas a como estaban.")}>
            <Clock size={13} /> Permitir 15 minutos
          </Button>
        )}
        <span className="text-xs text-mute">
          {st.until
            ? "Vuelven solas a como estaban, aunque se cierre AdminOps o se reinicie el equipo."
            : "Se deshace igual de fácil. Una memoria que ya estaba puesta: sácala y vuelve a meterla."}
        </span>
        {!isAdmin && <span className="text-xs text-warn">Hace falta modo técnico (administrador).</span>}
      </div>
      {st.portableDrive && (
        <Notice tone="warn" className="mt-3">
          AdminOps se está ejecutando desde la memoria {st.portableDrive}. Windows bloquea todas las memorias por igual, así que aquí solo se pueden permitir: para bloquearlas, ábrela instalada en este equipo.
        </Notice>
      )}
      <div className="mt-4 border-t border-line/60 pt-3">
        <div className="flex flex-wrap items-center justify-between gap-2">
          <div className="text-sm text-ink">¿Una memoria conectada no aparece en el Explorador?</div>
          <Button size="sm" kind="secondary" disabled={hiddenBusy} onClick={checkHidden}>
            {hiddenBusy ? <Loader2 size={13} className="animate-spin" /> : <Usb size={13} />} {hidden ? "Volver a mirar" : "Mirar"}
          </Button>
        </div>
        {hidden && hidden.length === 0 && !fixed && <p className="mt-2 text-sm text-mute">Todas las memorias conectadas tienen su letra y se ven.</p>}
        {hidden && hidden.length > 0 && (
          <div className="mt-2 space-y-2">
            <ul className="divide-y divide-line/60 rounded-lg border border-line px-3">
              {hidden.map((h) => (
                <li key={`${h.disk}-${h.partition}`} className="flex items-center justify-between gap-3 py-1.5 text-sm">
                  <span className="min-w-0 truncate text-ink">{h.name}</span>
                  <span className="shrink-0 text-xs text-warn">{h.reason === "sin-letra" ? "sin letra de unidad" : `${h.reason} la tapa una unidad de red`}</span>
                </li>
              ))}
            </ul>
            <Button size="sm" disabled={hiddenBusy || !isAdmin} onClick={fixHidden}>
              Darles una letra libre y abrirla
            </Button>
          </div>
        )}
        {fixed && fixed.length > 0 && <Notice tone="ok" className="mt-2">Listo: {fixed.join(" · ")}.</Notice>}
      </div>
      <div className="mt-4 border-t border-line/60 pt-3">
        <div className="flex flex-wrap items-center justify-between gap-2">
          <div className="text-[11px] font-medium uppercase tracking-wide text-mute">Se han conectado a este equipo</div>
          <div className="flex items-center gap-2">
            {seen && seen.length > 0 && (
              <Button size="sm" kind="ghost" onClick={noteSeen}>
                Apuntar en el caso
              </Button>
            )}
            <Button size="sm" kind="secondary" disabled={seenBusy} onClick={loadSeen}>
              {seenBusy ? <Loader2 size={13} className="animate-spin" /> : <History size={13} />} {seen ? "Volver a mirar" : "Ver las que se han conectado"}
            </Button>
          </div>
        </div>
        {seen && (
          <div className="mt-2">
            {seen.length === 0 ? (
              <p className="text-sm text-mute">Windows no tiene registrada ninguna memoria USB.</p>
            ) : (
              <ul className="divide-y divide-line/60 rounded-lg border border-line px-3">
                {seen.map((u) => (
                  <li key={`${u.name}-${u.serial}`} className="flex items-center justify-between gap-3 py-1.5 text-sm">
                    <span className="min-w-0 truncate text-ink">
                      {u.name} <span className="text-mute">· {shortSerial(u.serial)}</span>
                    </span>
                    <span className="shrink-0 text-xs text-mute tabular">{u.last ?? "sin fecha"}</span>
                  </li>
                ))}
              </ul>
            )}
          </div>
        )}
      </div>
    </Card>
  );
}
