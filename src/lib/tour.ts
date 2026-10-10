// Recorrido guiado tras actualizar: unos pocos pasos que señalan lo nuevo en la
// propia pantalla (oscurece el resto). Se salta con Esc y se vuelve a ver desde
// Ctrl+K → «recorrido de novedades». Une las marcas «Nuevo» y el registro de
// cambios en un solo gesto.

export interface TourStep {
  /** Dónde está lo que se enseña (`[data-tour=…]`); sin él, el paso va en el centro. */
  target?: string;
  title: string;
  text: string;
}

export const TOURS: Record<string, TourStep[]> = {
  "1.2.9": [
    { title: "AdminOps 1.2.9: más cuidada y más segura", text: "Cuatro cosas en un minuto. Esc lo salta; puedes volver a verlo desde Ctrl+K → «recorrido»." },
    { target: "sidebar", title: "El resaltado se desliza", text: "Tarjetas con profundidad, pestañas que se estiran y colores nuevos en Ajustes → Apariencia (pasa el ratón por uno para verlo en vivo)." },
    { target: "search", title: "Teclado y F11", text: "J/K recorren las tablas, Alt+↑/↓ reordena y F11 abre la pantalla de taller: cifras enormes para dejar en un monitor." },
    { target: "case", title: "Informes con portada", text: "Con tu marca, un semáforo grande y lo más urgente por orden. También hay «Una página» para entregar en mano." },
    { title: "Seguridad", text: "En Ajustes → Seguridad: cifrar tus datos, ver qué sale de este equipo y, en Acerca de, los errores de AdminOps." },
  ],
  "1.2.8": [
    { title: "AdminOps 1.2.8: más cómoda y con más respuestas", text: "Cuatro cosas en un minuto. Esc lo salta; puedes volver a verlo desde Ctrl+K → «recorrido»." },
    { target: "search", title: "Ctrl+K entiende frases y erratas", text: "Escribe «impresora no imprime», «dicsos» o pega un código como 0x80070005: te lleva a la solución, a la herramienta o a la explicación." },
    { target: "undo", title: "Deshacer siempre a mano", text: "Lo último que cambió AdminOps, a un clic, y también con Ctrl+Z fuera de los campos de texto." },
    { target: "case", title: "Clic derecho y «Enviar a…»", text: "Clic derecho sobre cualquier cosa para copiarla, abrirla en el Explorador o abrir un caso. En cada tarjeta, el avión de papel la manda al caso, a un correo, a Teams o a Excel." },
    { target: "sidebar", title: "Más problemas con respuesta", text: "Outlook, OneDrive, perfiles temporales, Windows Update, licencias, navegadores, USB… y diagnóstico de experto en Estado del equipo." },
  ],
};

/** El recorrido de una versión, si lo tiene (se ignora el sufijo «-beta»…). */
export function tourFor(version: string): TourStep[] | null {
  return TOURS[version.split("-")[0]] ?? null;
}

const EVENT = "adminops-tour";

/** Abre el recorrido de una versión (o el de la actual). */
export function startTour(version?: string) {
  window.dispatchEvent(new CustomEvent<string | undefined>(EVENT, { detail: version }));
}

export function onTour(cb: (version?: string) => void): () => void {
  const f = (e: Event) => cb((e as CustomEvent<string | undefined>).detail);
  window.addEventListener(EVENT, f);
  return () => window.removeEventListener(EVENT, f);
}
