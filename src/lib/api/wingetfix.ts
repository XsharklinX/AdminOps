// Cuando winget no funciona (wingetfix.rs).
import { invoke } from "./core";

export interface WingetHealth {
  state: "ok" | "missing" | "old" | "source" | "network";
  version: string | null;
  detail: string;
}

export interface WingetStep {
  id: "register" | "update" | "sources";
  state: "run" | "ok" | "fail" | "skip";
  text: string;
}

export const WINGET_STEP_EVENT = "winget://step";

export const wingetApi = {
  check: () => invoke<WingetHealth>("winget_check"),
  repair: () => invoke<WingetHealth>("winget_repair"),
};
