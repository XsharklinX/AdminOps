import { ArrowDown, ArrowUp, ClipboardList, Plus, Trash2, Wand2 } from "lucide-react";
import { useState } from "react";
import { Card } from "../../components/ui";
import type { Settings, VisitType } from "../../lib/api";
import { AUTO_TASKS } from "../../lib/visits";

/** Tipos de visita: cada uno con su checklist, y puntos que se marcan solos. */
export function VisitTypesEditor({ s, set }: { s: Settings; set: (p: Partial<Settings>) => void }) {
  const [open, setOpen] = useState(0);
  const types = s.visitTypes;
  const setTypes = (visitTypes: VisitType[]) => set({ visitTypes });
  const setType = (i: number, t: VisitType) => setTypes(types.map((x, j) => (j === i ? t : x)));
  const t = types[open];

  const moveItem = (i: number, d: number) => {
    if (!t) return;
    const items = [...t.items];
    const j = i + d;
    if (j < 0 || j >= items.length) return;
    [items[i], items[j]] = [items[j], items[i]];
    setType(open, { ...t, items });
  };

  return (
    <Card title="Tipos de visita" icon={<ClipboardList size={14} />} className="col-span-12">
      <p className="mb-3 text-xs text-dim">
        Al iniciar una sesión eliges el tipo y se carga su checklist. Los puntos con <Wand2 size={11} className="inline text-neon" /> se marcan solos cuando haces esa
        tarea con AdminOps.
      </p>
      <div className="grid gap-4 md:grid-cols-[minmax(150px,220px)_1fr]">
        <div className="space-y-1">
          {types.map((v, i) => (
            <div key={i} className={`group flex items-center gap-1 rounded-md px-2 py-1.5 ${open === i ? "bg-neon/10" : "hover:bg-panel-2"}`}>
              <input
                value={v.name}
                onFocus={() => setOpen(i)}
                onChange={(e) => setType(i, { ...v, name: e.target.value })}
                className="min-w-0 flex-1 bg-transparent text-sm text-ink outline-none"
              />
              <span className="text-[11px] text-mute">{v.items.length}</span>
              <button
                onClick={() => {
                  setTypes(types.filter((_, j) => j !== i));
                  setOpen(0);
                }}
                className="text-mute opacity-0 group-hover:opacity-100 hover:text-bad"
                title="Borrar tipo"
              >
                <Trash2 size={12} />
              </button>
            </div>
          ))}
          <button
            onClick={() => {
              setTypes([...types, { name: "Nuevo tipo", items: [] }]);
              setOpen(types.length);
            }}
            className="flex w-full items-center gap-1 rounded-md border border-dashed border-line px-2 py-1.5 text-xs text-neon hover:border-neon/50"
          >
            <Plus size={12} /> Tipo de visita
          </button>
        </div>
        {t ? (
          <div>
            <ul className="space-y-1">
              {t.items.map((item, i) => (
                <li key={i} className="group flex items-center gap-2 rounded-md px-2 py-1 hover:bg-panel-2">
                  <span className="w-5 font-mono text-[11px] text-mute">{i + 1}</span>
                  <input
                    value={item.text}
                    onChange={(e) => setType(open, { ...t, items: t.items.map((x, j) => (j === i ? { ...x, text: e.target.value } : x)) })}
                    className="min-w-0 flex-1 bg-transparent text-sm text-ink outline-none"
                  />
                  <select
                    value={item.auto}
                    onChange={(e) => setType(open, { ...t, items: t.items.map((x, j) => (j === i ? { ...x, auto: e.target.value } : x)) })}
                    className={`w-44 rounded border border-line bg-void/60 px-1 py-0.5 text-[11px] ${item.auto ? "text-neon" : "text-mute"}`}
                    title="Tarea que lo marca sola"
                  >
                    {AUTO_TASKS.map((a) => (
                      <option key={a.id} value={a.id}>
                        {a.id ? `Auto: ${a.label}` : a.label}
                      </option>
                    ))}
                  </select>
                  <span className="flex gap-1 opacity-0 group-hover:opacity-100">
                    <button onClick={() => moveItem(i, -1)} className="text-mute hover:text-ink">
                      <ArrowUp size={12} />
                    </button>
                    <button onClick={() => moveItem(i, 1)} className="text-mute hover:text-ink">
                      <ArrowDown size={12} />
                    </button>
                    <button onClick={() => setType(open, { ...t, items: t.items.filter((_, j) => j !== i) })} className="text-mute hover:text-bad">
                      <Trash2 size={12} />
                    </button>
                  </span>
                </li>
              ))}
            </ul>
            <button onClick={() => setType(open, { ...t, items: [...t.items, { text: "", auto: "" }] })} className="mt-2 flex items-center gap-1 text-xs text-neon hover:underline">
              <Plus size={11} /> Punto
            </button>
          </div>
        ) : (
          <p className="text-sm text-mute">Crea un tipo de visita.</p>
        )}
      </div>
    </Card>
  );
}
