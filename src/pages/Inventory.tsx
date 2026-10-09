import { Boxes, Download, Plus, Trash2 } from "lucide-react";
import { useCallback, useEffect, useMemo, useState } from "react";
import { useConfirm, useToast } from "../components/feedback";
import { inventoryLines, MachineActions, VerdictChip, VERDICT } from "../components/inventory";
import { TaskStatus } from "../components/TaskStatus";
import { Button, Card } from "../components/ui";
import { officeApi, toCsv, workApi, type Client, type Machine } from "../lib/api";
import { fullDate as date } from "../lib/format";

type Row = { client: Client; machine: Machine };
export function Inventory() {
  const [clients, setClients] = useState<Client[] | null>(null);
  const [filter, setFilter] = useState<string>("all");
  const [clientFilter, setClientFilter] = useState<string>("all");
  const [target, setTarget] = useState("");
  const [busy, setBusy] = useState(false);
  const toast = useToast();
  const { confirm, dialog } = useConfirm();

  const load = useCallback(async () => {
    const c = await workApi.clients();
    setClients(c);
    setTarget((t) => t || c[0]?.id || "");
  }, []);

  useEffect(() => {
    load().catch((e) => toast("error", String(e)));
  }, [load, toast]);

  const rows: Row[] = useMemo(
    () =>
      (clients ?? [])
        .filter((c) => clientFilter === "all" || c.id === clientFilter)
        .flatMap((client) => client.machines.map((machine) => ({ client, machine })))
        .filter((r) => filter === "all" || (r.machine.inventory?.verdict ?? "none") === filter),
    [clients, filter, clientFilter],
  );

  const counts = useMemo(() => {
    const all = (clients ?? []).filter((c) => clientFilter === "all" || c.id === clientFilter).flatMap((c) => c.machines);
    return { all: all.length, ok: all.filter((m) => m.inventory?.verdict === "ok").length, upgrade: all.filter((m) => m.inventory?.verdict === "upgrade").length, replace: all.filter((m) => m.inventory?.verdict === "replace").length };
  }, [clients, clientFilter]);

  const addThis = async () => {
    if (!target) return;
    setBusy(true);
    try {
      const c = await workApi.inventoryAddThis(target);
      toast("ok", `Este equipo quedó en el inventario de ${c.name}.`);
      void load();
    } catch (e) {
      toast("error", String(e));
    } finally {
      setBusy(false);
    }
  };

  const remove = async (r: Row) => {
    const ok = await confirm({
      title: `¿Quitar ${r.machine.host} del inventario?`,
      confirmLabel: "Quitar",
      body: <p>Se quita de la ficha de {r.client.name}. Su historial de visitas no se toca.</p>,
    });
    if (!ok) return;
    await workApi.inventoryRemove(r.client.id, r.machine.host).catch((e) => toast("error", String(e)));
    void load();
  };

  const exportCsv = async () => {
    const header = ["Cliente", "Equipo", "Fabricante", "Modelo", "Nº de serie", "Procesador", "RAM (GB)", "Discos", "Gráfica", "Windows", "Año aprox.", "TPM", "Seguridad", "Batería (%)", "IP", "MAC", "Estado", "Recomendaciones", "Actualizado"];
    const data = rows.map(({ client, machine: m }) => {
      const i = m.inventory;
      return [
        client.name,
        m.host,
        i?.manufacturer,
        i?.model,
        i?.serial,
        i?.cpu,
        i?.ramGb,
        i?.disks,
        i?.gpu,
        i?.os ?? m.os,
        i?.biosYear,
        i?.tpm === null || i?.tpm === undefined ? "" : i.tpm ? "Sí" : "No",
        i?.security,
        i?.battery !== null && i?.battery !== undefined ? Math.round(i.battery) : "",
        i?.ip,
        i?.mac,
        i ? VERDICT[i.verdict].label : "Sin ficha",
        i?.reasons.join(" | "),
        date(m.lastSeen),
      ];
    });
    const name = clientFilter === "all" ? "Inventario de equipos" : `Inventario ${clients?.find((c) => c.id === clientFilter)?.name ?? ""}`;
    try {
      const p = await officeApi.exportCsv(name, toCsv([header, ...data]));
      if (p) toast("ok", "Inventario exportado (se abre con Excel).");
    } catch (e) {
      toast("error", String(e));
    }
  };

  const chip = (id: string, label: string, n: number) => (
    <button
      key={id}
      onClick={() => setFilter(id)}
      className={`rounded-md px-3 py-1.5 text-[13px] transition-colors ${filter === id ? "bg-panel-2 font-medium text-ink" : "text-dim hover:text-ink"}`}
    >
      {label} <span className="text-mute">{n}</span>
    </button>
  );

  return (
    <div className="mx-auto max-w-(--page-max) space-y-4 p-6">
      <Card title="Inventario de equipos" icon={<Boxes size={14} />}>
        <p className="mb-3 text-sm text-dim">
          Cada equipo que pasa por una sesión de servicio (o que añades aquí) queda en la ficha de su cliente con su hardware y una recomendación:
          seguir, mejorar (SSD, memoria, batería…) o renovar.
        </p>
        <div className="flex flex-wrap items-center gap-2">
          <span className="text-sm text-dim">Añadir este equipo a</span>
          <select value={target} onChange={(e) => setTarget(e.target.value)} className="rounded-md border border-line bg-void/60 px-3 py-1.5 text-sm text-ink outline-none">
            {(clients ?? []).map((c) => (
              <option key={c.id} value={c.id}>
                {c.name}
              </option>
            ))}
          </select>
          <Button onClick={addThis} disabled={busy || !target}>
            <Plus size={14} /> Añadir
          </Button>
          {clients?.length === 0 && <span className="text-xs text-mute">Crea antes un cliente en Soporte → Personas y clientes → Clientes.</span>}
        </div>
        <TaskStatus task="inventory" active={busy} fallback="Analizando este equipo…" cancellable={false} className="mt-3" />
      </Card>

      <div className="flex flex-wrap items-center gap-2">
        <div className="inline-flex rounded-lg border border-line bg-void p-0.5">
          {chip("all", "Todos", counts.all)}
          {chip("replace", "Renovar", counts.replace)}
          {chip("upgrade", "Mejorar", counts.upgrade)}
          {chip("ok", "Bien", counts.ok)}
        </div>
        <select value={clientFilter} onChange={(e) => setClientFilter(e.target.value)} className="rounded-md border border-line bg-void/60 px-3 py-1.5 text-sm text-ink outline-none">
          <option value="all">Todos los clientes</option>
          {(clients ?? []).filter((c) => c.machines.length).map((c) => (
            <option key={c.id} value={c.id}>
              {c.name} · {c.machines.length}
            </option>
          ))}
        </select>
        <span className="ml-auto" />
        <Button kind="ghost" onClick={exportCsv} disabled={rows.length === 0}>
          <Download size={14} /> Exportar a Excel (CSV)
        </Button>
      </div>

      <Card title={`${rows.length} equipos`}>
        {rows.length === 0 ? (
          <p className="text-sm text-mute">{clients === null ? "Cargando…" : "No hay equipos con este filtro."}</p>
        ) : (
          <ul className="divide-y divide-line/60">
            {rows.map((r) => (
              <li key={`${r.client.id}-${r.machine.host}`} className="group flex items-start gap-4 py-3">
                <div className="min-w-0 flex-1">
                  <div className="flex flex-wrap items-center gap-2">
                    <span className="font-mono text-sm text-ink">{r.machine.host}</span>
                    <VerdictChip inv={r.machine.inventory} />
                    <span className="text-xs text-mute">{r.client.name}</span>
                  </div>
                  {r.machine.inventory ? (
                    <>
                      {inventoryLines(r.machine.inventory).map((l) => (
                        <div key={l} className="truncate text-xs text-dim">
                          {l}
                        </div>
                      ))}
                      {r.machine.inventory.reasons.length > 0 && (
                        <ul className="mt-1 space-y-0.5">
                          {r.machine.inventory.reasons.map((x) => (
                            <li key={x} className="text-xs text-ink">
                              → {x}
                            </li>
                          ))}
                        </ul>
                      )}
                    </>
                  ) : (
                    <div className="text-xs text-mute">{r.machine.hardware || r.machine.os} · se completa en la próxima visita</div>
                  )}
                </div>
                <div className="flex shrink-0 flex-col items-end gap-1">
                  <span className="text-[11px] text-mute">visto el {date(r.machine.lastSeen)}</span>
                  <span className="flex items-center gap-1">
                    <MachineActions machine={r.machine} />
                    <button onClick={() => remove(r)} className="rounded-md p-1.5 text-mute opacity-0 group-hover:opacity-100 hover:bg-panel-2 hover:text-bad" title="Quitar del inventario">
                      <Trash2 size={14} />
                    </button>
                  </span>
                </div>
              </li>
            ))}
          </ul>
        )}
      </Card>
      {dialog}
    </div>
  );
}
