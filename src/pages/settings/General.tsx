// Ajustes: Inicio y ventana, Avisos, Datos y copias, y piezas de Seguridad y Portales y red.
import { AlertRules } from "./AlertRules";
import { CheckCircle2, Download, HardDrive, Sparkles, Upload } from "lucide-react";
import { useEffect, useState } from "react";
import { NAV } from "../../components/Sidebar";
import { useToast } from "../../components/feedback";
import { Button, Card, inputClass, Toggle } from "../../components/ui";
import { openOnboarding } from "../../lib/navigate";
import { appcareApi, configApi, logQuietly, type Settings, storageApi, type StorageInfo } from "../../lib/api";
import { exportPrefs, importPrefs, setPrefs, usePrefs } from "../../lib/prefs";
import { DataSafety } from "./DataSafety";
import { AutoBackup } from "./AutoBackup";
import { CompanyConfig } from "../../components/CompanyConfig";
import { showFirstStepsAgain } from "../../components/FirstSteps";
import { ModeCard } from "./About";
import { DataCare } from "./DataCare";
import { Row, selectClass, type SettingsProps } from "./shared";

/** Ajustes → Inicio y ventana: para quién es, qué pasa al abrir y cómo se comporta la ventana. */
export function StartWindow({ s, set }: SettingsProps) {
  const prefs = usePrefs();
  const toast = useToast();
  const pages = NAV.filter((n) => n.id !== "settings");
  return (
    <div className="space-y-4">
      <ModeCard />
      <Card title="Al abrir AdminOps">
        <Row title="Página al abrir AdminOps" sub="La que se muestra al arrancar. De fábrica, el Panel.">
          <select
            value={prefs.startPage}
            onChange={(e) => setPrefs({ startPage: e.target.value as typeof prefs.startPage, startPageChosen: true })}
            className={selectClass}
          >
            <option value="last">La última que usé</option>
            {pages.map((p) => (
              <option key={p.id} value={p.id}>
                {p.label}
              </option>
            ))}
          </select>
        </Row>
        <Row title="Diagnosticar al abrir AdminOps" sub="Analiza el equipo en segundo plano nada más abrir: al entrar en Diagnóstico ya está hecho o a medias.">
          <Toggle checked={prefs.diagnoseOnOpen} onChange={(v) => setPrefs({ diagnoseOnOpen: v })} />
        </Row>
        <Autostart />
        <Row
          title="Volver a ver la bienvenida"
          sub="El asistente de la primera vez: elegir el modo (técnico o usuario), la página de inicio y lo básico de AdminOps. No borra nada de lo que ya tengas configurado."
        >
          <div className="flex flex-wrap gap-2">
            <Button kind="ghost" size="sm" onClick={openOnboarding}>
              <Sparkles size={13} /> Ver la bienvenida
            </Button>
            <Button
              kind="ghost"
              size="sm"
              onClick={() => {
                showFirstStepsAgain();
                toast("ok", "Primeros pasos vuelve a salir en el Panel.");
              }}
            >
              Ver «Primeros pasos»
            </Button>
          </div>
        </Row>
      </Card>
      <Card title="La ventana">
        <Row
          title="Al cerrar la ventana, minimizar"
          sub="La X deja AdminOps minimizada en vez de cerrarla, para no perder lo que tenías abierto: en la barra de tareas o, con el icono junto al reloj, escondida en él. Para cerrarla de verdad aparece el botón «Salir» en la barra de arriba."
        >
          <Toggle checked={s.closeMinimizes} onChange={(v) => set({ closeMinimizes: v })} />
        </Row>
        <Row
          title="Icono junto al reloj"
          sub="Un icono de AdminOps al lado del reloj de Windows. Clic para abrirla; clic derecho para un análisis rápido, la nota de llamada, el mini monitor, los avisos o salir. Windows 11 lo deja al principio en los iconos ocultos (la flecha «^» junto al reloj): arrástralo de ahí a la barra para tenerlo siempre a la vista."
        >
          <Toggle checked={s.trayIcon} onChange={(v) => set({ trayIcon: v })} />
        </Row>
        <Row title="Actualización del Panel" sub="Cada cuánto se refrescan CPU, memoria, red y procesos. Menos frecuente = menos consumo.">
          <select value={prefs.refreshMs} onChange={(e) => setPrefs({ refreshMs: Number(e.target.value) })} className={selectClass}>
            <option value={1000}>Cada segundo</option>
            <option value={2000}>Cada 2 segundos</option>
            <option value={5000}>Cada 5 segundos</option>
          </select>
        </Row>
      </Card>
    </div>
  );
}

/** Ajustes → Avisos: qué vigila AdminOps y cuándo avisa con una notificación de Windows. */
export function AlertSettings({ s, set }: SettingsProps) {
  return (
    <div className="space-y-4">
      <Card title="Errores de Windows">
        <Row
          title="Vigilar errores de Windows"
          sub="Mientras AdminOps está abierta, revisa el Visor de eventos y avisa (campana de arriba y notificación) de pantallazos, discos con fallos, programas que se cierran, falta de memoria o espacio…"
        >
          <Toggle checked={s.watchWindows} onChange={(v) => set({ watchWindows: v })} />
        </Row>
        <Row
          title="Notificaciones de Windows de los avisos"
          sub="Con AdminOps en segundo plano. A la campana llegan siempre todos; aquí se elige de cuáles salta además la notificación."
        >
          <select
            value={s.notifyAlerts}
            onChange={(e) => set({ notifyAlerts: e.target.value as typeof s.notifyAlerts })}
            disabled={!s.watchWindows}
            className={selectClass}
          >
            <option value="all">Todos</option>
            <option value="bad">Solo los graves</option>
            <option value="none">Ninguno</option>
          </select>
        </Row>
      </Card>
      <AlertRules />
      <Card title="Tareas de AdminOps">
        <Row title="Avisar al terminar tareas largas" sub="Notificación de Windows cuando una tarea de más de 20 s acaba con AdminOps en segundo plano.">
          <Toggle checked={s.notifyTasks} onChange={(v) => set({ notifyTasks: v })} />
        </Row>
      </Card>
    </div>
  );
}

/** Ajustes → Datos y copias: dónde están tus datos, sus copias y su limpieza. */
export function DataSettings({ s, set, portable, onImported }: SettingsProps & { portable: boolean; onImported: () => void }) {
  const toast = useToast();
  const doExport = async () => {
    try {
      const p = await configApi.export(exportPrefs());
      if (p) toast("ok", "Configuración exportada.");
    } catch (e) {
      toast("error", String(e));
    }
  };
  const doImport = async () => {
    try {
      const prefsIn = await configApi.import();
      if (prefsIn === null) return;
      importPrefs(prefsIn);
      onImported();
      window.dispatchEvent(new Event("adminops-lock"));
      toast("ok", "Configuración importada.");
    } catch (e) {
      toast("error", String(e));
    }
  };
  return (
    <div className="space-y-4">
      <WhereStored portable={portable} />
      <AutoBackup />
      <DataSafety />
      <DataCare s={s} set={set} part="data" />
      <Card title="Copia de la configuración">
        <p className="mb-3 text-sm text-dim">
          Guarda en un archivo tus ajustes (marca, precios, checklist, firma), los portales de Tickets y las preferencias de la interfaz, para llevarlos a otro
          equipo o recuperarlos. No incluye contraseñas ni clientes.
        </p>
        <div className="flex gap-2">
          <Button kind="ghost" onClick={doExport}>
            <Download size={14} /> Exportar
          </Button>
          <Button kind="ghost" onClick={doImport}>
            <Upload size={14} /> Importar
          </Button>
        </div>
      </Card>
      <CompanyConfig />
    </div>
  );
}

/** Ajustes → Seguridad (junto al bloqueo): la red de seguridad antes de cambiar Windows. */
export function SystemChanges({ s, set }: SettingsProps) {
  return (
    <Card title="Cambios en el sistema">
      <Row
        title="Punto de restauración antes de cambiar el sistema"
        sub="Permite volver atrás con Restaurar sistema si algo sale mal. Crear uno tarda 1–2 minutos (como mucho uno cada 30 min)."
      >
        <select value={s.restorePoints} onChange={(e) => set({ restorePoints: e.target.value as Settings["restorePoints"] })} className={selectClass}>
          <option value="risky">Solo antes de cambios con riesgo</option>
          <option value="always">Antes de cualquier cambio</option>
          <option value="never">Nunca (no recomendado)</option>
        </select>
      </Row>
    </Card>
  );
}

/** Ajustes → Portales y red: el dominio que se propone al unir equipos. */
export function DomainCard({ s, set }: SettingsProps) {
  return (
    <Card title="Red y dominio">
      <Row title="Dominio habitual" sub="Se propone al unir equipos a un dominio, para no escribirlo cada vez.">
        <input value={s.defaultDomain} onChange={(e) => set({ defaultDomain: e.target.value })} placeholder="p. ej. empresa.local" className={`${inputClass} w-56`} />
      </Row>
    </Card>
  );
}

/** Qué se guarda una vez para todos los equipos y qué es de cada equipo. */
function WhereStored({ portable }: { portable: boolean }) {
  const [info, setInfo] = useState<StorageInfo | null>(null);
  const [asked, setAsked] = useState(false);
  const [browserOnUsb, setBrowserOnUsb] = useState<boolean | null>(null);
  const toast = useToast();
  useEffect(() => {
    storageApi.info().then(setInfo).catch(logQuietly("SettingsPage"));
  }, []);

  const travels = [
    "Tus ajustes, marca, precios, checklist y firma",
    "Clientes, contactos, agenda, seguimientos y casos",
    "Portales (Tickets, Correo, Teams…) y sus cuentas guardadas para «Entrar solo», cifradas",
    "Accesos a routers y conexiones de acceso remoto (contraseñas cifradas)",
    "Apariencia, navegación, atajos y favoritos",
    "Informes PDF (todos juntos)",
  ];
  const perPc = [
    "La sesión iniciada en los portales (Correo, Teams…): Windows la cifra para cada equipo. Se entra una vez en cada PC y luego se mantiene (en el disco de ese PC, salvo que elijas el pendrive aquí abajo)",
    "Diario de cambios y «Deshacer» (solo sirven en ese equipo)",
    "Diagnósticos y su comparación",
    "Avisos de Windows y línea de tiempo",
    "Sesión de servicio en curso",
    "Pruebas de velocidad, registro técnico y tamaño de la ventana",
  ];

  const makePortable = async () => {
    try {
      await storageApi.makePortable();
      setAsked(true);
      toast("ok", "Listo. Cierra AdminOps y vuelve a abrirlo: traerá tus datos a la carpeta del programa.");
    } catch (e) {
      toast("error", String(e));
    }
  };

  const reason =
    info?.reason === "removable"
      ? `AdminOps está en un pendrive (${info.drive}): todo se guarda en la carpeta del programa, AdminOps-data.`
      : info?.reason === "marker"
        ? `Todo se guarda en la carpeta del programa (${info?.drive}), en AdminOps-data.`
        : "Instalada en este equipo: los datos están en este equipo.";

  return (
    <Card title={portable ? "Tus datos viajan con AdminOps" : "Dónde se guardan tus datos"}>
      <p className="mb-2 text-sm text-dim">{reason}</p>
      {portable && (
        <p className="mb-3 text-xs text-dim">
          Para actualizar, vuelve a pasar el instalador sobre la misma carpeta: los datos no se tocan (el instalador solo cambia los archivos del programa). Lo tuyo
          se configura una vez y te acompaña; lo de cada equipo se separa por su nombre.
        </p>
      )}
      {portable && info && (
        <div className="mb-3 rounded-lg border border-line bg-panel-2 p-3">
          <label className="flex items-start gap-2 text-sm text-ink">
            <input
              type="checkbox"
              checked={browserOnUsb ?? info.browserOnUsb}
              onChange={(e) => {
                const on = e.target.checked;
                storageApi
                  .setBrowserOnUsb(on)
                  .then(() => {
                    setBrowserOnUsb(on);
                    toast("ok", "Se aplicará al volver a abrir AdminOps.");
                  })
                  .catch((err) => toast("error", String(err)));
              }}
              className="mt-1 accent-[var(--color-neon)]"
            />
            <span>
              Guardar también el navegador interno en el pendrive
              <span className="block text-xs text-dim">
                No deja nada de Correo, Teams ni Tickets en el disco del equipo (para equipos de clientes), pero en un pendrive van mucho más lentos: páginas en
                blanco y un arranque de varios segundos más. Desactivado, cada equipo guarda su sesión en su propio disco, que es donde de todas formas vale.
              </span>
            </span>
          </label>
        </div>
      )}
      {info?.migrated && (
        <p className="mb-3 flex items-start gap-1.5 text-xs text-ok">
          <CheckCircle2 size={13} className="mt-0.5 shrink-0" />
          Datos traídos del equipo {info.migrated.fromHost} el {new Date(info.migrated.at * 1000).toLocaleDateString("es")}: {info.migrated.files} archivos
          {info.migrated.secrets > 0 && `, ${info.migrated.secrets} contraseñas cifradas de nuevo para el pendrive`}
          {info.migrated.browser && ", y la sesión de los portales de ese equipo"}.
        </p>
      )}
      {!portable && info && (
        <div className="mb-3 rounded-lg border border-line bg-panel-2 p-3">
          <p className="text-xs text-dim">
            Si llevas AdminOps en un pendrive (instalado en él o el portable), tus datos van con él: instálalo en el pendrive y lo detecta solo. Para usar la carpeta
            del programa en cualquier otra unidad (un disco externo), actívalo aquí: al volver a abrir AdminOps se traen tus datos de este equipo y las contraseñas
            se cifran con la clave de esa carpeta.
          </p>
          <div className="mt-2 flex flex-wrap items-center gap-2">
            <Button kind="ghost" onClick={() => void makePortable()} disabled={!info.canSwitch || asked}>
              <HardDrive size={14} /> Guardar todo en la carpeta del programa
            </Button>
            {!info.canSwitch && <span className="text-[11px] text-mute">La carpeta del programa no se puede escribir (Archivos de programa): abre AdminOps como administrador.</span>}
            {asked && <span className="text-[11px] text-ok">Se aplicará al volver a abrir AdminOps.</span>}
          </div>
        </div>
      )}
      <div className="grid gap-4 md:grid-cols-2">
        <div>
          <p className="mb-1.5 text-xs font-medium text-ok">{portable ? "Viaja contigo (igual en todos)" : "Tuyo (viaja si guardas en la carpeta del programa)"}</p>
          <ul className="space-y-1 text-xs text-dim">
            {travels.map((t) => (
              <li key={t}>· {t}</li>
            ))}
          </ul>
        </div>
        <div>
          <p className="mb-1.5 text-xs font-medium text-neon">Propio de cada equipo</p>
          <ul className="space-y-1 text-xs text-dim">
            {perPc.map((t) => (
              <li key={t}>· {t}</li>
            ))}
          </ul>
        </div>
      </div>
      {portable && (
        <p className="mt-3 text-xs text-mute">
          Las contraseñas guardadas van cifradas con una clave de la propia carpeta: funcionan en cualquier equipo, pero quien se lleve el pendrive entero podría
          leerlas. Activa el bloqueo con PIN en Seguridad y haz de vez en cuando una copia cifrada (aquí abajo): si pierdes el pendrive, lo pierdes todo.
        </p>
      )}
    </Card>
  );
}

function Autostart() {
  const [on, setOn] = useState<boolean | null>(null);
  const toast = useToast();
  useEffect(() => {
    appcareApi
      .autostart()
      .then(setOn)
      .catch(() => setOn(false));
  }, []);
  const toggle = async (v: boolean) => {
    try {
      await appcareApi.setAutostart(v);
      setOn(v);
      toast(
        "ok",
        v
          ? "AdminOps se abrirá minimizada al iniciar sesión."
          : "AdminOps ya no se abre al iniciar sesión.",
      );
    } catch (e) {
      toast("error", String(e));
    }
  };
  return (
    <Row
      title="Abrir AdminOps al iniciar Windows"
      sub="Minimizada en la barra de tareas, sin pedir permiso de administrador cada vez. En modo portable, si mueves la carpeta, vuelve a activarlo."
    >
      <Toggle checked={!!on} disabled={on === null} onChange={(v) => toggle(v)} />
    </Row>
  );
}
