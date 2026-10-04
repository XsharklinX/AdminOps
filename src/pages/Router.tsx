import { listen } from "@tauri-apps/api/event";
import {
  ArrowLeft,
  ArrowRight,
  Copy,
  ExternalLink,
  Eye,
  EyeOff,
  House,
  KeyRound,
  Loader2,
  QrCode,
  RotateCw,
  Router as RouterIcon,
  Save,
  SquareArrowOutUpRight,
  Wifi,
  X,
} from "lucide-react";
import { useCallback, useEffect, useLayoutEffect, useRef, useState } from "react";
import { useToast } from "../components/feedback";
import { Button, Card, inputClass, Loading, Modal, iconBtn } from "../components/ui";
import { lanApi, portalsApi, type LanInfo, type PublicIp, type RouterCheck, type RouterProfile } from "../lib/api";
import { windowRect } from "../lib/prefs";
import { stowPortal } from "../lib/portalState";

// Con el zoom de la interfaz aplicado: la vista web va en píxeles de la ventana.
const rectOf = windowRect;

const DOT = { ok: "bg-ok", info: "bg-mute", warn: "bg-warn", bad: "bg-bad" } as const;

const AUTH: Record<string, string> = { open: "Abierta (sin contraseña)", WPA2PSK: "WPA2", WPA3SAE: "WPA3", WPAPSK: "WPA", WPA2: "WPA2 empresarial", WPA3: "WPA3 empresarial" };

/** Texto oculto con botón de mostrar y copiar (contraseñas, IP pública). */
function Secret({ value, label }: { value: string; label: string }) {
  const [shown, setShown] = useState(false);
  const toast = useToast();
  if (!value) return <span className="text-mute">—</span>;
  return (
    <span className="inline-flex items-center gap-2">
      <span className="font-mono text-[13px] text-ink select-text">{shown ? value : "••••••••••"}</span>
      <button onClick={() => setShown(!shown)} className="text-mute hover:text-ink" title={shown ? "Ocultar" : "Mostrar"}>
        {shown ? <EyeOff size={13} /> : <Eye size={13} />}
      </button>
      <button onClick={() => navigator.clipboard.writeText(value).then(() => toast("ok", `${label} copiada.`))} className="text-mute hover:text-ink" title="Copiar">
        <Copy size={13} />
      </button>
    </span>
  );
}

function Row({ label, children }: { label: string; children: React.ReactNode }) {
  return (
    <div className="flex items-baseline gap-4 border-t border-line/60 py-2 first:border-t-0">
      <span className="w-40 shrink-0 text-[13px] text-dim">{label}</span>
      <span className="min-w-0 flex-1 text-[13px] text-ink">{children}</span>
    </div>
  );
}

/** `covered`: hay un diálogo de la app encima (la vista web nativa lo taparía). */
export function Router({ covered = false }: { covered?: boolean }) {
  const [info, setInfo] = useState<LanInfo | null | undefined>(undefined);
  const [check, setCheck] = useState<RouterCheck | null>(null);
  const [checking, setChecking] = useState(false);
  const [pub, setPub] = useState<PublicIp | null | "error">(null);
  const [profile, setProfile] = useState<RouterProfile | null>(null);
  const [dirty, setDirty] = useState(false);
  const [panel, setPanel] = useState<string | null>(null);
  const [loading, setLoading] = useState(false);
  const [qr, setQr] = useState<string | null>(null);
  const area = useRef<HTMLDivElement>(null);
  const toast = useToast();

  const load = useCallback(async () => {
    setInfo(undefined);
    setCheck(null);
    try {
      const i = await lanApi.info();
      setInfo(i);
      if (!i) return;
      lanApi
        .publicIp()
        .then(setPub)
        .catch(() => setPub("error"));
      const saved = (await lanApi.routers()).find((r) => r.key === i.key);
      setChecking(true);
      const c = await lanApi.check(i.gateway, i.ssid ? i.wifiAuth : "").catch((e) => {
        toast("error", String(e));
        return null;
      });
      setChecking(false);
      setCheck(c);
      setProfile(
        saved ?? {
          key: i.key,
          name: c?.brand ? `Router ${c.brand}` : i.network || "Router",
          url: c?.url ?? `http://${i.gateway}/`,
          username: "",
          password: "",
          notes: "",
          updated: 0,
          locked: false,
        },
      );
      setDirty(false);
    } catch (e) {
      setInfo(null);
      toast("error", String(e));
    }
  }, [toast]);

  useEffect(() => {
    void load();
  }, [load]);

  // Al cerrar la página, la vista del router (nativa, por encima de todo) se oculta.
  const panelRef = useRef<string | null>(null);
  panelRef.current = panel;
  useEffect(() => () => void (panelRef.current && stowPortal(panelRef.current)), []);

  useEffect(() => {
    if (!panel) return;
    const un = listen<{ id: string; loading: boolean }>("portal-load", (e) => {
      if (e.payload.id === panel) setLoading(e.payload.loading);
    });
    return () => {
      void un.then((f) => f());
    };
  }, [panel]);

  const overlay = covered || qr !== null;
  useLayoutEffect(() => {
    const el = area.current;
    // Solo la vista de esta página: la de Tickets puede estar viva en otra.
    if (!panel || !el || overlay) {
      if (panel) stowPortal(panel);
      return;
    }
    portalsApi.show(panel, rectOf(el)).catch((e) => toast("error", String(e)));
    const sync = () => portalsApi.bounds(panel, rectOf(el));
    const ro = new ResizeObserver(sync);
    ro.observe(el);
    window.addEventListener("resize", sync);
    return () => {
      ro.disconnect();
      window.removeEventListener("resize", sync);
    };
  }, [panel, overlay, toast]);

  const save = async (p = profile) => {
    if (!p) return null;
    try {
      const saved = await lanApi.saveRouter(p);
      setProfile(saved);
      setDirty(false);
      return saved;
    } catch (e) {
      toast("error", String(e));
      return null;
    }
  };

  const openPanel = async (where: "embedded" | "window" | "browser") => {
    if (!info || !profile) return;
    try {
      const saved = dirty || !profile.updated ? await save() : profile;
      if (!saved) return;
      const portal = await lanApi.routerPortal(info.key, saved.name, saved.url);
      if (where === "embedded") setPanel(portal.id);
      else if (where === "window") await portalsApi.openWindow(portal.id);
      else await portalsApi.openExternal(portal.id);
    } catch (e) {
      toast("error", String(e));
    }
  };

  const showQr = async () => {
    if (!info?.ssid) return;
    try {
      setQr(await lanApi.wifiQr(info.ssid, info.wifiPassword ?? "", info.wifiAuth));
    } catch (e) {
      toast("error", String(e));
    }
  };

  const copy = (text: string, what: string) => navigator.clipboard.writeText(text).then(() => toast("ok", `${what} copiado.`), () => toast("error", "No se pudo copiar."));
  const set = (patch: Partial<RouterProfile>) => {
    if (!profile) return;
    setProfile({ ...profile, ...patch });
    setDirty(true);
  };

  if (info === undefined) return <Loading page text="Leyendo la red…" />;
  if (info === null)
    return (
      <div className="mx-auto max-w-3xl p-6">
        <Card title="Sin red">
          <p className="text-sm text-dim">El equipo no está conectado a ninguna red con router (o la conexión no tiene puerta de enlace).</p>
          <div className="mt-3">
            <Button kind="ghost" onClick={load}>
              <RotateCw size={14} /> Volver a comprobar
            </Button>
          </div>
        </Card>
      </div>
    );

  // Panel del router dentro de la app, a toda la altura.
  if (panel) {
    const nav = (a: "back" | "forward" | "reload" | "home") => portalsApi.nav(panel, a).catch((e) => toast("error", String(e)));
    return (
      <div className="flex h-full flex-col">
        <div className="flex items-center gap-1 border-b border-line px-4 py-2">
          <span className="mr-2 flex items-center gap-1.5 text-sm text-ink">
            <RouterIcon size={14} className="text-mute" /> {profile?.name}
          </span>
          <button onClick={() => nav("back")} className={iconBtn} title="Atrás">
            <ArrowLeft size={15} />
          </button>
          <button onClick={() => nav("forward")} className={iconBtn} title="Adelante">
            <ArrowRight size={15} />
          </button>
          <button onClick={() => nav("reload")} className={iconBtn} title="Recargar">
            {loading ? <Loader2 size={15} className="animate-spin text-neon" /> : <RotateCw size={15} />}
          </button>
          <button onClick={() => nav("home")} className={iconBtn} title="Página inicial del panel">
            <House size={15} />
          </button>
          <div className="mx-auto flex items-center gap-2 text-xs">
            {profile?.username && (
              <button onClick={() => copy(profile.username, "Usuario")} className="flex items-center gap-1 rounded-md border border-line px-2 py-1 text-dim hover:text-ink">
                <Copy size={11} /> Usuario
              </button>
            )}
            {profile?.password && (
              <button onClick={() => copy(profile.password, "Contraseña")} className="flex items-center gap-1 rounded-md border border-line px-2 py-1 text-dim hover:text-ink">
                <KeyRound size={11} /> Contraseña
              </button>
            )}
          </div>
          <button onClick={() => openPanel("window")} className={iconBtn} title="Abrir en una ventana aparte">
            <SquareArrowOutUpRight size={15} />
          </button>
          <button onClick={() => openPanel("browser")} className={iconBtn} title="Abrir en el navegador">
            <ExternalLink size={15} />
          </button>
          <button
            onClick={() => {
              stowPortal(panel);
              setPanel(null);
            }}
            className={iconBtn}
            title="Cerrar el panel"
          >
            <X size={15} />
          </button>
        </div>
        <div ref={area} className="relative flex-1 bg-void">
          {overlay && <p className="p-8 text-sm text-mute">El panel del router vuelve al cerrar el diálogo.</p>}
        </div>
      </div>
    );
  }

  const wifiAuth = AUTH[info.wifiAuth] ?? info.wifiAuth;

  return (
    <div className="mx-auto grid max-w-6xl grid-cols-12 gap-4 p-6">
      <Card title="Esta red" className="col-span-12 lg:col-span-7">
        <Row label="Red">{info.ssid ?? (info.network || "—")}</Row>
        <Row label="Conexión">
          {info.wireless ? "Wi-Fi" : "Cable"} · {info.adapter}
          {info.linkSpeed && <span className="text-mute"> · {info.linkSpeed}</span>}
        </Row>
        <Row label="IP de este equipo">
          <span className="font-mono">
            {info.ip}/{info.prefix}
          </span>
          <span className="text-mute"> · {info.dhcp ? "automática (DHCP)" : "fija"}</span>
        </Row>
        <Row label="Router (puerta de enlace)">
          <span className="font-mono">{info.gateway}</span>
          {info.gatewayMac && <span className="font-mono text-[11px] text-mute"> · {info.gatewayMac}</span>}
        </Row>
        <Row label="DNS">{info.dns.length ? <span className="font-mono">{info.dns.join(", ")}</span> : "—"}</Row>
        <Row label="IP pública">
          {pub === null ? (
            <span className="text-mute">Consultando…</span>
          ) : pub === "error" ? (
            <span className="text-mute">No disponible</span>
          ) : (
            <>
              <Secret value={pub.ip} label="IP pública" />
              <span className="block text-xs text-mute">
                {[pub.org, [pub.city, pub.country].filter(Boolean).join(", ")].filter(Boolean).join(" · ")}
              </span>
            </>
          )}
        </Row>
        {info.category === "Public" && (
          <p className="mt-2 text-xs text-warn">Windows trata esta red como pública: compartir archivos e impresoras está bloqueado. Si es tu casa u oficina, márcala como privada.</p>
        )}
      </Card>

      <Card title="Router" icon={<RouterIcon size={14} />} className="col-span-12 lg:col-span-5">
        {checking && !check ? (
          <p className="flex items-center gap-2 text-sm text-mute">
            <Loader2 size={14} className="animate-spin" /> Revisando el router…
          </p>
        ) : (
          <>
            <div className="text-sm text-ink">{check?.brand ?? "Marca no reconocida"}</div>
            {check?.title && <div className="truncate text-xs text-mute">{check.title}</div>}
            <div className="mt-3 flex flex-wrap gap-2">
              <Button onClick={() => openPanel("embedded")}>
                <RouterIcon size={14} /> Abrir el panel del router
              </Button>
              <Button kind="ghost" onClick={() => openPanel("window")} title="Ventana aparte">
                <SquareArrowOutUpRight size={14} />
              </Button>
              <Button kind="ghost" onClick={() => openPanel("browser")} title="En el navegador">
                <ExternalLink size={14} />
              </Button>
            </div>
            {check && check.notes.length > 0 && (
              <ul className="mt-4 space-y-2">
                {check.notes.map((n, i) => (
                  <li key={i} className="flex gap-2.5 text-[13px] text-dim">
                    <span className={`mt-1.5 size-1.5 shrink-0 rounded-full ${DOT[n.level]}`} />
                    <span>{n.text}</span>
                  </li>
                ))}
              </ul>
            )}
            {check && check.openPorts.length > 0 && <p className="mt-3 text-[11px] text-mute">Puertos abiertos en el router: {check.openPorts.join(", ")}</p>}
          </>
        )}
      </Card>

      {profile && (
        <Card title="Acceso al router" icon={<KeyRound size={14} />} className="col-span-12 lg:col-span-7">
          <div className="grid grid-cols-2 gap-3">
            <label className="block">
              <span className="mb-1 block text-xs text-dim">Nombre</span>
              <input value={profile.name} onChange={(e) => set({ name: e.target.value })} className={inputClass} />
            </label>
            <label className="block">
              <span className="mb-1 block text-xs text-dim">Dirección del panel</span>
              <input value={profile.url} onChange={(e) => set({ url: e.target.value })} className={`${inputClass} font-mono`} />
            </label>
            <label className="block">
              <span className="mb-1 block text-xs text-dim">Usuario</span>
              <input value={profile.username} onChange={(e) => set({ username: e.target.value })} autoComplete="off" className={inputClass} />
            </label>
            <label className="block">
              <span className="mb-1 block text-xs text-dim">Contraseña</span>
              <input
                type="password"
                value={profile.password}
                onChange={(e) => set({ password: e.target.value, locked: false })}
                placeholder={profile.locked ? "Guardada en otro equipo: escríbela de nuevo" : ""}
                autoComplete="new-password"
                className={inputClass}
              />
            </label>
            <label className="col-span-2 block">
              <span className="mb-1 block text-xs text-dim">Notas</span>
              <textarea
                value={profile.notes}
                onChange={(e) => set({ notes: e.target.value })}
                rows={2}
                placeholder="Contraseña de la Wi-Fi de invitados, puertos abiertos, contacto del proveedor…"
                className={`${inputClass} resize-y`}
              />
            </label>
          </div>
          <div className="mt-3 flex items-center justify-between gap-3">
            <p className="text-[11px] text-mute">La contraseña se guarda cifrada con tu usuario de Windows: solo tú, en este equipo, puedes leerla.</p>
            <Button onClick={() => save().then((r) => r && toast("ok", "Acceso guardado."))} disabled={!dirty}>
              <Save size={14} /> Guardar
            </Button>
          </div>
          {check?.defaultHint && !profile.username && <p className="mt-3 rounded-md bg-panel-2 px-3 py-2 text-xs text-dim">{check.defaultHint}</p>}
        </Card>
      )}

      {info.ssid && (
        <Card title="Wi-Fi" icon={<Wifi size={14} />} className="col-span-12 lg:col-span-5">
          <Row label="Nombre (SSID)">{info.ssid}</Row>
          <Row label="Seguridad">{wifiAuth || "—"}</Row>
          <Row label="Contraseña">
            {info.wifiPassword ? <Secret value={info.wifiPassword} label="Contraseña" /> : <span className="text-mute">No disponible</span>}
          </Row>
          <div className="mt-3">
            <Button kind="ghost" onClick={showQr}>
              <QrCode size={14} /> Código QR para conectar
            </Button>
          </div>
        </Card>
      )}

      {qr && (
        <Modal title={`Conectarse a «${info.ssid}»`} onClose={() => setQr(null)} width="w-[360px]">
          {/* SVG generado por AdminOps (solo rectángulos): no incluye texto del usuario. */}
          <div className="mx-auto w-60 overflow-hidden rounded-lg bg-white p-2 [&_svg]:h-auto [&_svg]:w-full" dangerouslySetInnerHTML={{ __html: qr }} />
          <p className="mt-3 text-center text-xs text-dim">Apunta la cámara del móvil al código para conectarte sin escribir la contraseña.</p>
        </Modal>
      )}
    </div>
  );
}
