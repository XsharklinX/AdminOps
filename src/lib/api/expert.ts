// Diagnóstico de experto (1.2.8): todo lo que arranca, pantallazos con su
// driver, prueba de estrés, fugas y disco saturado, Visor de eventos agrupado.
import { invoke } from "./core";

export interface Autorun {
  id: string;
  category: "run" | "startup" | "task" | "service" | "driver" | "winlogon" | "ifeo" | "appinit" | "explorer" | "wmi";
  location: string;
  name: string;
  command: string;
  image: string;
  exists: boolean;
  signed: boolean;
  publisher: string;
  enabled: boolean;
  flags: string[];
  concern: "none" | "low" | "high";
  canDisable: boolean;
}

export interface StoreDriver {
  published: string;
  original: string;
  provider: string;
  class: string;
  date: string;
  version: string;
}

export interface CrashGroup {
  driver: string;
  hint: string;
  count: number;
  codes: string[];
  first: number;
  last: number;
  device: string;
  version: string;
  driverDate: string;
  installed: number;
  sinceInstall: boolean;
  inf: string;
  older: StoreDriver[];
}

export interface CrashReport {
  groups: CrashGroup[];
  withoutDump: number;
  verifier: string[];
}

export interface StressSample {
  t: number;
  temp: number | null;
  mhz: number | null;
  fan: number | null;
  watts: number | null;
}

export interface StressResult {
  at: number;
  seconds: number;
  memory: boolean;
  samples: StressSample[];
  maxTemp: number | null;
  stoppedHot: boolean;
  cancelled: boolean;
  clockRatio: number | null;
  verdict: string[];
  level: "ok" | "warn" | "bad" | "";
}

export interface Leak {
  name: string;
  fromMb: number;
  toMb: number;
  hours: number;
  at: number;
  series: [number, number][];
}

export interface PerfInsights {
  samples: number;
  hours: number;
  leaks: Leak[];
  saturatedMinutes: number;
  diskHogs: { name: string; minutes: number; avgKbs: number }[];
  peak: { hour: number; cpu: number; busy: number; names: string[] } | null;
}

export interface EventGroup {
  provider: string;
  id: number;
  log: string;
  count: number;
  last: number;
  weight: "matter" | "watch" | "noise";
  title: string;
  meaning: string;
  todo: string;
  page: string;
  sample: string;
}

export const expertApi = {
  autoruns: () => invoke<Autorun[]>("autoruns_list"),
  disableAutorun: (id: string) => invoke<string>("autorun_disable", { id }),
  crashes: () => invoke<CrashReport>("crash_report"),
  rollback: (inf: string) => invoke<string>("driver_rollback", { inf }),
  verifierOn: (drivers: string[]) => invoke<string>("verifier_enable", { drivers }),
  verifierOff: () => invoke<string>("verifier_disable"),
  stress: (seconds: number, memory: boolean) => invoke<StressResult>("stress_run", { seconds, memory }),
  stressLive: () => invoke<StressResult | null>("stress_live"),
  stressHistory: () => invoke<StressResult[]>("stress_history"),
  insights: () => invoke<PerfInsights>("perf_insights"),
  events: (days: number) => invoke<EventGroup[]>("event_digest", { days }),
};
