import { useEffect } from "react";
import { invoke } from "../lib/api/core";
import { useToast } from "./feedback";

interface SelfCheckLine {
  text: string;
  state: "ok" | "fixed" | "warn" | "failed";
}

/**
 * La primera vez que se abre una versión nueva, AdminOps se comprueba a sí misma (appcare.rs) y arregla lo que pueda.
 * Si todo estaba bien no dice nada; si corrigió algo o hay algo que mirar, lo cuenta en un aviso.
 */
export function SelfCheck() {
  const toast = useToast();
  useEffect(() => {
    let alive = true;
    // Un poco después de abrir, cuando la primera pantalla ya está pintada.
    const t = window.setTimeout(() => {
      invoke<SelfCheckLine[]>("selfcheck_after_update")
        .then((lines) => {
          if (!alive || lines.length === 0) return;
          // Una actualización que no abrió y se deshizo: aviso aparte, sin el «Comprobada la versión nueva».
          const failed = lines.filter((l) => l.state === "failed");
          const rest = lines.filter((l) => l.state !== "failed");
          if (failed.length) toast("error", failed.map((l) => l.text).join(" "));
          if (rest.length) {
            const warn = rest.some((l) => l.state === "warn");
            toast(warn ? "error" : "ok", `Comprobada la versión nueva: ${rest.map((l) => l.text).join(" ")}`);
          }
        })
        .catch(() => {});
    }, 4000);
    return () => {
      alive = false;
      window.clearTimeout(t);
    };
  }, [toast]);
  return null;
}
