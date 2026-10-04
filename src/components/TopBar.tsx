// La barra de arriba (1.2): el equipo en una línea. Cada dato se puede pulsar y
// lleva a donde se mira o se arregla: el dominio a Dominio, el disco a Espacio,
// los avisos al Diagnóstico. Teams y Correo se abren desde aquí, encima de lo
// que estés viendo, en lugar de ocupar sitio en la barra lateral.
import { Mail, MessagesSquare, Search } from "lucide-react";
import type { ReactNode } from "react";
import logo from "../assets/logo.svg";
import { api, type AppInfo, type TargetUser } from "../lib/api";
import { diskTone, gbText, type MachineState, type Tone } from "../lib/machineState";
import { openComm } from "../lib/comms";
import { usePrefs } from "../lib/prefs";
import { allowedInMode, type PageId } from "./Sidebar";

const DOT: Record<Tone, string> = { ok: "bg-ok", warn: "bg-warn", bad: "bg-bad", neutral: "bg-mute" };

function Chip({ tone, children, title, onClick }: { tone: Tone; children: ReactNode; title: string; onClick?: () => void }) {
  const cls = "flex h-7 shrink-0 items-center gap-1.5 rounded-full border border-line-2 bg-panel-2 px-2.5 text-xs whitespace-nowrap text-dim";
  const body = (
    <>
      <span className={`size-[7px] shrink-0 rounded-full ${DOT[tone]}`} />
      {children}
    </>
  );
  return onClick ? (
    <button onClick={onClick} title={title} className={`${cls} transition-colors hover:border-neon/60 hover:text-ink`}>
      {body}
    </button>
  ) : (
    <span title={title} className={cls}>
      {body}
    </span>
  );
}

export function TopBar({
  state,
  isAdmin,
  targetUser,
  appInfo,
  onAbout,
  onSearch,
  onNavigate,
  right,
}: {
  state: MachineState;
  isAdmin: boolean | null;
  targetUser: TargetUser | null;
  appInfo: AppInfo | null;
  onAbout: () => void;
  onSearch: () => void;
  onNavigate: (page: PageId, section?: string | null) => void;
  /** Lo que ya había a la derecha: caso, tareas, auditoría y avisos. */
  right: ReactNode;
}) {
  const prefs = usePrefs();
  const { ctx, net, findings } = state;
  const b = (t: string) => <b className="font-medium text-ink">{t}</b>;
  const where = { ask: "", adminops: " (en AdminOps)", browser: " (en el navegador)", app: " (en su aplicación)" };
  const appButton = (page: "teams" | "mail", label: string, Icon: typeof Mail) =>
    allowedInMode(page, prefs.mode) && (
      <button
        onClick={() => openComm(page)}
        className="grid size-8 place-items-center rounded-md text-mute transition-colors hover:bg-panel-2 hover:text-ink"
        title={`${label}${where[prefs.comms[page]]} · se cambia en Ajustes → Portales y correo`}
        aria-label={label}
      >
        <Icon size={17} strokeWidth={1.6} />
      </button>
    );

  return (
    <div className="flex h-12 shrink-0 items-center gap-3 border-b border-line bg-panel px-3">
      <button onClick={onAbout} className="flex shrink-0 items-center gap-2 rounded-md px-1.5 py-1 hover:bg-panel-2" title="Acerca de AdminOps">
        <img src={logo} alt="" className="size-7" draggable={false} />
        <span className="text-left leading-tight">
          <span className="block text-[13.5px] font-semibold tracking-tight text-ink">AdminOps</span>
          <span className="flex items-center gap-1 text-[10.5px] text-mute">
            {appInfo ? `v${appInfo.version}` : "…"}
            {appInfo?.portable && (
              <span className="rounded border border-line-2 px-1 text-[10px]" title="Modo portable: los datos se guardan junto a AdminOps.exe, no en este equipo">
                portable
              </span>
            )}
          </span>
        </span>
      </button>

      {prefs.sidebar.showSearch && (
        <button
          onClick={onSearch}
          className="flex h-8 w-56 shrink-0 items-center gap-2 rounded-lg border border-line-2 bg-void px-2.5 text-left text-[13px] text-mute transition-colors hover:border-neon/50 hover:text-dim"
          title="Todo AdminOps: cada función del programa, y el buscador"
        >
          <Search size={14} strokeWidth={1.8} />
          <span className="flex-1">Buscar o ir a…</span>
          <kbd className="rounded border border-line-2 px-1 font-mono text-[10.5px]">Ctrl K</kbd>
        </button>
      )}

      {/* El equipo en una línea. Si no cabe todo, se corta por la derecha sin empujar nada. */}
      <div className="no-scrollbar flex min-w-0 flex-1 items-center gap-1.5 overflow-x-auto" aria-label="Estado de este equipo">
        {ctx && (
          <Chip tone="ok" title="Nombre de este equipo · ver sus piezas" onClick={() => onNavigate("machine", "hardware")}>
            {b(ctx.computerName)}
          </Chip>
        )}
        {ctx?.join === "domain" && (
          <Chip tone="ok" title={`En el dominio ${ctx.joinName ?? ""} · ver el estado del dominio`} onClick={() => onNavigate("users", "domain")}>
            Dominio {b(ctx.joinName ?? "")}
          </Chip>
        )}
        {ctx?.join === "workgroup" && (
          <Chip tone="neutral" title="Este equipo no está en ningún dominio · unirlo" onClick={() => onNavigate("users", "domain")}>
            Sin dominio {ctx.joinName && <span className="text-mute">({ctx.joinName})</span>}
          </Chip>
        )}
        {isAdmin !== null &&
          (isAdmin ? (
            <Chip tone="ok" title={targetUser ? `AdminOps tiene permisos de administrador · los ajustes de usuario se aplican a ${targetUser.name}` : "AdminOps tiene permisos de administrador"}>
              Administrador
            </Chip>
          ) : (
            <Chip tone="warn" title="Sin permisos de administrador solo se puede mirar · pulsa para reiniciar AdminOps como administrador (Windows lo confirma)" onClick={() => void api.relaunchAsAdmin().catch(() => {})}>
              Solo lectura
            </Chip>
          ))}
        {net &&
          (net.online ? (
            <Chip tone="ok" title="Hay Internet · medir la velocidad" onClick={() => onNavigate("router", "speed")}>
              Internet {net.ms !== null && b(`${net.ms} ms`)}
            </Chip>
          ) : (
            <Chip tone="bad" title="No hay conexión a Internet · ver qué pasa" onClick={() => onNavigate("troubleshoot", "internet")}>
              Sin Internet
            </Chip>
          ))}
        {ctx && ctx.totalBytes > 0 && (
          <Chip tone={diskTone(ctx.freeBytes, ctx.totalBytes)} title={`Espacio libre en ${ctx.systemDrive} · ver qué ocupa`} onClick={() => onNavigate("space", "space")}>
            {ctx.systemDrive} {b(`${gbText(ctx.freeBytes)} libres`)}
          </Chip>
        )}
        {findings ? (
          findings.bad + findings.warn > 0 ? (
            <Chip tone={findings.bad ? "bad" : "warn"} title="Lo que encontró el último diagnóstico · verlo" onClick={() => onNavigate("machine", "diagnostics")}>
              {b(String(findings.bad + findings.warn))} {findings.bad + findings.warn === 1 ? "aviso" : "avisos"}
            </Chip>
          ) : (
            <Chip tone="ok" title="El último diagnóstico no encontró nada que atender" onClick={() => onNavigate("machine", "diagnostics")}>
              Sin avisos
            </Chip>
          )
        ) : (
          ctx && (
            <Chip tone="neutral" title="Este equipo aún no se ha revisado · hacer un diagnóstico" onClick={() => onNavigate("machine", "diagnostics")}>
              Sin revisar
            </Chip>
          )
        )}
      </div>

      <div className="flex shrink-0 items-center gap-1">
        {appButton("teams", "Teams", MessagesSquare)}
        {appButton("mail", "Correo", Mail)}
        <span className="mx-1 h-5 w-px bg-line" />
        {right}
      </div>
    </div>
  );
}
