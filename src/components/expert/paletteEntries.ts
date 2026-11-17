// Entradas de Ctrl+K del diagnóstico de experto: llevan a la pantalla donde se hace.
import type { PageId } from "../Sidebar";

export interface ExpertEntry {
  id: string;
  title: string;
  subtitle: string;
  keywords: string;
  page: PageId;
  focus: string | null;
}

export const EXPERT_ENTRIES: ExpertEntry[] = [
  {
    id: "whynot",
    title: "¿Por qué no responde este programa?",
    subtitle: "Procesos → elige el programa → «¿Por qué?»",
    keywords: "colgado no responde congelado bloqueado espera excel outlook cuelga",
    page: "processes",
    focus: null,
  },
  {
    id: "whoholds",
    title: "¿Quién tiene este archivo?",
    subtitle: "Qué programa tiene un archivo, carpeta o unidad agarrada",
    keywords: "archivo en uso abierto en otro programa no se puede expulsar usb no deja borrar carpeta bloqueado",
    page: "tools",
    focus: "whoholds",
  },
  {
    id: "errorlens",
    title: "Leer un error de una imagen",
    subtitle: "Pega o suelta el recorte o la foto: dice qué significa el código",
    keywords: "lector de errores captura recorte foto whatsapp ocr codigo de error mensaje",
    page: "troubleshoot",
    focus: "errorlens",
  },
  {
    id: "testuser",
    title: "¿Es el perfil o el equipo?",
    subtitle: "Crea un usuario local de prueba que se borra solo",
    keywords: "usuario de prueba perfil dañado perfil corrupto nuevo usuario temporal",
    page: "troubleshoot",
    focus: "testuser",
  },
  {
    id: "bisect",
    title: "Encontrar el cambio que lo rompió",
    subtitle: "Historial del equipo: deshace por mitades hasta dar con el culpable",
    keywords: "bisecar culpable ajuste que rompio dejo de funcionar despues de cambios deshacer",
    page: "history",
    focus: null,
  },
];
