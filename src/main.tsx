import React from "react";
import ReactDOM from "react-dom/client";
import App from "./App";
import { ErrorBoundary } from "./components/ErrorBoundary";
import { QuickNote } from "./components/QuickNote";
import { MiniMonitor } from "./components/MiniMonitor";
import { appApi } from "./lib/api";
import { showFullTextOnHover } from "./lib/fullTextOnHover";
import { applyAppearance } from "./lib/prefs";
import { applyTheme, getTheme, watchTheme } from "./lib/theme";
import { watchSpotlight } from "./lib/spotlight";
import { watchCopy } from "./lib/copyFlash";
import "./index.css";

// ¿Es la ventanita de la nota de llamada (Ctrl+Alt+N) y no la aplicación?
// Lo marca el programa al crearla (quicknote.rs).
const flags = window as unknown as { __ADMINOPS_NOTE__?: boolean; __ADMINOPS_MONITOR__?: boolean };
const isMonitor = flags.__ADMINOPS_MONITOR__ === true;
// Para el arranque, la nota y el mini monitor son lo mismo: una ventanita, no la aplicación.
const isNote = flags.__ADMINOPS_NOTE__ === true || isMonitor;

// Lo primero de todo: decirle al programa que el código ya se está ejecutando.
// En un equipo viejo pueden pasar diez segundos largos de aquí a la primera
// pantalla, y sin esta señal el vigilante del arranque lo tomaría por WebView2
// muerto y recargaría, dejándolo peor. (La nota no es la ventana principal.)
if (!isNote) void appApi.uiBooting();

applyTheme(getTheme());
watchTheme();
watchSpotlight();
watchCopy();
// El tamaño de la interfaz se aplica a la ventana principal; la nota no lo necesita.
if (!isNote) applyAppearance();

// Sin menú contextual del navegador: esto es una app de escritorio, no una web.
document.addEventListener("contextmenu", (e) => e.preventDefault());

// El texto recortado con «…» se puede leer entero pasando el ratón por encima.
showFullTextOnHover();

// Errores fuera de React (eventos, promesas sin capturar): al registro técnico.
window.addEventListener("error", (e) => appApi.logError(`${e.message} @ ${e.filename}:${e.lineno}`));
window.addEventListener("unhandledrejection", (e) => appApi.logError(`Promesa sin capturar: ${String(e.reason)}`));

ReactDOM.createRoot(document.getElementById("root") as HTMLElement).render(
  <React.StrictMode>
    <ErrorBoundary>{isMonitor ? <MiniMonitor /> : isNote ? <QuickNote /> : <App />}</ErrorBoundary>
  </React.StrictMode>,
);

// Con la primera pantalla ya dibujada se quita el «Abriendo AdminOps…» y se le
// dice al programa que enseñe la ventana: así nunca se ve vacía ni en negro.
// Dos fotogramas: el primero encola el pintado, el segundo ocurre ya pintado.
requestAnimationFrame(() =>
  requestAnimationFrame(() => {
    document.getElementById("boot")?.remove();
    if (!isNote) void appApi.uiReady();
  }),
);
