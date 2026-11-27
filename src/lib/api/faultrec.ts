import { invoke } from "./core";

// ---------- Grabar el fallo (Estado del equipo → Grabar el fallo) ----------

export type FaultMarkKind = "disk" | "cpu" | "hang" | "error";

/** Un punto de la línea de tiempo de una grabación. */
export interface FaultMark {
  /** Segundo del vídeo en que empieza. */
  t: number;
  kind: FaultMarkKind;
  title: string;
  detail: string;
  /** La marca de disco o procesador que más probablemente provocó un «no responde» o un error de programa. */
  probable?: boolean;
  /** Un programa que falló o dejó de responder. */
  app?: boolean;
}

/** Una grabación guardada en este equipo. */
export interface FaultRecording {
  id: string;
  /** Segundos desde 1970 del primer fotograma. */
  started: number;
  /** Lo que dura el vídeo, en segundos. */
  secs: number;
  monitor: string;
  width: number;
  height: number;
  /** El caso abierto al empezar; sin él, se borra al cerrar AdminOps. */
  caseId: string | null;
  marks: FaultMark[];
  bytes: number;
  /** Algo que el técnico debe saber (p. ej. que faltan las marcas de errores). */
  note: string | null;
  /** Qué se grabó: «Pantalla entera», «Ventana · Excel», «Zona de 800 × 600» (vacío en las de antes de la 1.4). */
  target?: string;
  /** Se apuntaron los clics y las teclas. */
  inputOn?: boolean;
  /** Los clics y las teclas, por orden. Las letras escritas nunca se guardan: salen como «•••». */
  input?: FaultInput[];
  /** Lleva la voz del micrófono. */
  mic?: boolean;
  /** Volumen de la voz por décima de segundo, de 0 a 100. */
  voice?: number[];
  /** «record» a mano, «replay» con «¡Acaba de pasar!», «auto» guardada sola al vigilar. */
  origin?: "record" | "replay" | "auto" | "";
  /** Qué disparó la automática: «Excel no responde». */
  trigger?: string | null;
  /** Para «No volver a grabar por esto»: «app:excel.exe», «cpu», «disk». */
  triggerKey?: string | null;
  /** Las pistas del reproductor, un valor por segundo (vacío en las de antes de la 1.4). */
  samples?: FaultSample[];
}

/** Un segundo de las pistas: procesador, disco y memoria en %, red en bytes por segundo. */
export interface FaultSample {
  t: number;
  cpu: number;
  disk: number | null;
  mem: number | null;
  net: number | null;
}

/** Lo que dejó «Exportar MP4». */
export interface FaultExported {
  path: string;
  ms: number;
}

/** Un clic o unas teclas del usuario durante la grabación. */
export interface FaultInput {
  /** Milisegundos desde el principio del vídeo. */
  t: number;
  kind: "click" | "keys";
  /** Clics: dónde, de 0 a 1 dentro de lo grabado. */
  x: number;
  y: number;
  /** «Clic», «Doble clic», «Ctrl + P», «Intro», «•••». */
  label: string;
}

export interface FaultRecordingDetail {
  meta: FaultRecording;
  /** El trozo que va en el informe del cliente (milisegundos), si se mandó. */
  clip?: [number, number] | null;
  /** Ruta del vídeo, solo para reproducirlo (`convertFileSrc`); no se enseña. Vacía en la demo. */
  path: string;
}

export interface FaultRecStatus {
  recording: boolean;
  /** Parado y preparando las marcas. */
  finishing: boolean;
  secs: number;
  maxSecs: number;
  id: string | null;
  /** El fallo de la última grabación (se da una sola vez). */
  error: string | null;
  /** Se está grabando la voz. */
  mic?: boolean;
}

/** Una zona de un monitor, en píxeles físicos desde su esquina superior izquierda. */
export interface FaultRegion {
  x: number;
  y: number;
  w: number;
  h: number;
}

/** Cómo se graba (lo que no es el monitor). */
export interface FaultStartOptions {
  /** Solo esta ventana (el id de `windows()`). */
  window?: number | null;
  /** Solo esta zona del monitor elegido. */
  region?: FaultRegion | null;
  /** Apuntar clics y teclas. */
  input: boolean;
  /** Grabar el micrófono. */
  mic: boolean;
}

/** Una ventana abierta que se puede grabar. El título solo se enseña en el selector; no se guarda. */
export interface FaultRecWindow {
  id: number;
  title: string;
  /** El programa: «Excel», «Microsoft Edge». */
  app: string;
  minimized: boolean;
}

/** Lo que no se vuelve a grabar solo. */
export interface FaultIgnored {
  key: string;
  label: string;
}

/** La grabación automática al vigilar (apagada de serie). */
export interface FaultAutoSettings {
  enabled: boolean;
  /** Un programa deja de responder. */
  hang: boolean;
  /** Error de aplicación en el registro de Windows. */
  appError: boolean;
  /** Procesador o disco al 100 % más de 20 s. */
  busy: boolean;
  ignored: FaultIgnored[];
}

/** Vigilar: el último minuto en memoria. */
export interface FaultWatchStatus {
  on: boolean;
  /** Segundos Unix desde que se vigila. */
  since: number | null;
  /** Cuántos segundos hay ya en memoria (hasta 75). */
  bufferedSecs: number;
  monitor: string;
  auto: FaultAutoSettings;
  /** Apagado un momento porque se está grabando a mano; vuelve solo al terminar. */
  pausedForRecording: boolean;
  /** Ctrl+Alt+G está disponible (otro programa puede tenerlo). */
  hotkey?: boolean;
  /** Automáticas que no se grabaron hoy por los límites. */
  skippedToday?: number;
  /** Por qué se apagó solo (una vez). */
  error?: string | null;
}

/** Lo que avisa al guardarse una automática. */
export interface FaultAutoSaved {
  id: string;
  trigger: string;
  at: number;
}

/** Lo que manda la ventana de dibujar la zona al terminar (`region` null: cancelada). */
export interface FaultRegionPicked {
  monitor: string | null;
  region: FaultRegion | null;
}

export interface FaultRecMonitor {
  /** La misma ruta de dispositivo que usa «Pantallas». */
  id: string;
  name: string;
  number: number;
  primary: boolean;
  width: number;
  height: number;
}

/** Los avisos que manda el programa mientras se graba y al terminar. */
export const FAULTREC_TICK_EVENT = "faultrec://tick";
export const FAULTREC_DONE_EVENT = "faultrec://done";
export const FAULTREC_REGION_EVENT = "faultrec://region";
export const FAULTREC_AUTO_EVENT = "faultrec://auto";
/** Se pulsó el aviso de Windows de una grabación: `{ id }`. */
export const FAULTREC_OPEN_EVENT = "faultrec://open";

export interface FaultRecDone {
  id: string | null;
  error: string | null;
  /** Guardado con «¡Acaba de pasar!» (Ctrl+Alt+G). */
  replay?: boolean;
}

export const faultRecApi = {
  monitors: () => invoke<FaultRecMonitor[]>("faultrec_monitors"),
  /** Minimiza AdminOps, abre el contador y empieza a grabar el monitor (el principal si no se dice). */
  start: (monitor?: string | null, opts?: FaultStartOptions | null) => invoke<FaultRecStatus>("faultrec_start", { monitor: monitor || null, opts: opts ?? null }),
  /** Las ventanas abiertas, la de arriba primero. */
  windows: () => invoke<FaultRecWindow[]>("faultrec_windows"),
  /** Abre encima del monitor la capa para dibujar la zona; la respuesta llega con FAULTREC_REGION_EVENT. */
  pickRegion: (monitor?: string | null) => invoke<void>("faultrec_pick_region", { monitor: monitor || null }),
  /** La capa de la zona: terminado (o cancelado con null). */
  regionDone: (region: FaultRegion | null) => invoke<void>("faultrec_region_done", { region }),
  status: () => invoke<FaultRecStatus>("faultrec_status"),
  /** Para, guarda y trae AdminOps al frente. */
  stop: () => invoke<FaultRecording>("faultrec_stop"),
  list: () => invoke<FaultRecording[]>("faultrec_list"),
  get: (id: string) => invoke<FaultRecordingDetail>("faultrec_get", { id }),
  remove: (id: string) => invoke<void>("faultrec_delete", { id }),
  /** Vigilar: el último minuto queda en memoria hasta «¡Acaba de pasar!». */
  watchStart: (monitor: string | null, input: boolean) => invoke<FaultWatchStatus>("faultrec_watch_start", { monitor: monitor || null, opts: { input } }),
  watchStop: () => invoke<FaultWatchStatus>("faultrec_watch_stop"),
  watchStatus: () => invoke<FaultWatchStatus>("faultrec_watch_status"),
  /** Guarda el último minuto. */
  justHappened: () => invoke<FaultRecording>("faultrec_just_happened"),
  /** La miniatura de la lista (JPEG como data URL; se hace una vez y se guarda junto al vídeo). */
  thumb: (id: string) => invoke<string>("faultrec_thumb", { id }),
  /** Al caso abierto; devuelve si ya estaba en él. */
  attachCase: (id: string) => invoke<boolean>("faultrec_attach_case", { id }),
  /** El vídeo como archivo al portapapeles. */
  copyFile: (id: string) => invoke<void>("faultrec_copy_file", { id }),
  openFile: (id: string) => invoke<void>("faultrec_open_file", { id }),
  /** «Al informe»: tres fotogramas del trozo y sus marcas, para el PDF del cliente. */
  toReport: (id: string, fromMs: number, toMs: number) => invoke<unknown>("faultrec_to_report", { id, fromMs: Math.round(fromMs), toMs: Math.round(toMs) }),
  reportRemove: (id: string) => invoke<void>("faultrec_report_remove", { id }),
  /** «Exportar MP4»: pregunta dónde y copia el trozo sin recodificar; null si se canceló. */
  exportClip: (id: string, fromMs: number, toMs: number) => invoke<FaultExported | null>("faultrec_export", { id, fromMs: Math.round(fromMs), toMs: Math.round(toMs) }),
  autoGet: () => invoke<FaultAutoSettings>("faultrec_auto_get"),
  autoSet: (settings: FaultAutoSettings) => invoke<FaultAutoSettings>("faultrec_auto_set", { settings }),
  autoIgnore: (key: string, label: string) => invoke<void>("faultrec_auto_ignore", { key, label }),
  autoUnignore: (key: string) => invoke<void>("faultrec_auto_unignore", { key }),
};
