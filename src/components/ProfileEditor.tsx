import { Save, X } from "lucide-react";
import { useEffect, useMemo, useState } from "react";
import { tweaksApi, workApi, type ProfileDef, type TweakView } from "../lib/api";

const CATEGORY: Record<string, string> = {
  privacy: "Privacidad",
  performance: "Rendimiento",
  services: "Servicios",
  cleanup: "Limpieza",
  repair: "Reparaciones",
};

export const PROFILE_ICONS = ["layers", "briefcase", "gamepad", "turtle", "eye-off", "zap", "wrench", "shield", "graduation-cap", "house"];

/** Crear o editar un perfil propio eligiendo ajustes y tareas del catálogo. */
export function ProfileEditor({
  initial,
  icons,
  onClose,
  onSaved,
}: {
  initial: ProfileDef | null;
  icons: Record<string, React.ComponentType<{ size?: number }>>;
  onClose: () => void;
  onSaved: (p: ProfileDef) => void;
}) {
  const [p, setP] = useState<ProfileDef>(initial ?? { id: "", name: "", icon: "layers", description: "", items: [], custom: true });
  const [catalog, setCatalog] = useState<TweakView[]>([]);
  const [error, setError] = useState<string | null>(null);
  const [saving, setSaving] = useState(false);

  useEffect(() => {
    tweaksApi.list().then(setCatalog);
  }, []);

  const groups = useMemo(() => {
    const g: Record<string, TweakView[]> = {};
    for (const t of catalog) (g[t.category] ??= []).push(t);
    return Object.entries(g);
  }, [catalog]);

  const toggle = (id: string) => setP((x) => ({ ...x, items: x.items.includes(id) ? x.items.filter((i) => i !== id) : [...x.items, id] }));

  const save = async () => {
    setSaving(true);
    setError(null);
    try {
      onSaved(await workApi.saveProfile(p));
    } catch (e) {
      setError(String(e));
    } finally {
      setSaving(false);
    }
  };

  const input = "w-full rounded-md border border-line bg-void/60 px-3 py-2 text-sm text-ink outline-none placeholder:text-mute focus:border-neon/50";

  return (
    <div className="fixed inset-0 z-40 grid place-items-center bg-black/55" onClick={onClose}>
      <div className="flex max-h-[88vh] w-[760px] flex-col rounded-xl border border-line-2 bg-panel shadow-2xl" onClick={(e) => e.stopPropagation()}>
        <div className="flex items-center justify-between border-b border-line px-5 py-3">
          <h3 className="font-semibold">{initial ? "Editar perfil" : "Nuevo perfil"}</h3>
          <button onClick={onClose} className="text-mute hover:text-ink">
            <X size={16} />
          </button>
        </div>
        <div className="grid grid-cols-5 gap-3 px-5 pt-4">
          <input value={p.name} onChange={(e) => setP({ ...p, name: e.target.value })} placeholder="Nombre del perfil" className={`${input} col-span-3`} />
          <div className="col-span-2 flex flex-wrap gap-1">
            {PROFILE_ICONS.filter((n) => icons[n]).map((n) => {
              const I = icons[n];
              return (
                <button
                  key={n}
                  onClick={() => setP({ ...p, icon: n })}
                  className={`grid size-8 place-items-center rounded-md border ${p.icon === n ? "border-neon/60 bg-neon/10 text-neon" : "border-line text-mute hover:text-ink"}`}
                >
                  <I size={15} />
                </button>
              );
            })}
          </div>
          <input
            value={p.description}
            onChange={(e) => setP({ ...p, description: e.target.value })}
            placeholder="Descripción (para qué equipos es)"
            className={`${input} col-span-5`}
          />
        </div>
        <div className="min-h-0 flex-1 overflow-y-auto px-5 py-3">
          {groups.map(([cat, list]) => (
            <div key={cat} className="mb-3">
              <div className="mb-1 text-[11px] font-semibold text-mute">{CATEGORY[cat] ?? cat}</div>
              <div className="grid grid-cols-2 gap-x-4">
                {list.map((t) => (
                  <label key={t.id} className="flex cursor-pointer items-center gap-2 rounded px-1.5 py-1 text-sm hover:bg-panel-2">
                    <input type="checkbox" checked={p.items.includes(t.id)} onChange={() => toggle(t.id)} className="accent-[var(--color-neon)]" />
                    <span className="truncate text-dim">{t.name}</span>
                    {t.kind === "action" && <span className="text-[11px] text-mute">tarea</span>}
                    {t.risk !== "low" && <span className={`text-[11px] ${t.risk === "high" ? "text-bad" : "text-warn"}`}>{t.risk === "high" ? "alto" : "medio"}</span>}
                  </label>
                ))}
              </div>
            </div>
          ))}
        </div>
        <div className="flex items-center gap-3 border-t border-line px-5 py-3">
          <span className="text-xs text-dim">{p.items.length} elementos</span>
          {error && <span className="min-w-0 flex-1 truncate text-xs text-bad">{error}</span>}
          <button
            onClick={save}
            disabled={saving}
            className="ml-auto flex items-center gap-1.5 rounded-md border border-neon/50 bg-neon/10 px-4 py-1.5 text-sm text-neon hover:bg-neon/20 disabled:opacity-50"
          >
            <Save size={13} /> Guardar perfil
          </button>
        </div>
      </div>
    </div>
  );
}
