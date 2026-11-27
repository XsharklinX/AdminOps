// «No me oyen en Teams» (audiofix.rs).
import { invoke } from "./core";

export interface AudioDev {
  id: string;
  name: string;
  default: boolean;
  defaultComm: boolean;
  volume: number;
  muted: boolean;
}

export interface MicPrivacy {
  device: boolean;
  apps: boolean;
  desktop: boolean;
  teamsNew: boolean | null;
}

export interface AudioView {
  capture: AudioDev[];
  render: AudioDev[];
  privacy: MicPrivacy;
}

/** Niveles en vivo mientras se escucha: [id, pico 0–1 o null si no se pudo abrir]; null al terminar. */
export const AUDIO_LEVELS_EVENT = "audio://levels";

export const audioApi = {
  view: () => invoke<AudioView>("audio_view"),
  listen: (secs: number) => invoke<void>("audio_listen", { secs }),
  beep: (id: string) => invoke<void>("audio_beep", { id }),
  use: (id: string, capture: boolean, volume: number) => invoke<string[]>("audio_use", { id, capture, volume }),
  allowMic: () => invoke<string[]>("audio_allow_mic"),
  /** Graba unos segundos de ese micrófono y los reproduce por ese altavoz; devuelve el pico (0–1). */
  echo: (mic: string, speaker: string, secs: number) => invoke<number>("audio_echo", { mic, speaker, secs }),
};
