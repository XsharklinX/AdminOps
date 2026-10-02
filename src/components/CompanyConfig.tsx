// Configuración de empresa: exportarla para dársela a otro técnico, o
// importarla para empezar con todo listo (portales, dominio,
// datos de la empresa). Nunca lleva contraseñas ni sesiones.
import { Building2, Download, Loader2, Upload } from "lucide-react";
import { useState } from "react";
import { useToast } from "./feedback";
import { Button, Card, Modal } from "./ui";
import { companyApi, type CompanyPreview } from "../lib/api";

/** Botón + diálogo para importar (también lo usa Primeros pasos). */
export function useCompanyImport(onDone?: () => void) {
  const [preview, setPreview] = useState<CompanyPreview | null>(null);
  const toast = useToast();
  const start = async () => {
    try {
      const p = await companyApi.preview();
      if (p) setPreview(p);
    } catch (e) {
      toast("error", String(e));
    }
  };
  const dialog = preview ? (
    <ImportDialog
      p={preview}
      onClose={() => setPreview(null)}
      onDone={(msg) => {
        setPreview(null);
        toast("ok", msg);
        onDone?.();
      }}
    />
  ) : null;
  return { start, dialog };
}

function ImportDialog({ p, onClose, onDone }: { p: CompanyPreview; onClose: () => void; onDone: (msg: string) => void }) {
  const [settings, setSettings] = useState(true);
  const [portals, setPortals] = useState(p.portalsNew.length > 0);
  const [busy, setBusy] = useState(false);
  const toast = useToast();

  const apply = async () => {
    setBusy(true);
    try {
      onDone(await companyApi.apply(p.path, settings, portals));
    } catch (e) {
      toast("error", String(e));
      setBusy(false);
    }
  };

  const row = (on: boolean, set: (v: boolean) => void, title: string, detail: string, disabled = false) => (
    <label className={`flex items-start gap-2 rounded-lg border border-line p-3 text-sm ${disabled ? "opacity-50" : ""}`}>
      <input type="checkbox" checked={on && !disabled} disabled={disabled} onChange={(e) => set(e.target.checked)} className="mt-1 accent-[var(--color-neon)]" />
      <span>
        <span className="text-ink">{title}</span>
        <span className="block text-xs text-dim">{detail}</span>
      </span>
    </label>
  );

  return (
    <Modal
      title={p.company ? `Configuración de ${p.company}` : "Configuración de empresa"}
      onClose={onClose}
      width="w-[520px]"
      footer={
        <>
          <Button kind="ghost" onClick={onClose}>
            Cancelar
          </Button>
          <Button onClick={() => void apply()} disabled={busy || (!settings && !portals)}>
            {busy && <Loader2 size={14} className="animate-spin" />} Importar
          </Button>
        </>
      }
    >
      <div className="space-y-2">
        {row(
          settings,
          setSettings,
          "Datos de la empresa",
          [p.company || "Sin nombre", p.domain && `dominio ${p.domain}`, p.visitTypes && `${p.visitTypes} tipos de visita`, p.catalog && `${p.catalog} precios`].filter(Boolean).join(" · "),
        )}
        {row(
          portals,
          setPortals,
          p.portalsNew.length ? `Portales: ${p.portalsNew.join(", ")}` : "Portales",
          p.portalsNew.length ? `${p.portalsExisting ? `${p.portalsExisting} ya los tenías; ` : ""}tendrás que entrar con tu cuenta en cada uno.` : "Ya tienes todos los del archivo.",
          p.portalsNew.length === 0,
        )}
      </div>
      <p className="mt-3 text-[11px] text-mute">No trae contraseñas, sesiones, tu nombre ni tu firma: eso lo pone cada técnico.</p>
    </Modal>
  );
}

/** Tarjeta de Ajustes → General. */
export function CompanyConfig() {
  const [busy, setBusy] = useState(false);
  const toast = useToast();
  const imp = useCompanyImport(() => window.setTimeout(() => window.location.reload(), 600));

  const exp = async () => {
    setBusy(true);
    try {
      const name = await companyApi.export();
      if (name) toast("ok", `Guardada como ${name}. Pásasela al otro técnico.`);
    } catch (e) {
      toast("error", String(e));
    } finally {
      setBusy(false);
    }
  };

  return (
    <Card title="Configuración de empresa" icon={<Building2 size={14} />}>
      <p className="text-xs text-dim">
        Para que otro técnico empiece con todo listo: los datos de la empresa (logo, condiciones, precios, tipos de visita), los portales y el dominio, en un
        archivo. No lleva contraseñas ni sesiones: cada técnico entra con su cuenta.
      </p>
      <div className="mt-3 flex flex-wrap gap-2">
        <Button kind="ghost" onClick={() => void exp()} disabled={busy}>
          {busy ? <Loader2 size={14} className="animate-spin" /> : <Download size={14} />} Exportar
        </Button>
        <Button kind="ghost" onClick={() => void imp.start()}>
          <Upload size={14} /> Importar
        </Button>
      </div>
      {imp.dialog}
    </Card>
  );
}
