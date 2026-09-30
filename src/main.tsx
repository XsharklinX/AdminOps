import React from "react";
import ReactDOM from "react-dom/client";
import App from "./App";
import { ErrorBoundary } from "./components/ErrorBoundary";
import { appApi } from "./lib/api";
import { showFullTextOnHover } from "./lib/fullTextOnHover";
import { applyAppearance } from "./lib/prefs";
import { applyTheme, getTheme } from "./lib/theme";
import "./index.css";

// Lo primero de todo: decirle al programa que el código ya se está ejecutando.
// En un equipo viejo pueden pasar diez segundos largos de aquí a la primera
// pantalla, y sin esta señal el vigilante del arranque lo tomaría por WebView2
// muerto y recargaría, dejándolo peor.
void appApi.uiBooting();

applyTheme(getTheme());
applyAppearance();

// Sin menú contextual del navegador: esto es una app de escritorio, no una web.
document.addEventListener("contextmenu", (e) => e.preventDefault());

// El texto recortado con «…» se puede leer entero pasando el ratón por encima.
showFullTextOnHover();

// Errores fuera de React (eventos, promesas sin capturar): al registro técnico.
window.addEventListener("error", (e) => appApi.logError(`${e.message} @ ${e.filename}:${e.lineno}`));
window.addEventListener("unhandledrejection", (e) => appApi.logError(`Promesa sin capturar: ${String(e.reason)}`));

ReactDOM.createRoot(document.getElementById("root") as HTMLElement).render(
  <React.StrictMode>
    <ErrorBoundary>
      <App />
    </ErrorBoundary>
  </React.StrictMode>,
);

// Con la primera pantalla ya dibujada se quita el «Abriendo AdminOps…» y se le
// dice al programa que enseñe la ventana: así nunca se ve vacía ni en negro.
// Dos fotogramas: el primero encola el pintado, el segundo ocurre ya pintado.
requestAnimationFrame(() =>
  requestAnimationFrame(() => {
    document.getElementById("boot")?.remove();
    void appApi.uiReady();
  }),
);
