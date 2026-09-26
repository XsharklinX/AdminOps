import type { LucideIcon } from "lucide-react";

export function Placeholder({ label, icon: Icon, phase }: { label: string; icon: LucideIcon; phase?: number }) {
  return (
    <div className="grid h-full place-items-center p-8">
      <div className="text-center">
        <div className="mx-auto mb-4 grid size-14 place-items-center rounded-2xl border border-line bg-panel text-mute">
          <Icon size={24} strokeWidth={1.5} />
        </div>
        <h2 className="text-lg font-semibold">{label}</h2>
        <p className="mt-1 text-sm text-dim">Este módulo llega en la Fase {phase}.</p>
      </div>
    </div>
  );
}
