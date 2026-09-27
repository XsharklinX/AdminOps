import React from "react";
import ReactDOM from "react-dom/client";
import App from "./App";
import { ErrorBoundary } from "./components/ErrorBoundary";
import { appApi } from "./lib/api";
import { applyTheme, getTheme } from "./lib/theme";
import "./index.css";

applyTheme(getTheme());

// Sin menú contextual del navegador: esto es una app de escritorio, no una web.
document.addEventListener("contextmenu", (e) => e.preventDefault());

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
