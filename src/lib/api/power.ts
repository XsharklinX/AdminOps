// Suspensión y pantalla según el uso (power.rs).
import { invoke } from "./core";

/** Minutos (0 = nunca) y qué hace la tapa: 0 nada, 1 suspender, 2 hibernar, 3 apagar. */
export interface PowerSide {
  screenMin: number;
  sleepMin: number;
  lid: number;
}

export interface PowerState {
  ac: PowerSide;
  dc: PowerSide;
  lidPresent: boolean;
  battery: boolean;
  profile: string | null;
  admin: boolean;
}

export interface PowerProfile {
  id: "office" | "present" | "always" | "dock";
  ac: PowerSide;
  dc: PowerSide;
}

export const powerApi = {
  state: () => invoke<PowerState>("power_state"),
  profiles: () => invoke<PowerProfile[]>("power_profiles"),
  set: (ac: PowerSide, dc: PowerSide) => invoke<PowerState>("power_set", { ac, dc }),
  wake: () => invoke<string>("power_wake"),
};
