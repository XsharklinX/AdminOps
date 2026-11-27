import { AlarmClock, BatteryMedium, Briefcase, Laptop, Moon, Plug, Presentation, Server, SlidersHorizontal } from "lucide-react";
import { useCallback, useEffect, useState, type ReactNode } from "react";
import { powerApi, type PowerProfile, type PowerSide, type PowerState } from "../lib/api/power";
import { logQuietly } from "../lib/api/core";
import { LID_OPTIONS, lidSentence, PROFILE_TEXT, sideSentence } from "../lib/power";
import { useToast } from "./feedback";
import { Button, Card } from "./ui";

const ICON: Record<string, ReactNode> = {
  office: <Briefcase size={15} />,
  present: <Presentation size={15} />,
  always: <Server size={15} />,
  dock: <Laptop size={15} />,
};

/**
 * Suspensión y pantalla según el uso (power.rs): cómo está ahora en una frase, perfiles de un clic,
 * a medida y quién lo despierta de noche.
 */
export function PowerCard({ isAdmin }: { isAdmin: boolean }) {
  const [st, setSt] = useState<PowerState | null>(null);
  const [profiles, setProfiles] = useState<PowerProfile[]>([]);
  const [error, setError] = useState("");
  const [busy, setBusy] = useState<string | null>(null);
  const [custom, setCustom] = useState<{ ac: PowerSide; dc: PowerSide } | null>(null);
  const [wake, setWake] = useState<string | null>(null);
  const toast = useToast();

  const load = useCallback(() => {
    powerApi
      .state()
      .then((s) => {
        setSt(s);
        setError("");
      })
      .catch((e) => setError(String(e)));
    powerApi.profiles().then(setProfiles).catch(logQuietly("PowerCard"));
  }, []);
  useEffect(load, [load]);

  if (error) return <Card title="Energía" icon={<Moon size={14} />} className="mb-4"><p className="text-sm text-bad">{error}</p></Card>;
  if (!st) return null;

  const apply = async (id: string, ac: PowerSide, dc: PowerSide) => {
    setBusy(id);
    try {
      const next = await powerApi.set(ac, dc);
      setSt(next);
      toast("ok", id === "custom" ? "Energía cambiada." : `Perfil «${PROFILE_TEXT[id]?.title}» puesto.`);
      if (id === "custom") setCustom(null);
    } catch (e) {
      toast("error", String(e));
    } finally {
      setBusy(null);
    }
  };
  const disabled = !isAdmin || busy !== null;
  const shown = profiles.filter((p) => st.lidPresent || p.id !== "dock");
  const tile = "flex min-w-0 flex-col items-start gap-0.5 rounded-lg border px-3 py-2.5 text-left transition-colors disabled:opacity-50";

  return (
    <Card title="Energía" icon={<Moon size={14} />} className="mb-4" right={st.profile ? <span className="text-[11px] text-ok">Perfil: {PROFILE_TEXT[st.profile]?.title}</span> : undefined}>
      <div className="mb-3 space-y-1 rounded-lg border border-line bg-void/40 px-3 py-2.5 text-sm text-dim">
        <div className="text-[11px] font-medium uppercase tracking-wide text-mute">Así está ahora</div>
        <p>
          <Plug size={13} className="mr-1.5 inline text-mute" aria-hidden />
          <b className="font-medium text-ink">Enchufado:</b> {sideSentence(st.ac)}
        </p>
        {st.battery && (
          <p>
            <BatteryMedium size={13} className="mr-1.5 inline text-mute" aria-hidden />
            <b className="font-medium text-ink">Con batería:</b> {sideSentence(st.dc)}
          </p>
        )}
        {st.lidPresent && (
          <p>
            <Laptop size={13} className="mr-1.5 inline text-mute" aria-hidden />
            <b className="font-medium text-ink">Al cerrar la tapa:</b> {lidSentence(st)}
          </p>
        )}
      </div>
      <div className="grid grid-cols-2 gap-2 lg:grid-cols-3">
        {shown.map((p) => {
          const on = st.profile === p.id;
          return (
            <button
              key={p.id}
              type="button"
              disabled={disabled}
              onClick={() => void apply(p.id, p.ac, p.dc)}
              className={`${tile} ${on ? "border-ok/50 bg-ok/5" : "border-line bg-panel-2/40 hover:border-neon/50"}`}
              title={isAdmin ? undefined : "Hace falta abrir AdminOps como administrador"}
            >
              <span className="flex items-center gap-1.5 text-sm font-medium text-ink">
                <span className={on ? "text-ok" : "text-neon"}>{ICON[p.id]}</span>
                {PROFILE_TEXT[p.id]?.title}
                {busy === p.id && <span className="text-[11px] font-normal text-mute">poniendo…</span>}
                {on && <span className="text-[11px] font-normal text-ok">puesto</span>}
              </span>
              <span className="text-[12px] text-mute">{PROFILE_TEXT[p.id]?.hint}</span>
            </button>
          );
        })}
        <button type="button" disabled={disabled} onClick={() => setCustom(custom ? null : { ac: st.ac, dc: st.dc })} className={`${tile} ${custom ? "border-neon/60" : "border-line bg-panel-2/40 hover:border-neon/50"}`}>
          <span className="flex items-center gap-1.5 text-sm font-medium text-ink">
            <SlidersHorizontal size={15} className="text-neon" />A tu medida
          </span>
          <span className="text-[12px] text-mute">minutos y tapa, uno a uno</span>
        </button>
        <button
          type="button"
          disabled={wake === "…"}
          onClick={() => {
            if (wake && wake !== "…") return setWake(null);
            setWake("…");
            powerApi
              .wake()
              .then(setWake)
              .catch((e) => setWake(String(e)));
          }}
          className={`${tile} border-line bg-panel-2/40 hover:border-neon/50`}
        >
          <span className="flex items-center gap-1.5 text-sm font-medium text-ink">
            <AlarmClock size={15} className="text-neon" />
            Despertar
          </span>
          <span className="text-[12px] text-mute">quién lo despierta de noche</span>
        </button>
      </div>
      {custom && <CustomEditor st={st} value={custom} onChange={setCustom} busy={busy === "custom"} onApply={() => void apply("custom", custom.ac, custom.dc)} />}
      {wake && <pre className="mt-3 max-h-64 overflow-auto whitespace-pre-wrap rounded-lg border border-line bg-void/60 p-3 font-mono text-[11.5px] text-dim">{wake === "…" ? "Preguntando a Windows…" : wake}</pre>}
      {!isAdmin && <p className="mt-2 text-xs text-warn">Para cambiar la energía hay que abrir AdminOps como administrador.</p>}
    </Card>
  );
}

const STEPS = [0, 1, 2, 3, 5, 10, 15, 20, 30, 45, 60, 120, 180, 300];

function MinSelect({ value, onChange, label }: { value: number; onChange: (n: number) => void; label: string }) {
  const opts = STEPS.includes(value) ? STEPS : [...STEPS, value].sort((a, b) => a - b);
  return (
    <select aria-label={label} value={value} onChange={(e) => onChange(Number(e.target.value))} className="rounded border border-line bg-void/60 px-1.5 py-1 text-xs text-ink outline-none">
      {opts.map((n) => (
        <option key={n} value={n}>
          {n === 0 ? "nunca" : n < 60 ? `${n} min` : n % 60 === 0 ? `${n / 60} h` : `${n} min`}
        </option>
      ))}
    </select>
  );
}

function CustomEditor({ st, value, onChange, busy, onApply }: { st: PowerState; value: { ac: PowerSide; dc: PowerSide }; onChange: (v: { ac: PowerSide; dc: PowerSide }) => void; busy: boolean; onApply: () => void }) {
  const sides: ["ac" | "dc", string][] = st.battery ? [["ac", "Enchufado"], ["dc", "Con batería"]] : [["ac", "Enchufado"]];
  const set = (k: "ac" | "dc", patch: Partial<PowerSide>) => {
    // Sin batería, lo de «con batería» sigue a lo de enchufado (Windows lo guarda igual).
    const next = { ...value, [k]: { ...value[k], ...patch } };
    if (!st.battery) next.dc = { ...next.ac };
    onChange(next);
  };
  return (
    <div className="mt-3 space-y-2 rounded-lg border border-line bg-void/40 p-3 text-sm text-dim">
      {sides.map(([k, label]) => (
        <div key={k} className="flex flex-wrap items-center gap-2">
          <span className="w-24 font-medium text-ink">{label}</span>
          pantalla
          <MinSelect label={`${label}: pantalla`} value={value[k].screenMin} onChange={(n) => set(k, { screenMin: n })} />
          suspender
          <MinSelect label={`${label}: suspender`} value={value[k].sleepMin} onChange={(n) => set(k, { sleepMin: n })} />
          {st.lidPresent && (
            <>
              tapa
              <select aria-label={`${label}: tapa`} value={value[k].lid} onChange={(e) => set(k, { lid: Number(e.target.value) })} className="rounded border border-line bg-void/60 px-1.5 py-1 text-xs text-ink outline-none">
                {LID_OPTIONS.map((l, i) => (
                  <option key={l} value={i}>
                    {l}
                  </option>
                ))}
              </select>
            </>
          )}
        </div>
      ))}
      <div className="pt-1">
        <Button size="sm" disabled={busy} onClick={onApply}>
          {busy ? "Poniendo…" : "Poner estos valores"}
        </Button>
      </div>
    </div>
  );
}
