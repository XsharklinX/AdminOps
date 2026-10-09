// Ajustes → Informes: la plantilla de informe propia. El técnico elige qué
// secciones lleva el PDF y en qué orden; la cabecera va siempre arriba y las
// firmas y condiciones, abajo.
import { ArrowDown, ArrowUp, LayoutList } from "lucide-react";
import { Card, inputClass, Toggle } from "../../components/ui";
import type { ReportLayout, Settings } from "../../lib/api";

/** Las secciones que se pueden quitar o mover, en su orden de fábrica (el mismo que en workflow.rs). */
export const REPORT_SECTIONS: { id: string; label: string; hint: string }[] = [
  { id: "summary", label: "Resumen y estado por áreas", hint: "La nota del equipo, lo más importante y cómo está cada área." },
  { id: "problem", label: "Motivo de la visita", hint: "Lo que contó el cliente." },
  { id: "work", label: "Trabajo realizado", hint: "Lo hecho con AdminOps y la lista de comprobación." },
  { id: "findings", label: "Problemas detectados", hint: "Lo resuelto en la visita y lo que queda pendiente." },
  { id: "disks", label: "Estado de los discos", hint: "Semáforo de cada disco, qué significa y qué hacer, con el detalle técnico en la plantilla técnica." },
  { id: "backups", label: "Copias de seguridad", hint: "Si las copias del cliente están al día, en otro disco y se pueden restaurar." },
  { id: "comparison", label: "Antes y después", hint: "La comparación con el diagnóstico inicial, si lo hay." },
  { id: "recommendations", label: "Recomendaciones", hint: "Lo que aconsejas hacer." },
  { id: "billing", label: "Presupuesto o recibo, y garantías", hint: "El cobro, las garantías y el próximo mantenimiento." },
  { id: "machine", label: "El equipo", hint: "Sus datos: en resumen o, con el detalle técnico, pieza a pieza." },
  { id: "speed", label: "Conexión a Internet", hint: "La última prueba de velocidad de la visita." },
  { id: "notes", label: "Observaciones del técnico", hint: "Lo que encontraste y lo que hiciste." },
];

export const DEFAULT_LAYOUT: ReportLayout = { name: "Mi plantilla", technical: false, sections: REPORT_SECTIONS.map((s) => s.id) };

/** La plantilla guardada, o la de fábrica si los ajustes son de una versión anterior. */
export const layoutOf = (s: Pick<Settings, "reportLayout"> | null | undefined): ReportLayout => s?.reportLayout ?? DEFAULT_LAYOUT;

/** Sube o baja una sección dentro de las que lleva la plantilla. */
export function moveSection(sections: string[], id: string, delta: -1 | 1): string[] {
  const i = sections.indexOf(id);
  const j = i + delta;
  if (i < 0 || j < 0 || j >= sections.length) return sections;
  const next = [...sections];
  [next[i], next[j]] = [next[j], next[i]];
  return next;
}

export function ReportLayoutEditor({ s, set }: { s: Settings; set: (patch: Partial<Settings>) => void }) {
  const layout = layoutOf(s);
  const update = (patch: Partial<ReportLayout>) => set({ reportLayout: { ...layout, ...patch } });
  const on = layout.sections.filter((id) => REPORT_SECTIONS.some((x) => x.id === id));
  const off = REPORT_SECTIONS.filter((x) => !on.includes(x.id));
  const label = (id: string) => REPORT_SECTIONS.find((x) => x.id === id)!;

  return (
    <Card title="Tu plantilla de informe" icon={<LayoutList size={14} />} className="col-span-12">
      <p className="mb-3 text-xs text-dim">
        Además de las dos plantillas de fábrica (para el cliente y técnica), puedes tener la tuya: qué secciones lleva el PDF y en qué orden. Al generar un informe aparece como tercera opción. La
        cabecera va siempre arriba, y las firmas y tus condiciones, abajo.
      </p>
      <div className="mb-4 flex flex-wrap items-end gap-4">
        <label className="block">
          <span className="mb-1 block text-xs text-dim">Nombre</span>
          <input value={layout.name} onChange={(e) => update({ name: e.target.value.slice(0, 40) })} placeholder="Mi plantilla" className={`${inputClass} w-64 max-w-full`} />
        </label>
        <label className="flex items-center gap-2 pb-2 text-sm text-dim">
          <Toggle checked={layout.technical} onChange={(v) => update({ technical: v })} />
          Con el detalle técnico (hardware, discos, drivers, estabilidad)
        </label>
      </div>

      <div className="grid grid-cols-1 gap-4 lg:grid-cols-2">
        <div>
          <div className="mb-1.5 text-[11px] font-medium tracking-wide text-mute uppercase">En el informe, en este orden · {on.length}</div>
          {on.length === 0 ? (
            <p className="rounded-lg border border-dashed border-line px-3 py-4 text-center text-xs text-mute">Sin secciones, el informe lleva solo la cabecera, las firmas y tus condiciones.</p>
          ) : (
            <ol className="divide-y divide-line/60 rounded-lg border border-line">
              {on.map((id, i) => (
                <li key={id} className="flex items-center gap-2 px-3 py-2">
                  <span className="w-5 shrink-0 font-mono text-[11px] text-mute">{i + 1}</span>
                  <span className="min-w-0 flex-1">
                    <span className="block text-sm text-ink">{label(id).label}</span>
                    <span className="block truncate text-[11px] text-mute">{label(id).hint}</span>
                  </span>
                  <button onClick={() => update({ sections: moveSection(on, id, -1) })} disabled={i === 0} className="rounded p-1 text-mute hover:bg-panel-2 hover:text-ink disabled:opacity-25" title="Subir">
                    <ArrowUp size={13} />
                  </button>
                  <button onClick={() => update({ sections: moveSection(on, id, 1) })} disabled={i === on.length - 1} className="rounded p-1 text-mute hover:bg-panel-2 hover:text-ink disabled:opacity-25" title="Bajar">
                    <ArrowDown size={13} />
                  </button>
                  <button onClick={() => update({ sections: on.filter((x) => x !== id) })} className="rounded px-1.5 py-0.5 text-[11px] text-mute hover:bg-panel-2 hover:text-bad">
                    Quitar
                  </button>
                </li>
              ))}
            </ol>
          )}
        </div>
        <div>
          <div className="mb-1.5 text-[11px] font-medium tracking-wide text-mute uppercase">Fuera del informe · {off.length}</div>
          {off.length === 0 ? (
            <p className="rounded-lg border border-dashed border-line px-3 py-4 text-center text-xs text-mute">Lleva todas las secciones.</p>
          ) : (
            <ul className="divide-y divide-line/60 rounded-lg border border-line">
              {off.map((x) => (
                <li key={x.id} className="flex items-center gap-2 px-3 py-2">
                  <span className="min-w-0 flex-1">
                    <span className="block text-sm text-dim">{x.label}</span>
                    <span className="block truncate text-[11px] text-mute">{x.hint}</span>
                  </span>
                  <button onClick={() => update({ sections: [...on, x.id] })} className="rounded px-1.5 py-0.5 text-[11px] text-neon hover:bg-neon/10">
                    Añadir
                  </button>
                </li>
              ))}
            </ul>
          )}
          <button onClick={() => update({ sections: DEFAULT_LAYOUT.sections })} className="mt-2 text-xs text-mute hover:text-ink">
            Volver a todas, en el orden de fábrica
          </button>
        </div>
      </div>
      <p className="mt-3 text-[11px] text-mute">Una sección sin contenido (sin recomendaciones escritas, sin prueba de velocidad) no sale aunque esté en la lista. Los cambios se guardan solos.</p>
    </Card>
  );
}
