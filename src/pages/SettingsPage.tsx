import { ArrowDown, ArrowUp, BadgeCheck, ImagePlus, PenLine, Plus, Receipt, Save, Trash2, X } from "lucide-react";
import { useEffect, useState } from "react";
import logo from "../assets/logo.svg";
import { useToast } from "../components/feedback";
import { SignaturePad } from "../components/service";
import { Card } from "../components/ui";
import { appApi, workApi, type AppInfo, type Settings } from "../lib/api";
import { getTheme, setTheme, type Theme } from "../lib/theme";

export function SettingsPage({ appInfo }: { appInfo: AppInfo | null }) {
  const [s, setS] = useState<Settings | null>(null);
  const [newItem, setNewItem] = useState("");
  const [dirty, setDirty] = useState(false);
  const toast = useToast();

  useEffect(() => {
    workApi.settings().then(setS);
  }, []);

  if (!s) return <p className="p-8 font-mono text-sm text-mute">Cargando…</p>;

  const set = (patch: Partial<Settings>) => {
    setS({ ...s, ...patch });
    setDirty(true);
  };

  const save = async () => {
    try {
      await workApi.saveSettings(s);
      setDirty(false);
      toast("ok", "Ajustes guardados. Se usarán en los próximos informes.");
    } catch (e) {
      toast("error", String(e));
    }
  };

  const pickLogo = (file: File | undefined) => {
    if (!file) return;
    if (file.size > 500 * 1024) {
      toast("error", "El logo debe pesar menos de 500 KB.");
      return;
    }
    const r = new FileReader();
    r.onload = () => set({ logo: String(r.result) });
    r.readAsDataURL(file);
  };

  const move = (i: number, d: number) => {
    const list = [...s.checklist];
    const j = i + d;
    if (j < 0 || j >= list.length) return;
    [list[i], list[j]] = [list[j], list[i]];
    set({ checklist: list });
  };

  const numField = (key: "taxRate" | "laborWarrantyDays" | "maintenanceMonths" | "quoteValidityDays", label: string, max: number) => (
    <label className="block">
      <span className="mb-1 block truncate text-xs text-dim" title={label}>
        {label}
      </span>
      <input
        type="number"
        min={0}
        max={max}
        step={key === "taxRate" ? "any" : 1}
        value={s[key]}
        onChange={(e) => set({ [key]: Math.min(max, Math.max(0, Number(e.target.value) || 0)) })}
        className={input}
      />
    </label>
  );
  const setItem = (i: number, patch: Partial<Settings["catalog"][number]>) => set({ catalog: s.catalog.map((c, j) => (j === i ? { ...c, ...patch } : c)) });

  const input = "w-full rounded-md border border-line bg-void/60 px-3 py-2 text-sm text-ink outline-none placeholder:text-mute focus:border-neon/50";
  const field = (key: keyof Settings, label: string, placeholder = "") => (
    <label className="block">
      <span className="mb-1 block text-xs text-dim">{label}</span>
      <input value={s[key] as string} onChange={(e) => set({ [key]: e.target.value })} placeholder={placeholder} className={input} />
    </label>
  );

  return (
    <div className="mx-auto grid max-w-5xl grid-cols-12 gap-4 p-6 pb-20">
      <Card title="Tu marca en los informes" className="col-span-12 lg:col-span-7">
        <div className="grid grid-cols-2 gap-3">
          {field("technician", "Técnico", "Tu nombre")}
          {field("company", "Empresa o marca", "Opcional")}
          {field("phone", "Teléfono")}
          {field("email", "Correo")}
          <div className="col-span-2">{field("website", "Web o redes")}</div>
          <div className="col-span-2">{field("defaultDomain", "Dominio habitual (se propone al unir equipos)", "p. ej. empresa.local")}</div>
          <label className="col-span-2 block">
            <span className="mb-1 block text-xs text-dim">Condiciones / garantía (pie del informe)</span>
            <textarea
              value={s.conditions}
              onChange={(e) => set({ conditions: e.target.value })}
              rows={3}
              placeholder="Ej.: Garantía de 30 días sobre el trabajo realizado. No incluye daños por software de terceros."
              className={`${input} resize-y`}
            />
          </label>
        </div>
      </Card>

      <Card title="Logo" className="col-span-12 lg:col-span-5">
        <div className="flex items-center gap-4">
          <div className="grid size-24 shrink-0 place-items-center rounded-xl border border-line bg-white">
            <img src={s.logo ?? logo} alt="" className="max-h-20 max-w-20 object-contain" />
          </div>
          <div className="space-y-2 text-xs">
            <label className="flex cursor-pointer items-center gap-1.5 rounded-md border border-neon/50 px-3 py-1.5 text-neon hover:bg-neon/10">
              <ImagePlus size={13} /> Elegir imagen
              <input type="file" accept="image/png,image/jpeg,image/webp,image/svg+xml" className="hidden" onChange={(e) => pickLogo(e.target.files?.[0])} />
            </label>
            {s.logo && (
              <button onClick={() => set({ logo: null })} className="flex items-center gap-1 text-mute hover:text-bad">
                <X size={12} /> Quitar (usar el de AdminOps)
              </button>
            )}
            <p className="text-mute">PNG, JPG, WebP o SVG · máx. 500 KB. Aparece en la cabecera del informe.</p>
          </div>
        </div>
      </Card>

      <Card title="Checklist de servicio" className="col-span-12 lg:col-span-7">
        <ul className="mb-3 space-y-1">
          {s.checklist.map((item, i) => (
            <li key={i} className="group flex items-center gap-2 rounded-md px-2 py-1 hover:bg-panel-2">
              <span className="w-5 font-mono text-[11px] text-mute">{i + 1}</span>
              <input
                value={item}
                onChange={(e) => set({ checklist: s.checklist.map((x, j) => (j === i ? e.target.value : x)) })}
                className="flex-1 bg-transparent text-sm text-ink outline-none"
              />
              <div className="flex gap-1 opacity-0 group-hover:opacity-100">
                <button onClick={() => move(i, -1)} className="text-mute hover:text-ink">
                  <ArrowUp size={12} />
                </button>
                <button onClick={() => move(i, 1)} className="text-mute hover:text-ink">
                  <ArrowDown size={12} />
                </button>
                <button onClick={() => set({ checklist: s.checklist.filter((_, j) => j !== i) })} className="text-mute hover:text-bad">
                  <Trash2 size={12} />
                </button>
              </div>
            </li>
          ))}
        </ul>
        <form
          className="flex gap-2"
          onSubmit={(e) => {
            e.preventDefault();
            if (!newItem.trim()) return;
            set({ checklist: [...s.checklist, newItem.trim()] });
            setNewItem("");
          }}
        >
          <input value={newItem} onChange={(e) => setNewItem(e.target.value)} placeholder="Nuevo punto…" className={input} />
          <button type="submit" className="flex items-center gap-1 rounded-md border border-line-2 px-3 text-xs text-dim hover:text-ink">
            <Plus size={12} /> Añadir
          </button>
        </form>
        <p className="mt-2 text-[11px] text-mute">Se copia en cada sesión nueva; las sesiones en curso no cambian.</p>
      </Card>

      <Card title="Presupuestos, recibos y garantías" icon={<Receipt size={14} />} className="col-span-12">
        <div className="grid grid-cols-2 gap-3 md:grid-cols-6">
          <label className="block">
            <span className="mb-1 block text-xs text-dim">Moneda</span>
            <input value={s.currency} onChange={(e) => set({ currency: e.target.value })} placeholder="RD$" className={input} />
          </label>
          <label className="block">
            <span className="mb-1 block text-xs text-dim">Impuesto</span>
            <input value={s.taxName} onChange={(e) => set({ taxName: e.target.value })} placeholder="ITBIS" className={input} />
          </label>
          {numField("taxRate", "Porcentaje (0: sin impuesto)", 100)}
          {numField("laborWarrantyDays", "Garantía mano de obra (días)", 3650)}
          {numField("maintenanceMonths", "Mantenimiento cada (meses)", 60)}
          {numField("quoteValidityDays", "Validez presupuesto (días)", 365)}
        </div>

        <h4 className="mt-5 mb-2 text-xs font-medium text-dim">Catálogo de servicios y piezas</h4>
        <table className="w-full text-[13px]">
          <thead>
            <tr className="text-left text-xs text-mute">
              <th className="pb-1.5 font-medium">Nombre</th>
              <th className="w-32 pb-1.5 font-medium">Precio</th>
              <th className="w-14 pb-1.5 text-center font-medium">Pieza</th>
              <th className="w-28 pb-1.5 font-medium">Garantía (días)</th>
              <th className="w-8" />
            </tr>
          </thead>
          <tbody>
            {s.catalog.map((c, i) => (
              <tr key={i}>
                <td className="py-1 pr-2">
                  <input value={c.name} onChange={(e) => setItem(i, { name: e.target.value })} className={input} />
                </td>
                <td className="py-1 pr-2">
                  <input type="number" min={0} step="any" value={c.price} onChange={(e) => setItem(i, { price: Math.max(0, Number(e.target.value) || 0) })} className={input} />
                </td>
                <td className="py-1 text-center">
                  <input type="checkbox" checked={c.part} onChange={(e) => setItem(i, { part: e.target.checked })} className="size-4 accent-[var(--color-neon)]" />
                </td>
                <td className="py-1 pr-2">
                  <input
                    type="number"
                    min={0}
                    disabled={!c.part}
                    value={c.part ? c.warrantyDays : ""}
                    onChange={(e) => setItem(i, { warrantyDays: Math.max(0, Math.round(Number(e.target.value) || 0)) })}
                    className={`${input} disabled:opacity-40`}
                  />
                </td>
                <td className="py-1 text-right">
                  <button onClick={() => set({ catalog: s.catalog.filter((_, j) => j !== i) })} className="text-mute hover:text-bad" title="Quitar">
                    <Trash2 size={13} />
                  </button>
                </td>
              </tr>
            ))}
          </tbody>
        </table>
        <button
          onClick={() => set({ catalog: [...s.catalog, { name: "", price: 0, part: false, warrantyDays: 0 }] })}
          className="mt-2 flex items-center gap-1 rounded-md border border-line-2 px-2.5 py-1.5 text-xs text-dim hover:text-ink"
        >
          <Plus size={12} /> Añadir al catálogo
        </button>
        <p className="mt-2 text-[11px] text-mute">
          En la sesión o el informe se añaden con un clic. Las piezas pueden llevar su propia garantía, que aparece en el recibo y en la ficha del cliente.
        </p>
      </Card>

      <Card title="Tu firma" icon={<PenLine size={14} />} className="col-span-12 lg:col-span-5">
        <SignaturePad value={s.techSignature} onChange={(techSignature) => set({ techSignature })} height={130} />
        <p className="mt-1 text-[11px] text-mute">Aparece sobre tu nombre en todos los informes. Opcional.</p>
      </Card>

      <Appearance />

      <Card title="Acerca de" icon={<BadgeCheck size={14} />} className="col-span-12 lg:col-span-5">
        <div className="flex items-center gap-3">
          <img src={logo} alt="" className="size-12" />
          <div>
            <div className="font-semibold">AdminOps v{appInfo?.version}</div>
            <div className="text-sm text-dim">
              by <span className="font-semibold text-neon">David Bonilla</span>
            </div>
          </div>
        </div>
        <div className="mt-4 flex flex-wrap gap-2 text-xs">
          {(
            [
              ["data", "Carpeta de datos"],
              ["reports", "Carpeta de informes"],
              ["logs", "Registros"],
            ] as const
          ).map(([kind, label]) => (
            <button
              key={kind}
              onClick={() => appApi.openFolder(kind).catch((e) => toast("error", String(e)))}
              className="rounded-md border border-line-2 px-2.5 py-1 text-dim hover:border-neon/40 hover:text-neon"
            >
              {label} →
            </button>
          ))}
        </div>
        <div className="mt-4 border-t border-line/60 pt-3">
          <button
            onClick={() =>
              appApi
                .supportPackage()
                .then(() => toast("ok", "Paquete de soporte creado: se abrió su carpeta."))
                .catch((e) => toast("error", String(e)))
            }
            className="rounded-md border border-neon/40 px-3 py-1.5 text-xs text-neon hover:bg-neon/10"
          >
            Crear paquete de soporte
          </button>
          <p className="mt-1.5 text-[11px] text-mute">
            Un .zip con el registro de actividad, el último diagnóstico y la versión, para enviarlo si algo falla. Puede contener el nombre del
            equipo y del usuario: revísalo antes de compartirlo.
          </p>
        </div>
      </Card>

      {dirty && (
        <div className="fixed right-0 bottom-0 left-60 z-30 border-t border-line bg-panel px-6 py-3">
          <div className="mx-auto flex max-w-5xl items-center justify-between">
            <span className="text-sm text-warn">Cambios sin guardar</span>
            <button onClick={save} className="flex items-center gap-1.5 rounded-md border border-neon/50 bg-neon/10 px-4 py-1.5 text-sm font-medium text-neon hover:bg-neon/20">
              <Save size={14} /> Guardar ajustes
            </button>
          </div>
        </div>
      )}
    </div>
  );
}

/** Tema oscuro o claro (se aplica al momento). */
function Appearance() {
  const [theme, set] = useState<Theme>(getTheme);
  const pick = (t: Theme) => {
    setTheme(t);
    set(t);
  };
  const option = (t: Theme, title: string, sub: string, bg: string, bar: string) => (
    <button
      onClick={() => pick(t)}
      aria-pressed={theme === t}
      className={`flex flex-1 items-center gap-3 rounded-lg border p-3 text-left transition-colors ${theme === t ? "border-neon" : "border-line hover:border-line-2"}`}
    >
      <span className="flex h-10 w-14 shrink-0 flex-col justify-end gap-1 rounded-md border border-line-2 p-1.5" style={{ background: bg }}>
        <span className="h-1 w-8 rounded-full" style={{ background: bar }} />
        <span className="h-1 w-5 rounded-full" style={{ background: bar, opacity: 0.5 }} />
      </span>
      <span>
        <span className="block text-sm font-medium text-ink">{title}</span>
        <span className="block text-xs text-mute">{sub}</span>
      </span>
    </button>
  );
  return (
    <Card title="Apariencia" className="col-span-12 lg:col-span-7">
      <div className="flex gap-3">
        {option("dark", "Oscuro", "Menos brillo en talleres y de noche", "#111315", "#a5acb5")}
        {option("light", "Claro", "Más legible con mucha luz y en oficinas", "#f6f6f4", "#4b5058")}
      </div>
    </Card>
  );
}
