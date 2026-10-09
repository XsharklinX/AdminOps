// Etiqueta con QR de un equipo: se imprime y se pega en la caja. Al escanearla con el móvil
// se ve la ficha mínima (equipo, dueño, última visita y a quién llamar), sin internet.
import { Loader2, Printer, QrCode } from "lucide-react";
import { useEffect, useState } from "react";
import { useToast } from "./feedback";
import { Button, inputClass, iconBtn, Modal } from "./ui";
import { labelsApi, type Client, type LabelInfo, type Machine } from "../lib/api";

const day = (secs: number) => (secs ? new Date(secs * 1000).toLocaleDateString("es", { day: "2-digit", month: "2-digit", year: "numeric" }) : "");

export const labelOf = (m: Machine, client: Client | undefined, extra?: Partial<LabelInfo>): LabelInfo => ({
  host: m.host,
  owner: client?.name ?? "",
  place: "",
  lastVisit: day(m.lastSeen),
  phone: client?.phone ?? "",
  extension: "",
  ...extra,
});

/** Botón que abre la etiqueta del equipo. */
export function LabelButton({ machine, client }: { machine: Machine; client?: Client }) {
  const [open, setOpen] = useState(false);
  return (
    <>
      <button type="button" onClick={() => setOpen(true)} className={iconBtn} title="Etiqueta con QR para pegar en el equipo">
        <QrCode size={14} />
      </button>
      {open && <LabelDialog machine={machine} client={client} onClose={() => setOpen(false)} />}
    </>
  );
}

function LabelDialog({ machine, client, onClose }: { machine: Machine; client?: Client; onClose: () => void }) {
  const toast = useToast();
  const [info, setInfo] = useState<LabelInfo>(() => labelOf(machine, client));
  const [svg, setSvg] = useState<string | null>(null);
  const [busy, setBusy] = useState(false);

  useEffect(() => {
    let live = true;
    const t = window.setTimeout(() => {
      labelsApi
        .qr(info)
        .then((s) => live && setSvg(s))
        .catch(() => undefined);
    }, 200);
    return () => {
      live = false;
      window.clearTimeout(t);
    };
  }, [info]);

  const print = async (labels: LabelInfo[], cols: number, rows: number) => {
    setBusy(true);
    try {
      await labelsApi.sheet(labels, cols, rows);
    } catch (e) {
      toast("error", String(e));
    } finally {
      setBusy(false);
    }
  };

  const others = client?.machines ?? [];
  const field = (label: string, key: keyof LabelInfo) => (
    <label className="block text-xs text-dim">
      {label}
      <input value={info[key]} onChange={(e) => setInfo({ ...info, [key]: e.target.value })} className={`mt-1 ${inputClass}`} />
    </label>
  );

  return (
    <Modal
      title={`Etiqueta de ${machine.host}`}
      onClose={onClose}
      width="w-[560px]"
      footer={
        <>
          {others.length > 1 && (
            <Button kind="ghost" disabled={busy} onClick={() => void print(others.map((m) => labelOf(m, client, { place: m.host === machine.host ? info.place : "" })), 3, 8)}>
              Hoja con los {others.length} equipos
            </Button>
          )}
          <Button disabled={busy || !info.host.trim()} onClick={() => void print([info], 2, 6)}>
            {busy ? <Loader2 size={14} className="animate-spin" /> : <Printer size={14} />} Imprimir etiqueta
          </Button>
        </>
      }
    >
      <div className="flex flex-wrap gap-4">
        <div className="flex w-full items-center gap-4 rounded-lg border border-line bg-white p-3 text-[#111] sm:w-auto">
          {svg ? <img src={`data:image/svg+xml;utf8,${encodeURIComponent(svg)}`} alt="Código QR de la ficha" className="size-28 shrink-0" /> : <div className="size-28 shrink-0" />}
          <div className="min-w-0 text-xs">
            <div className="text-sm font-semibold">{info.host}</div>
            {(info.owner || info.place) && <div>{[info.owner, info.place].filter(Boolean).join(" · ")}</div>}
            {(info.phone || info.extension) && (
              <div>
                Soporte {info.phone}
                {info.extension && ` · ext. ${info.extension}`}
              </div>
            )}
            {info.lastVisit && <div className="text-[#555]">Última visita {info.lastVisit}</div>}
          </div>
        </div>
        <div className="grid min-w-0 flex-1 gap-2">
          {field("Dueño o cliente", "owner")}
          {field("Dónde está", "place")}
          {field("Teléfono de soporte", "phone")}
          {field("Extensión", "extension")}
        </div>
      </div>
      <p className="mt-3 text-[11px] text-mute">El QR lleva el texto de la ficha, no un enlace: se lee con la cámara del móvil, sin internet y sin AdminOps.</p>
    </Modal>
  );
}
