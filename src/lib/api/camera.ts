// «No me ven en Teams»: la cámara (camfix.rs).
import { invoke } from "./core";

export interface CamDevice {
  id: string;
  name: string;
  state: "ok" | "disabled" | "error";
}

export interface CamView {
  cameras: CamDevice[];
  privacy: { device: boolean; apps: boolean; desktop: boolean; teamsNew: boolean | null };
  inUse: string[];
  admin: boolean;
}

export const camApi = {
  view: () => invoke<CamView>("cam_view"),
  allow: () => invoke<string[]>("cam_allow"),
  enable: (id: string, enable: boolean) => invoke<void>("cam_enable", { id, enable }),
};
