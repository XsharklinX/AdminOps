import { MonitorSmartphone, Power } from "lucide-react";
import { officeApi, type Client, type Machine, type MachineInventory } from "../lib/api";
import { LabelButton } from "./MachineLabel";
import { useToast } from "./feedback";
import { iconBtn } from "./ui";

export const VERDICT: Record<MachineInventory["verdict"], { label: string; cls: string }> = {
  ok: { label: "Bien", cls: "bg-ok/10 text-ok" },
  upgrade: { label: "Mejorar", cls: "bg-warn/10 text-warn" },
  replace: { label: "Renovar", cls: "bg-bad/10 text-bad" },
};

export function VerdictChip({ inv }: { inv: MachineInventory | null }) {
  if (!inv) return <span className="rounded bg-panel-2 px-2 py-0.5 text-[11px] text-mute">Sin ficha</span>;
  const v = VERDICT[inv.verdict] ?? VERDICT.ok;
  return (
    <span className={`rounded px-2 py-0.5 text-[11px] font-medium ${v.cls}`} title={inv.reasons.join("\n")}>
      {v.label}
    </span>
  );
}

/** Encender por red y conectarse por Escritorio remoto a un equipo del inventario. */
export function MachineActions({ machine, client }: { machine: Machine; client?: Client }) {
  const toast = useToast();
  const inv = machine.inventory;
  return (
    <span className="inline-flex gap-0.5">
      {inv?.mac && (
        <button
          onClick={() =>
            officeApi
              .wake(inv.mac)
              .then(() => toast("ok", `Señal de encendido enviada a ${machine.host}.`))
              .catch((e) => toast("error", String(e)))
          }
          className={iconBtn}
          title="Encender por la red (Wake-on-LAN). El equipo debe estar en esta misma red y tenerlo activado."
        >
          <Power size={14} />
        </button>
      )}
      <LabelButton machine={machine} client={client} />
      <button onClick={() => officeApi.rdp(inv?.ip || machine.host).catch((e) => toast("error", String(e)))} className={iconBtn} title="Conectar por Escritorio remoto">
        <MonitorSmartphone size={14} />
      </button>
    </span>
  );
}

/** Líneas de la ficha para mostrar debajo del nombre del equipo. */
export function inventoryLines(inv: MachineInventory): string[] {
  return [
    [inv.manufacturer, inv.model].filter(Boolean).join(" "),
    `${inv.cpu} · ${inv.ramGb} GB RAM${inv.disks ? ` · ${inv.disks}` : ""}`,
    [inv.os, inv.biosYear ? `de hacia ${inv.biosYear}` : "", inv.security !== null ? `seguridad ${inv.security}/100` : ""].filter(Boolean).join(" · "),
  ].filter(Boolean);
}
