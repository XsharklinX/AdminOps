import React from "react";
import ReactDOM from "react-dom/client";
import App from "./App";
import "./index.css";

// Sin menú contextual del navegador: esto es una app de escritorio, no una web.
document.addEventListener("contextmenu", (e) => e.preventDefault());

ReactDOM.createRoot(document.getElementById("root") as HTMLElement).render(
  <React.StrictMode>
    <App />
  </React.StrictMode>,
);
