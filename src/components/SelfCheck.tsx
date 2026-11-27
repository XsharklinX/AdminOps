import { useEffect } from "react";
import { invoke } from "../lib/api/core";
import { useToast } from "./feedback";

interface SelfCheckLine {
  text: string;
  state: "ok" | "fixed" | "warn";
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
          const warn = lines.some((l) => l.state === "warn");
          toast(warn ? "error" : "ok", `Comprobada la versión nueva: ${lines.map((l) => l.text).join(" ")}`);
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
