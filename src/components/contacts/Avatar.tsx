import { createContext, useContext, useEffect, useMemo, useState } from "react";
import type { Contact, TeamsPresence } from "../../lib/api";
import { graphApi } from "../../lib/api";
import { usePageActive } from "../../lib/pageActive";
import { useGraph } from "../M365";

/** Tono estable para cada nombre: la misma persona tiene siempre el mismo color. */
export function hueOf(name: string): number {
  let h = 0;
  for (const ch of name) h = (h * 31 + ch.charCodeAt(0)) % 360;
  return h;
}

export function initials(name: string): string {
  return (
    name
      .split(/\s+/)
      .filter(Boolean)
      .slice(0, 2)
      .map((w) => w.charAt(0).toUpperCase())
      .join("") || "?"
  );
}

// ---------- Presencia de Teams y fotos de Microsoft 365 ----------

const AVAIL: Record<string, { label: string; dot: string }> = {
  Available: { label: "Disponible", dot: "bg-ok" },
  AvailableIdle: { label: "Disponible (inactivo)", dot: "bg-ok" },
  Busy: { label: "Ocupado", dot: "bg-bad" },
  BusyIdle: { label: "Ocupado (inactivo)", dot: "bg-bad" },
  DoNotDisturb: { label: "No molestar", dot: "bg-bad" },
  Away: { label: "Ausente", dot: "bg-warn" },
  BeRightBack: { label: "Vuelvo enseguida", dot: "bg-warn" },
  Offline: { label: "Desconectado", dot: "bg-mute" },
};

const ACTIVITY: Record<string, string> = {
  InACall: "en una llamada",
  InAConferenceCall: "en una llamada",
  InAMeeting: "en una reunión",
  Presenting: "presentando",
  OutOfOffice: "fuera de la oficina",
  Inactive: "inactivo",
};

/** «Ocupado · en una reunión», o null si no se sabe. */
export function presenceText(p: TeamsPresence | undefined): string | null {
  if (!p || !AVAIL[p.availability]) return null;
  const extra = ACTIVITY[p.activity];
  return extra ? `${AVAIL[p.availability].label} · ${extra}` : AVAIL[p.availability].label;
}

interface PresenceData {
  presence: Record<string, TeamsPresence>;
  photos: Record<string, string>;
}

const PresenceCtx = createContext<PresenceData>({ presence: {}, photos: {} });

/**
 * Presencia (cada 2 minutos mientras la página está a la vista) y fotos (una
 * vez; el programa las guarda una semana) de unos correos. Sin Microsoft 365
 * conectado, o sin permiso, no hace nada y no molesta.
 */
export function useTeamsPresence(emails: string[]): PresenceData {
  const graph = useGraph();
  const active = usePageActive();
  const list = useMemo(() => [...new Set(emails.map((e) => e.trim().toLowerCase()).filter((e) => e.includes("@")))].slice(0, 200), [emails]);
  const key = list.join(",");
  const [data, setData] = useState<PresenceData>({ presence: {}, photos: {} });

  useEffect(() => {
    if (!graph?.connected || !active || list.length === 0) return;
    let vivo = true;
    let failed = false;
    const tick = () => {
      if (failed) return;
      graphApi
        .presence(list)
        .then((p) => vivo && setData((d) => ({ ...d, presence: p })))
        .catch(() => (failed = true));
    };
    tick();
    graphApi
      .photos(list)
      .then((ph) => vivo && setData((d) => ({ ...d, photos: { ...d.photos, ...ph } })))
      .catch(() => {});
    const t = window.setInterval(tick, 120_000);
    return () => {
      vivo = false;
      window.clearInterval(t);
    };
    // eslint-disable-next-line react-hooks/exhaustive-deps -- `key` resume `list`
  }, [graph, active, key]);
  return data;
}

/** La presencia de una persona dentro de un PresenceProvider. */
export function usePresenceOf(email?: string): TeamsPresence | undefined {
  const ctx = useContext(PresenceCtx);
  return email ? ctx.presence[email.trim().toLowerCase()] : undefined;
}

/** Da presencia y fotos a todos los Avatar de dentro. */
export function PresenceProvider({ emails, children }: { emails: string[]; children: React.ReactNode }) {
  const data = useTeamsPresence(emails);
  return <PresenceCtx.Provider value={data}>{children}</PresenceCtx.Provider>;
}

/** Círculo con la foto de Microsoft 365 o las iniciales, y el punto de presencia de Teams. */
export function Avatar({
  c,
  size = 36,
  photo,
  presence,
}: {
  c: Pick<Contact, "name" | "favorite"> & { email?: string };
  size?: number;
  /** Si no se dan, se toman del PresenceProvider por el correo. */
  photo?: string;
  presence?: TeamsPresence;
}) {
  const ctx = useContext(PresenceCtx);
  const mail = c.email?.trim().toLowerCase() ?? "";
  const pic = photo ?? (mail ? ctx.photos[mail] : undefined);
  const pres = presence ?? (mail ? ctx.presence[mail] : undefined);
  const state = pres ? AVAIL[pres.availability] : undefined;
  const h = hueOf(c.name);
  const dot = Math.max(8, Math.round(size * 0.28));
  return (
    <span className="relative inline-grid shrink-0" style={{ width: size, height: size }} title={presenceText(pres) ?? undefined}>
      {pic ? (
        <img src={pic} alt="" className="size-full rounded-full object-cover" />
      ) : (
        <span
          className="grid size-full place-items-center rounded-full font-semibold"
          style={{
            fontSize: Math.round(size * 0.36),
            background: `hsl(${h} 70% 50% / 0.16)`,
            color: `hsl(${h} 65% 55%)`,
            boxShadow: `inset 0 0 0 1px hsl(${h} 70% 50% / 0.35)`,
          }}
          aria-hidden
        >
          {initials(c.name)}
        </span>
      )}
      {state && <span className={`absolute right-0 bottom-0 rounded-full ring-2 ring-[var(--color-panel)] ${state.dot}`} style={{ width: dot, height: dot }} />}
    </span>
  );
}
