// «¿Qué cambió?» y «Pregúntale al equipo».
import { invoke } from "./core";

export interface WhatChangedInfo {
  /** Primer día con foto en este equipo (vacío: todavía ninguna). */
  firstDay: string;
  lastDay: string;
  /** Días en que cambió algo. */
  days: string[];
}

export interface ChangeFix {
  label: string;
  page: string;
  focus: string | null;
}

export type ChangeKind = "program" | "driver" | "update" | "service" | "startup" | "power";

export interface ChangeView {
  kind: ChangeKind;
  change: "added" | "removed" | "changed";
  label: string;
  old: string | null;
  new: string | null;
  day: string;
  fix: ChangeFix | null;
}

export const whatChangedApi = {
  info: () => invoke<WhatChangedInfo>("whatchanged_info"),
  /** Hace la foto de hoy ahora. */
  snapshot: () => invoke<WhatChangedInfo>("whatchanged_snapshot"),
  /** Lo que es distinto desde ese día (AAAA-MM-DD). */
  since: (day: string) => invoke<ChangeView[]>("whatchanged_since", { day }),
};

export interface Evidence {
  t: number | null;
  text: string;
}

export interface PcAnswer {
  question: string;
  answer: string;
  known: boolean;
  evidence: Evidence[];
  lookAt: string;
  page: string | null;
  focus: string | null;
}

export type PcQuestion = "restart" | "crash" | "slow" | "installed" | "printer";

export const askPcApi = {
  ask: (question: PcQuestion, arg?: string, since?: string) => invoke<PcAnswer>("ask_pc", { question, arg, since }),
};
