// Ajustes → Alertas → Reglas propias: «si pasa esto, avísame». El técnico elige qué mirar, a partir de qué
// valor y durante cuánto tiempo; AdminOps lo vigila en segundo plano y avisa en la campana y en «Hoy».
import { Bell, Loader2, Plus, Trash2 } from "lucide-react";
import { useEffect, useState } from "react";
import { useToast } from "../../components/feedback";
import { Button, Card, inputClass, Toggle } from "../../components/ui";
import { alertRulesApi, type AlertRule } from "../../lib/api";

const METRICS: { id: string; label: string; unit: string; target?: string; hint: string }[] = [
  { id: "disk_free", label: "Espacio libre de un disco", unit: "%", target: "Unidad (C, D…) o vacío para la de Windows", hint: "Salta cuando queda menos de este porcentaje libre." },
  { id: "cpu", label: "Procesador ocupado", unit: "%", hint: "Salta cuando está por encima durante el tiempo indicado." },
  { id: "ram", label: "Memoria en uso", unit: "%", hint: "Salta cuando está por encima durante el tiempo indicado." },
  { id: "service_running", label: "Un servicio parado", unit: "", target: "Nombre del servicio (Spooler, W32Time…)", hint: "Salta cuando el servicio no está en marcha." },
  { id: "backup_age", label: "Copia de seguridad atrasada", unit: "días", target: "Nombre de la copia o vacío para todas", hint: "Salta si la copia más atrasada supera estos días. Usa Datos del equipo → Copias de seguridad." },
  { id: "toner", label: "Tóner de una impresora", unit: "%", target: "Nombre de la impresora", hint: "Salta cuando queda menos de este porcentaje. Solo impresoras de red que contestan por SNMP." },
  { id: "disk_score", label: "Salud de un disco", unit: "puntos", hint: "Salta si un disco baja de esta nota (0-100) la última vez que se miraron los discos." },
];

const blank = (): AlertRule => ({ id: "", name: "", enabled: true, metric: "disk_free", target: "", op: "lt", value: 10, forMinutes: 0, level: "warn" });

export function AlertRules() {
  const toast = useToast();
  const [rules, setRules] = useState<AlertRule[] | null>(null);
  const [saved, setSaved] = useState("");
  const [templates, setTemplates] = useState<{ name: string; rules: AlertRule[] }[]>([]);
  const [busy, setBusy] = useState(false);
  const [now, setNow] = useState<Record<number, string>>({});

  useEffect(() => {
    void alertRulesApi
      .get()
      .then((r) => (setRules(r), setSaved(JSON.stringify(r))))
      .catch((e) => toast("error", String(e)));
    void alertRulesApi.templates().then(setTemplates).catch(() => undefined);
  }, [toast]);

  if (!rules) return null;
  const dirty = JSON.stringify(rules) !== saved;
  const set = (i: number, patch: Partial<AlertRule>) => setRules(rules.map((r, j) => (j === i ? { ...r, ...patch } : r)));

  const save = async () => {
    setBusy(true);
    try {
      const out = await alertRulesApi.save(rules);
      setRules(out);
      setSaved(JSON.stringify(out));
      toast("ok", "Reglas guardadas.");
    } catch (e) {
      toast("error", String(e));
    } finally {
      setBusy(false);
    }
  };
  const peek = async (i: number) => {
    const m = METRICS.find((x) => x.id === rules[i].metric);
    const v = await alertRulesApi.reading(rules[i]).catch(() => null);
    setNow((n) => ({ ...n, [i]: v === null ? "No se pudo leer ahora." : rules[i].metric === "service_running" ? (v >= 1 ? "Ahora: en marcha." : "Ahora: parado.") : `Ahora: ${v.toFixed(0)} ${m?.unit ?? ""}`.trim() }));
  };

  return (
    <Card title="Reglas propias" icon={<Bell size={14} />}>
      <div className="space-y-3 p-4">
        <p className="text-xs leading-relaxed text-dim">
          «Si pasa esto, avísame.» AdminOps mira las reglas cada minuto con la aplicación abierta (aunque esté minimizada) y avisa <b className="text-ink">una vez</b> cuando se cumplen; no vuelve a avisar hasta que dejen de cumplirse y vuelvan a ocurrir. Los avisos van a la
          campana, a «Hoy» y al diario.
        </p>
        {rules.length === 0 && <p className="rounded-md border border-line bg-panel-2 px-3 py-2 text-xs text-mute">Todavía no hay reglas. Empieza con una plantilla o añade la tuya.</p>}
        <ul className="space-y-3">
          {rules.map((r, i) => {
            const m = METRICS.find((x) => x.id === r.metric) ?? METRICS[0];
            return (
              <li key={r.id || i} className="space-y-2 rounded-lg border border-line bg-panel-2 p-3">
                <div className="flex items-center gap-3">
                  <Toggle checked={r.enabled} onChange={(v) => set(i, { enabled: v })} label={`Regla ${r.name || i + 1}`} />
                  <input value={r.name} onChange={(e) => set(i, { name: e.target.value })} placeholder="Nombre de la regla (opcional)" className={`${inputClass} min-w-0 flex-1`} aria-label="Nombre de la regla" />
                  <button type="button" onClick={() => setRules(rules.filter((_, j) => j !== i))} className="text-mute hover:text-bad" aria-label="Quitar la regla" title="Quitar la regla">
                    <Trash2 size={14} />
                  </button>
                </div>
                <div className="flex flex-wrap items-center gap-2 text-xs text-dim">
                  <span>Si</span>
                  <select value={r.metric} onChange={(e) => set(i, { metric: e.target.value, ...(e.target.value === "service_running" ? { op: "lt", value: 1 } : {}) })} className={`${inputClass} w-auto`} aria-label="Qué mirar">
                    {METRICS.map((x) => (
                      <option key={x.id} value={x.id}>
                        {x.label}
                      </option>
                    ))}
                  </select>
                  {m.target && <input value={r.target} onChange={(e) => set(i, { target: e.target.value })} placeholder={m.target} className={`${inputClass} w-56`} aria-label="Sobre qué" />}
                  {r.metric !== "service_running" && (
                    <>
                      <select value={r.op} onChange={(e) => set(i, { op: e.target.value as "lt" | "gt" })} className={`${inputClass} w-auto`} aria-label="Comparación">
                        <option value="lt">es menor que</option>
                        <option value="gt">es mayor que</option>
                      </select>
                      <input type="number" value={r.value} onChange={(e) => set(i, { value: Number(e.target.value) })} className={`${inputClass} w-20 text-right`} aria-label="Valor" />
                      <span>{m.unit}</span>
                    </>
                  )}
                  <span>durante</span>
                  <input type="number" min={0} value={r.forMinutes} onChange={(e) => set(i, { forMinutes: Math.max(0, Number(e.target.value)) })} className={`${inputClass} w-16 text-right`} aria-label="Minutos" />
                  <span>min, avisar como</span>
                  <select value={r.level} onChange={(e) => set(i, { level: e.target.value as "warn" | "bad" })} className={`${inputClass} w-auto`} aria-label="Gravedad">
                    <option value="warn">aviso</option>
                    <option value="bad">grave</option>
                  </select>
                </div>
                <div className="flex flex-wrap items-center gap-3 text-[11px] text-mute">
                  <span className="min-w-0 flex-1">{m.hint}</span>
                  <button type="button" className="text-neon hover:underline" onClick={() => void peek(i)}>
                    Ver valor de ahora
                  </button>
                  {now[i] && <span className="text-dim">{now[i]}</span>}
                </div>
              </li>
            );
          })}
        </ul>
        <div className="flex flex-wrap items-center gap-2">
          <Button kind="secondary" size="sm" onClick={() => setRules([...rules, blank()])}>
            <Plus size={13} /> Añadir regla
          </Button>
          {templates.length > 0 && (
            <select
              value=""
              onChange={(e) => {
                const t = templates.find((x) => x.name === e.target.value);
                if (t) setRules([...rules.filter((r) => !t.rules.some((n) => n.id === r.id)), ...t.rules]);
              }}
              className={`${inputClass} w-auto`}
              aria-label="Plantillas"
            >
              <option value="">Empezar con una plantilla…</option>
              {templates.map((t) => (
                <option key={t.name} value={t.name}>
                  {t.name}
                </option>
              ))}
            </select>
          )}
          <span className="flex-1" />
          <Button onClick={() => void save()} disabled={!dirty || busy}>
            {busy && <Loader2 size={13} className="animate-spin" />} Guardar las reglas
          </Button>
        </div>
      </div>
    </Card>
  );
}
