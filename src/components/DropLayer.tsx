// Soltar archivos sobre la ventana: AdminOps entiende qué son y los lleva a
// donde se usan. Una imagen de disco va a Recuperar archivos, un .zip cifrado a
// la Caja fuerte, un perfil .json a Perfiles y un PDF se abre. Si no lo
// reconoce, lo dice en lugar de no hacer nada.
import { listen } from "@tauri-apps/api/event";
import { FileDown } from "lucide-react";
import { useEffect, useRef, useState } from "react";
import type { PageId } from "./Sidebar";
import { useToast } from "./feedback";
import { logQuietly, uxApi, type Dropped } from "../lib/api";
import { deliverDropped } from "../lib/dropped";
import { bytes } from "../lib/format";

/** Qué se hará con un archivo, en una frase. */
export function dropAction(kind: string): string {
  switch (kind) {
    case "image":
      return "Abrir la imagen en Recuperar archivos";
    case "vault":
      return "Abrir el .zip cifrado en la Caja fuerte";
    case "profile":
      return "Importar el perfil en Perfiles";
    case "config":
      return "Importar la configuración desde Ajustes";
    case "pdf":
      return "Abrir el PDF";
    case "dump":
      return "Analizar el volcado en Arranques y cuelgues";
    default:
      return "AdminOps no sabe qué hacer con este tipo de archivo";
  }
}

const webview = () => (window as unknown as { chrome?: { webview?: { postMessageWithAdditionalObjects?: (m: string, o: unknown) => void } } }).chrome?.webview;

export function DropLayer({ onNavigate }: { onNavigate: (page: PageId, section: string | null) => void }) {
  const toast = useToast();
  const [over, setOver] = useState(false);
  const depth = useRef(0);
  // El texto de un perfil soltado (se lee en la interfaz: es pequeño).
  const profileText = useRef<string | null>(null);

  useEffect(() => {
    const handle = (files: Dropped[]) => {
      for (const f of files) {
        switch (f.kind) {
          case "image":
            deliverDropped("image", f.path);
            onNavigate("recover", null);
            toast("info", `${f.name} (${bytes(f.size)}) lista en Recuperar archivos → por firma.`);
            break;
          case "vault":
            deliverDropped("vault", f.path);
            onNavigate("vault", null);
            break;
          case "profile":
            if (profileText.current) deliverDropped("profile", profileText.current);
            onNavigate("profiles", null);
            break;
          case "config":
            onNavigate("settings", "data");
            toast("info", `Para importar «${f.name}», usa Ajustes → Datos → Importar configuración.`);
            break;
          case "pdf":
            void uxApi.openPdf(f.path).catch((e) => toast("error", String(e)));
            break;
          case "dump":
            onNavigate("machine", "boots");
            toast("info", "Los volcados de pantallazo se leen solos en Arranques y cuelgues.");
            break;
          default:
            toast("info", `«${f.name}»: ${dropAction(f.kind)}.`);
        }
      }
    };
    const off = listen<Dropped[]>("files-dropped", ({ payload }) => handle(payload));
    return () => void off.then((f) => f());
  }, [onNavigate, toast]);

  useEffect(() => {
    const hasFiles = (e: DragEvent) => !!e.dataTransfer?.types.includes("Files");
    const enter = (e: DragEvent) => {
      if (!hasFiles(e)) return;
      depth.current++;
      setOver(true);
    };
    const leave = (e: DragEvent) => {
      if (!hasFiles(e)) return;
      depth.current = Math.max(0, depth.current - 1);
      if (!depth.current) setOver(false);
    };
    const overEv = (e: DragEvent) => {
      if (!hasFiles(e)) return;
      e.preventDefault();
      if (e.dataTransfer) e.dataTransfer.dropEffect = "copy";
    };
    const drop = (e: DragEvent) => {
      if (!hasFiles(e)) return;
      e.preventDefault();
      depth.current = 0;
      setOver(false);
      const files = Array.from(e.dataTransfer?.files ?? []);
      if (!files.length) return;
      const json = files.find((f) => f.name.toLowerCase().endsWith(".json") && f.size < 2_000_000);
      const send = () => {
        const wv = webview();
        if (wv?.postMessageWithAdditionalObjects) wv.postMessageWithAdditionalObjects("adminops-drop", e.dataTransfer?.files ?? files);
        else toast("error", "Esta versión de WebView2 no deja leer la ruta de lo soltado. Actualiza WebView2 o elige el archivo desde su pantalla.");
      };
      if (json) {
        json
          .text()
          .then((t) => (profileText.current = t))
          .catch(logQuietly("DropLayer"))
          .finally(send);
      } else send();
    };
    window.addEventListener("dragenter", enter);
    window.addEventListener("dragleave", leave);
    window.addEventListener("dragover", overEv);
    window.addEventListener("drop", drop);
    return () => {
      window.removeEventListener("dragenter", enter);
      window.removeEventListener("dragleave", leave);
      window.removeEventListener("dragover", overEv);
      window.removeEventListener("drop", drop);
    };
  }, [toast]);

  if (!over) return null;
  return (
    <div className="pointer-events-none fixed inset-0 z-[65] grid place-items-center bg-void/70 p-8 backdrop-blur-[2px]">
      <div className="flex max-w-lg flex-col items-center gap-3 rounded-2xl border-2 border-dashed border-neon/60 bg-panel px-10 py-8 text-center">
        <FileDown size={34} className="text-neon" />
        <p className="text-base font-semibold text-ink">Suelta para abrirlo en AdminOps</p>
        <ul className="space-y-0.5 text-xs text-dim">
          <li>Imagen de disco (.img, .vhd, .iso) → Recuperar archivos</li>
          <li>.zip cifrado → Caja fuerte</li>
          <li>Perfil (.json) → Perfiles</li>
          <li>Informe (.pdf) → se abre</li>
        </ul>
      </div>
    </div>
  );
}
