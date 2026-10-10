// Detalles de la interfaz que necesitan al sistema (uxio.rs).
import { invoke } from "./core";

export interface Dropped {
  path: string;
  name: string;
  size: number;
  /** image · vault · profile · config · pdf · dump · unknown */
  kind: string;
}

export const uxApi = {
  mediaInUse: () => invoke<boolean>("media_in_use"),
  describeFile: (path: string) => invoke<Dropped>("describe_file", { path }),
  revealPath: (path: string) => invoke<void>("reveal_path", { path }),
  openPdf: (path: string) => invoke<void>("open_dropped_pdf", { path }),
  composeMail: (subject: string, body: string) => invoke<void>("compose_mail", { subject, body }),
};
