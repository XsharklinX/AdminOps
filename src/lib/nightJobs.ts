// Trabajos de noche (nightjobs.rs): lo que tarda horas se apunta y AdminOps lo
// hace a partir de la hora elegida, con el equipo sin suspenderse. AdminOps
// tiene que quedarse abierta (minimizada o junto al reloj).
import { invoke } from "./api/core";

export type NightJob =
  | { kind: "surface"; disk: number; model: string; hdd: boolean; mode: "quick" | "full" }
  | { kind: "repair" }
  | { kind: "smartLong"; disk: number; model: string };

export interface Queued {
  id: string;
  job: NightJob;
  title: string;
  added: number;
}

export interface Outcome {
  id: string;
  title: string;
  started: number;
  ended: number;
  level: "ok" | "warn" | "bad";
  summary: string;
  page: string;
  section: string | null;
}

export interface NightState {
  queue: Queued[];
  at: string;
  lastNight: string | null;
  results: Outcome[];
  unseen: number;
  running: string | null;
}

export const nightApi = {
  get: () => invoke<NightState>("night_jobs"),
  add: (job: NightJob) => invoke<NightState>("night_job_add", { job }),
  remove: (id: string) => invoke<NightState>("night_job_remove", { id }),
  setTime: (at: string) => invoke<NightState>("night_set_time", { at }),
  runNow: () => invoke<void>("night_run_now"),
  seen: () => invoke<void>("night_seen"),
};

/** Apunta un trabajo para esta noche. */
export const scheduleNight = (job: NightJob) => nightApi.add(job);
