// «¿Qué cambió?»: «desde el martes va mal». Dice qué es distinto en este equipo
// desde la fecha que se elija, con su arreglo si ya existe en AdminOps.
import { Camera, GitCompare, Loader2 } from "lucide-react";
import { useCallback, useEffect, useState } from "react";
import { logQuietly, whatChangedApi, type ChangeView, type WhatChangedInfo } from "../lib/api";
import { goToPage } from "../lib/navigate";
import { addDays, dayEs, describeChange, groupByKind, isoDay, KIND_LABEL, weekdayOf } from "../lib/whatChanged";
import { useToast } from "./feedback";
import { Button, Card, EmptyLine } from "./ui";

export function WhatChanged() {
  const [info, setInfo] = useState<WhatChangedInfo | null>(null);
  const [day, setDay] = useState(() => isoDay(addDays(new Date(), -7)));
  const [list, setList] = useState<ChangeView[] | null>(null);
  const [busy, setBusy] = useState(false);
  const [snapping, setSnapping] = useState(false);
  const toast = useToast();

  useEffect(() => {
    whatChangedApi.info().then(setInfo).catch(logQuietly("WhatChanged"));
  }, []);

  const load = useCallback(
    async (d: string) => {
      setBusy(true);
      try {
        setList(await whatChangedApi.since(d));
      } catch (e) {
        toast("error", String(e));
        setList(null);
      } finally {
        setBusy(false);
      }
    },
    [toast],
  );

  const hasPhotos = !!info?.firstDay;
  useEffect(() => {
    if (hasPhotos && day) void load(day);
  }, [hasPhotos, day, load]);

  const snap = async () => {
    setSnapping(true);
    try {
      setInfo(await whatChangedApi.snapshot());
      if (day) await load(day);
      toast("ok", "Foto de hoy guardada.");
    } catch (e) {
      toast("error", String(e));
    } finally {
      setSnapping(false);
    }
  };

  const groups = groupByKind(list ?? []);
  return (
    <div className="mx-auto max-w-(--page-max) space-y-4 p-6">
      <Card title="¿Qué cambió?" icon={<GitCompare size={14} />}>
        <p className="text-sm text-dim">
          AdminOps guarda cada día una foto pequeña de este equipo (programas, drivers, actualizaciones, servicios, lo que arranca y los ajustes de energía) y te dice qué es distinto
          desde la fecha que elijas. Solo lee.
        </p>
        <div className="mt-3 flex flex-wrap items-center gap-3 text-sm">
          <label className="flex items-center gap-2 text-dim">
            Desde
            <input
              type="date"
              value={day}
              min={info?.firstDay || undefined}
              max={isoDay(new Date())}
              onChange={(e) => e.target.value && setDay(e.target.value)}
              className="rounded-md border border-line bg-void px-2 py-1 text-ink"
              aria-label="Fecha desde la que mirar"
            />
          </label>
          {day && <span className="text-mute">({weekdayOf(day)})</span>}
          <Button kind="ghost" onClick={() => void snap()} disabled={snapping}>
            {snapping ? <Loader2 size={13} className="animate-spin" /> : <Camera size={13} />} Hacer la foto de hoy
          </Button>
        </div>
        <p className="mt-2 text-[11px] text-mute">
          {hasPhotos
            ? `Fotos desde el ${dayEs(info.firstDay)} · 60 días como máximo, unos KB por día, en los datos de este equipo.`
            : "Todavía no hay ninguna foto: AdminOps hace la primera unos minutos después de abrirse. También puedes hacerla ahora."}
        </p>
      </Card>

      {hasPhotos && (
        <Card title={list ? `Desde el ${weekdayOf(day)} ${dayEs(day)}: ${list.length} ${list.length === 1 ? "cambio" : "cambios"}` : "Cambios"}>
          {busy && !list && (
            <p className="flex items-center gap-2 text-xs text-mute">
              <Loader2 size={13} className="animate-spin" /> Comparando…
            </p>
          )}
          {list && list.length === 0 && <EmptyLine>No ha cambiado nada de lo que se vigila desde esa fecha.</EmptyLine>}
          <div className="space-y-4">
            {groups.map((g) => (
              <section key={g.kind}>
                <h3 className="mb-1 text-[11px] font-medium tracking-wide text-mute uppercase">
                  {KIND_LABEL[g.kind]} · {g.items.length}
                </h3>
                <ul className="divide-y divide-line/60">
                  {g.items.map((c, i) => (
                    <li key={`${c.kind}-${c.label}-${i}`} className="flex flex-wrap items-center gap-x-3 gap-y-1 py-1.5 text-sm">
                      <span className={`size-2 shrink-0 rounded-full ${c.change === "added" ? "bg-neon" : c.change === "removed" ? "bg-bad" : "bg-warn"}`} />
                      <span className="min-w-0 flex-1">
                        <span className="block truncate text-ink" title={c.label}>
                          {c.label}
                        </span>
                        <span className="block truncate text-[11px] text-mute">{describeChange(c)}</span>
                      </span>
                      <span className="shrink-0 text-[11px] text-mute">{dayEs(c.day)}</span>
                      {c.fix && (
                        <Button kind="ghost" onClick={() => goToPage(c.fix?.page ?? "machine", c.fix?.focus ?? null)}>
                          {c.fix.label}
                        </Button>
                      )}
                    </li>
                  ))}
                </ul>
              </section>
            ))}
          </div>
        </Card>
      )}
    </div>
  );
}
