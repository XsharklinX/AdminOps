import {
  Activity,
  Brush,
  Cog,
  History,
  Package,
  Power,
  ShieldHalf,
  Stethoscope,
  type LucideIcon,
} from "lucide-react";

export type PageId =
  | "dashboard"
  | "cleanup"
  | "privacy"
  | "bloatware"
  | "services"
  | "startup"
  | "diagnostics"
  | "history";

export const NAV: { id: PageId; label: string; icon: LucideIcon; phase?: number }[] = [
  { id: "dashboard", label: "Panel", icon: Activity },
  { id: "cleanup", label: "Limpieza", icon: Brush, phase: 3 },
  { id: "privacy", label: "Privacidad", icon: ShieldHalf, phase: 3 },
  { id: "bloatware", label: "Bloatware", icon: Package, phase: 3 },
  { id: "services", label: "Servicios", icon: Cog, phase: 3 },
  { id: "startup", label: "Inicio", icon: Power, phase: 3 },
  { id: "diagnostics", label: "Diagnóstico", icon: Stethoscope, phase: 4 },
  { id: "history", label: "Historial", icon: History, phase: 2 },
];

export function Sidebar({
  active,
  onSelect,
  isAdmin,
}: {
  active: PageId;
  onSelect: (id: PageId) => void;
  isAdmin: boolean | null;
}) {
  return (
    <aside className="flex w-56 shrink-0 flex-col border-r border-line bg-panel/60">
      <div className="flex items-center gap-2.5 px-5 pt-5 pb-6">
        <div className="grid size-8 place-items-center rounded-lg bg-neon/10 glow-neon">
          <span className="font-mono text-sm font-bold text-neon">A/</span>
        </div>
        <div>
          <div className="text-[15px] font-semibold tracking-tight">AdminOps</div>
          <div className="text-[10px] tracking-widest text-mute uppercase">v0.1 · Fase 1</div>
        </div>
      </div>

      <nav className="flex flex-1 flex-col gap-0.5 px-3">
        {NAV.map(({ id, label, icon: Icon, phase }) => {
          const on = id === active;
          return (
            <button
              key={id}
              onClick={() => onSelect(id)}
              className={`group relative flex items-center gap-3 rounded-lg px-3 py-2 text-left text-sm transition-colors ${
                on ? "bg-neon/10 text-neon" : "text-dim hover:bg-panel-2 hover:text-ink"
              }`}
            >
              {on && <span className="absolute top-1.5 bottom-1.5 left-0 w-0.5 rounded-full bg-neon shadow-[0_0_8px_var(--color-neon)]" />}
              <Icon size={16} strokeWidth={1.8} />
              <span className="flex-1">{label}</span>
              {phase && <span className="rounded bg-line px-1.5 py-px font-mono text-[9px] text-mute">F{phase}</span>}
            </button>
          );
        })}
      </nav>

      <div className="m-3 rounded-lg border border-line bg-void/60 px-3 py-2.5">
        <div className="flex items-center gap-2 text-xs">
          <span
            className={`size-2 rounded-full ${isAdmin ? "bg-ok shadow-[0_0_8px_var(--color-ok)]" : "bg-warn shadow-[0_0_8px_var(--color-warn)]"}`}
          />
          <span className={isAdmin ? "text-ok" : "text-warn"}>
            {isAdmin === null ? "Comprobando…" : isAdmin ? "Administrador" : "Usuario estándar"}
          </span>
        </div>
      </div>
    </aside>
  );
}
