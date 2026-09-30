import { useEffect, useMemo, useState } from "react";
import { useToast } from "./feedback";
import { solutionForFinding } from "../lib/solutionsCatalog";
import type { PageId } from "./Sidebar";
import { agendaApi, alertsApi, appBackupApi, diagApi, toolsApi, tweaksApi, workApi, type Finding, type FindingAction } from "../lib/api";

export interface PlanItem {
  key: string;
  level: "bad" | "warn" | "info";
  title: string;
  detail?: string;
  /** De dónde sale: diagnóstico, aviso, actualizaciones… */
  source: string;
  action?: { label: string; run: () => void };
}

const ORDER = { bad: 0, warn: 1, info: 2 };
const DOT = { bad: "bg-bad", warn: "bg-warn", info: "bg-neon" };
const DAY = 86_400;

/**
 * «Qué hacer ahora»: todo lo que pide atención en este equipo, de cualquier
 * parte de AdminOps, en una sola lista ordenada por importancia.
 */
export function ActionPlan({
  findings,
  diagnosedAt,
  systemDisk,
  refresh,
  onNavigate,
}: {
  findings: Finding[];
  diagnosedAt: number | null;
  systemDisk: { free: number; total: number; mount: string } | null;
  refresh: number;
  onNavigate: (page: PageId, focus?: string | null) => void;
}) {
  const toast = useToast();
  const [extra, setExtra] = useState<PlanItem[]>([]);
  const [all, setAll] = useState(false);

  // Lo que no depende del diagnóstico: avisos, actualizaciones, datos, sesión.
  useEffect(() => {
    let alive = true;
    (async () => {
      const now = Date.now() / 1000;
      const [alerts, updates, ignored, health, session, agenda] = await Promise.all([
        alertsApi.list().catch(() => []),
        toolsApi.cachedUpdates().catch(() => null),
        toolsApi.ignoredUpdates().catch(() => [] as string[]),
        appBackupApi.health().catch(() => null),
        workApi.session().catch(() => null),
        agendaApi.list().catch(() => null),
      ]);
      const out: PlanItem[] = [];
      const today = new Date();
      const isToday = (ts: number) => new Date(ts * 1000).toDateString() === today.toDateString();
      for (const v of (agenda?.visits ?? []).filter((v) => v.status === "planned" && isToday(v.start) && v.start > now - 3 * 3600)) {
        const at = new Date(v.start * 1000).toLocaleTimeString("es", { hour: "2-digit", minute: "2-digit" });
        out.push({
          key: `visit:${v.id}`,
          level: "info",
          source: "Agenda",
          title: `Hoy a las ${at}: visita a ${v.clientName}`,
          detail: [v.machines ? `${v.machines} ${v.machines === 1 ? "equipo" : "equipos"}` : "", v.notes].filter(Boolean).join(" · ") || undefined,
          action: { label: "Empezar", run: () => onNavigate("session", v.clientId) },
        });
      }
      const overdue = (agenda?.due ?? []).filter((d) => d.date < now);
      if (overdue.length) {
        out.push({
          key: "due",
          level: "info",
          source: "Agenda",
          title: `${overdue.length} ${overdue.length === 1 ? "cliente necesita" : "clientes necesitan"} mantenimiento`,
          detail: overdue
            .slice(0, 3)
            .map((d) => d.name)
            .join(", ") + (overdue.length > 3 ? "…" : ""),
          action: { label: "Agendar", run: () => onNavigate("agenda") },
        });
      }
      if (session) {
        out.push({ key: "session", level: "info", source: "Sesión", title: `Sesión de servicio en curso con ${session.clientName}`, action: { label: "Continuar", run: () => onNavigate("session") } });
      }
      for (const a of alerts.filter((a) => !a.read && a.level !== "info" && now - a.time < 3 * DAY).slice(0, 5)) {
        out.push({
          key: `alert:${a.id}`,
          level: a.level === "bad" ? "bad" : "warn",
          source: "Aviso de Windows",
          title: a.title,
          detail: a.detail || a.advice,
          action: a.page ? { label: "Revisar", run: () => onNavigate(a.page as PageId) } : undefined,
        });
      }
      const pending = (updates ?? []).filter((u) => !ignored.includes(u.id));
      if (pending.length) {
        out.push({
          key: "updates",
          level: pending.length >= 10 ? "warn" : "info",
          source: "Actualizaciones",
          title: `${pending.length} ${pending.length === 1 ? "programa tiene" : "programas tienen"} versión nueva`,
          detail: pending
            .slice(0, 4)
            .map((u) => u.name)
            .join(", ") + (pending.length > 4 ? "…" : ""),
          action: { label: "Actualizar", run: () => onNavigate("software") },
        });
      }
      for (const w of (health?.warnings ?? []).slice(0, 2)) {
        out.push({ key: `health:${w}`, level: /problema|poco espacio|falta la clave/i.test(w) ? "warn" : "info", source: "Tus datos", title: w, action: { label: "Revisar", run: () => onNavigate("settings") } });
      }
      if (alive) setExtra(out);
    })();
    return () => {
      alive = false;
    };
  }, [refresh, onNavigate]);

  const items = useMemo(() => {
    const act = (a: FindingAction) => {
      if (a.kind === "fix")
        return () =>
          void tweaksApi
            .fixFinding(a.id)
            .then((m) => toast("ok", `${a.label}: ${m}`))
            .catch((e) => toast("error", `${a.label}: ${e}`));
      if (a.kind === "tool") return () => void diagApi.openTool(a.tool);
      return () => onNavigate(a.page as PageId, a.focus);
    };
    const fromDiag: PlanItem[] = findings
      .filter((f) => f.severity !== "info")
      .map((f, i) => ({
        key: `diag:${i}`,
        level: f.severity === "bad" ? "bad" : "warn",
        source: "Diagnóstico",
        title: f.title,
        detail: f.detail ?? undefined,
        action: f.actions[0]
          ? { label: f.actions[0].label, run: act(f.actions[0]) }
          : // Sin acción directa, al menos los pasos para arreglarlo.
            (() => {
              const sol = solutionForFinding(f);
              return sol ? { label: "Cómo se arregla", run: () => onNavigate("knowledge", `solution:${sol}`) } : undefined;
            })(),
      }));
    // Espacio en vivo: por si el diagnóstico es antiguo o no lo hay.
    const live: PlanItem[] = [];
    const stale = !diagnosedAt || Date.now() / 1000 - diagnosedAt > DAY;
    if (systemDisk && stale && systemDisk.total > 0) {
      const gb = systemDisk.free / 1024 ** 3;
      if (gb < 10 || systemDisk.free / systemDisk.total < 0.1) {
        live.push({ key: "disk", level: "bad", source: "Ahora", title: `Queda poco espacio en ${systemDisk.mount.replace(/\\$/, "")}`, detail: `${gb.toFixed(1)} GB libres`, action: { label: "Liberar espacio", run: () => onNavigate("space") } });
      }
    }
    // Sin repetir lo mismo que dicen dos fuentes.
    const seen = new Set<string>();
    return [...live, ...extra, ...fromDiag]
      .filter((x) => {
        const k = x.title.toLowerCase().slice(0, 40);
        if (seen.has(k)) return false;
        seen.add(k);
        return true;
      })
      .sort((a, b) => ORDER[a.level] - ORDER[b.level]);
  }, [findings, diagnosedAt, systemDisk, extra, onNavigate, toast]);

  if (!items.length) return <p className="flex items-center gap-2.5 py-5 text-sm text-dim"><span className="size-2 rounded-full bg-ok" /> Nada pendiente: el equipo está al día.</p>;

  const shown = all ? items : items.slice(0, 7);
  return (
    <div>
      {shown.map((it) => (
        <div key={it.key} className="flex items-center gap-3.5 border-b border-line py-3">
          <span className={`size-2 shrink-0 rounded-full ${DOT[it.level]}`} />
          <div className="min-w-0 flex-1">
            <div className="truncate text-sm">{it.title}</div>
            <div className="truncate text-xs text-mute">
              <span className="text-dim">{it.source}</span>
              {it.detail && ` · ${it.detail}`}
            </div>
          </div>
          {it.action && (
            <button onClick={it.action.run} className="shrink-0 text-[13px] whitespace-nowrap text-neon hover:underline">
              {it.action.label}
            </button>
          )}
        </div>
      ))}
      {items.length > 7 && (
        <button onClick={() => setAll(!all)} className="mt-2 text-xs text-neon hover:underline">
          {all ? "Mostrar menos" : `Ver todo (${items.length})`}
        </button>
      )}
    </div>
  );
}
