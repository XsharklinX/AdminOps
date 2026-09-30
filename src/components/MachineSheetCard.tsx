import { ClipboardCopy, Download, ExternalLink, FileSpreadsheet, IdCard, Loader2, RefreshCw, UserPlus } from "lucide-react";
import { useEffect, useState } from "react";
import { useToast } from "./feedback";
import { Button, Card } from "./ui";
import { officeApi, sheetApi, workApi, type Client, type MachineSheet } from "../lib/api";
import { useLiveEffect } from "../lib/useLiveEffect";

const yesNo = (v: boolean | null) => (v == null ? "Desconocido (requiere administrador)" : v ? "Sí" : "No");

export function rows(s: MachineSheet): [string, string][] {
  return [
    ["Equipo", s.host],
    ["Usuario", s.user || "—"],
    [s.partOfDomain ? "Dominio" : "Grupo de trabajo", s.domain],
    ["Fabricante", s.manufacturer],
    ["Modelo", s.model],
    ["Número de serie", s.serial],
    ["Tipo", s.chassis],
    ["Procesador", `${s.cpu}${s.cores ? ` (${s.cores} núcleos)` : ""}`],
    ["Memoria RAM", `${s.ramGb} GB`],
    ["Discos", s.disks],
    ["Gráfica", s.gpu],
    ["Windows", `${s.os} ${s.osVersion}`],
    ["Instalado el", s.installed],
    ["Licencia de Windows", s.license],
    ["BIOS", s.bios],
    ["TPM", yesNo(s.tpm)],
    ["Arranque seguro", yesNo(s.secureBoot)],
    ["IP", s.ip],
    ["MAC", s.mac],
  ];
}

/** Ficha del equipo para el inventario de la empresa: copiar, Excel, garantía, cliente. */
export function MachineSheetCard({ autoLoad = false }: { autoLoad?: boolean }) {
  const [sheet, setSheet] = useState<MachineSheet | null>(null);
  const [busy, setBusy] = useState(false);
  const [clients, setClients] = useState<Client[]>([]);
  const [clientId, setClientId] = useState("");
  const toast = useToast();

  const load = async () => {
    setBusy(true);
    try {
      setSheet(await sheetApi.get());
    } catch (e) {
      toast("error", String(e));
    } finally {
      setBusy(false);
    }
  };

  useLiveEffect(
    (vigente) => {
      if (autoLoad) void load();
      workApi.clients().then((c) => vigente() && setClients(c)).catch(() => {});
    },
    // eslint-disable-next-line react-hooks/exhaustive-deps -- `load` se redefine en cada render: incluirlo repetiría la consulta sin parar
    [autoLoad],
  );

  const copyText = () => {
    if (!sheet) return;
    void navigator.clipboard.writeText(rows(sheet).map(([k, v]) => `${k}: ${v}`).join("\n")).then(() => toast("ok", "Ficha copiada."));
  };
  // Tabulado: se pega en Excel como una fila con su cabecera.
  const copyExcel = () => {
    if (!sheet) return;
    const r = rows(sheet);
    void navigator.clipboard.writeText(`${r.map(([k]) => k).join("\t")}\n${r.map(([, v]) => v.replace(/\t|\n/g, " ")).join("\t")}`).then(() => toast("ok", "Copiada para pegar en Excel."));
  };
  const exportCsv = () => {
    if (!sheet) return;
    const r = rows(sheet);
    const cell = (c: string) => (/[;"\n]/.test(c) ? `"${c.replace(/"/g, '""')}"` : c);
    officeApi.exportCsv(`Ficha ${sheet.host}`, `${r.map(([k]) => cell(k)).join(";")}\r\n${r.map(([, v]) => cell(v)).join(";")}`).catch((e) => toast("error", String(e)));
  };
  const addToClient = async () => {
    if (!clientId) return;
    try {
      await workApi.inventoryAddThis(clientId);
      toast("ok", "Equipo añadido al inventario del cliente.");
    } catch (e) {
      toast("error", String(e));
    }
  };

  return (
    <Card
      id="focus-sheet"
      title="Ficha del equipo"
      icon={<IdCard size={14} />}
      className="col-span-12"
      right={
        sheet && (
          <button onClick={load} disabled={busy} className="flex items-center gap-1 text-[11px] text-mute hover:text-ink">
            <RefreshCw size={11} className={busy ? "animate-spin" : ""} /> Volver a leer
          </button>
        )
      }
    >
      {!sheet ? (
        <div className="flex items-center gap-3">
          <p className="flex-1 text-sm text-dim">Modelo, número de serie, hardware, licencia, red y usuario, listo para pegar en el inventario de la empresa.</p>
          <Button onClick={load} disabled={busy}>
            {busy ? <Loader2 size={13} className="animate-spin" /> : <IdCard size={13} />} Generar ficha
          </Button>
        </div>
      ) : (
        <>
          <div className="grid gap-x-8 gap-y-1 text-sm md:grid-cols-2">
            {rows(sheet).map(([k, v]) => (
              <div key={k} className="flex gap-3 border-b border-line/40 py-1">
                <span className="w-36 shrink-0 text-xs text-mute">{k}</span>
                <span className={`min-w-0 flex-1 break-words select-text ${k === "Licencia de Windows" && v !== "Activado" ? "text-warn" : "text-ink"}`}>{v || "—"}</span>
              </div>
            ))}
          </div>
          <div className="mt-4 flex flex-wrap items-center gap-2">
            <Button onClick={copyText}>
              <ClipboardCopy size={13} /> Copiar
            </Button>
            <Button kind="ghost" onClick={copyExcel}>
              <FileSpreadsheet size={13} /> Copiar para Excel
            </Button>
            <Button kind="ghost" onClick={exportCsv}>
              <Download size={13} /> CSV
            </Button>
            {sheet.warrantyUrl && (
              <Button kind="ghost" onClick={() => sheetApi.openWarranty(sheet.warrantyUrl!).catch((e) => toast("error", String(e)))}>
                <ExternalLink size={13} /> Ver garantía
              </Button>
            )}
            {clients.length > 0 && (
              <span className="ml-auto flex items-center gap-1.5">
                <select value={clientId} onChange={(e) => setClientId(e.target.value)} className="h-9 rounded-md border border-line bg-void/60 px-2 text-xs text-ink">
                  <option value="">Añadir al inventario de…</option>
                  {clients.map((c) => (
                    <option key={c.id} value={c.id}>
                      {c.name}
                    </option>
                  ))}
                </select>
                <Button kind="ghost" onClick={addToClient} disabled={!clientId}>
                  <UserPlus size={13} />
                </Button>
              </span>
            )}
          </div>
        </>
      )}
    </Card>
  );
}

/**
 * Panel lateral con los datos del equipo, cada uno para copiar con un clic:
 * para rellenar el formulario del inventario web de la empresa.
 */
export function SheetPanel({ onClose }: { onClose: () => void }) {
  const [sheet, setSheet] = useState<MachineSheet | null>(null);
  const [error, setError] = useState<string | null>(null);
  const [copied, setCopied] = useState<string | null>(null);

  useEffect(() => {
    sheetApi
      .get()
      .then(setSheet)
      .catch((e) => setError(String(e)));
  }, []);

  const copy = (k: string, v: string) =>
    navigator.clipboard.writeText(v).then(() => {
      setCopied(k);
      window.setTimeout(() => setCopied((c) => (c === k ? null : c)), 1500);
    });

  return (
    <aside className="flex w-72 shrink-0 flex-col border-l border-line bg-panel">
      <div className="flex items-center justify-between border-b border-line px-3 py-2">
        <span className="flex items-center gap-1.5 text-sm font-medium text-ink">
          <IdCard size={14} className="text-neon" /> Datos del equipo
        </span>
        <button onClick={onClose} className="text-xs text-mute hover:text-ink">
          Cerrar
        </button>
      </div>
      <p className="px-3 pt-2 text-[11px] text-mute">Clic en un dato para copiarlo y pégalo en el formulario.</p>
      <div className="min-h-0 flex-1 overflow-y-auto p-2">
        {error ? (
          <p className="p-2 text-xs text-bad">{error}</p>
        ) : !sheet ? (
          <p className="flex items-center gap-2 p-2 text-xs text-mute">
            <Loader2 size={12} className="animate-spin" /> Leyendo el equipo…
          </p>
        ) : (
          rows(sheet)
            .filter(([, v]) => v && v !== "—")
            .map(([k, v]) => (
              <button key={k} onClick={() => copy(k, v)} className="block w-full rounded-md px-2 py-1.5 text-left hover:bg-panel-2" title="Copiar">
                <span className="flex items-center justify-between text-[10px] tracking-wide text-mute uppercase">
                  {k}
                  {copied === k && <span className="normal-case text-ok">copiado</span>}
                </span>
                <span className="block truncate text-xs text-ink">{v}</span>
              </button>
            ))
        )}
      </div>
      {sheet && (
        <div className="border-t border-line p-2">
          <button
            onClick={() => copy("__all", rows(sheet).map(([k, v]) => `${k}: ${v}`).join("\n"))}
            className="flex w-full items-center justify-center gap-1.5 rounded-md border border-neon/40 py-1.5 text-xs text-neon hover:bg-neon/10"
          >
            <ClipboardCopy size={12} /> {copied === "__all" ? "Ficha copiada" : "Copiar la ficha entera"}
          </button>
        </div>
      )}
    </aside>
  );
}
