// Conocimiento que crece (1.2.8): lo que funcionó, paquete para escalar y
// archivos de la biblioteca compartida.
import { invoke } from "./core";

export interface LearnedStat {
  fix: string;
  count: number;
  pct: number;
}

export const knowledgeApi = {
  record: (topic: string, fix: string, client = "") => invoke<void>("learned_record", { topic, fix, client }),
  stats: (topic: string, client = "") => invoke<LearnedStat[]>("learned_stats", { topic, client }),
  escalate: (summary: string) => invoke<string>("escalation_pack", { summary }),
  saveText: (name: string, content: string, ext: string) => invoke<string | null>("save_text_file", { name, content, ext }),
  openText: (ext: string) => invoke<string | null>("open_text_file", { ext }),
};
