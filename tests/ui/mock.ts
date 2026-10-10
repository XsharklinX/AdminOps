// Simulador del sistema para las pruebas de la interfaz (tests/ui/smoke.mjs):
// responde a las órdenes que la interfaz manda a Rust con datos de ejemplo
// (los del modo demostración) o con un valor vacío del tipo que espera cada una.
// Solo se carga desde tests/ui/index.html, nunca en la aplicación.
import { demoAnswer } from "../../src/lib/demo";
import { FIXTURES } from "./fixtures";

type Defaults = Record<string, "array" | "null" | "string" | "bool" | "number">;
const w = window as unknown as {
  __TAURI_INTERNALS__: unknown;
  __TAURI_EVENT_PLUGIN_INTERNALS__: unknown;
  __E2E_DEFAULTS__?: Defaults;
  __E2E_CALLS__: string[];
  __E2E_UNKNOWN__: Set<string>;
};

const callbacks = new Map<number, (v: unknown) => void>();
let nextId = 1;
w.__E2E_CALLS__ = [];
w.__E2E_UNKNOWN__ = new Set();

function answer(cmd: string, args: Record<string, unknown> | undefined): unknown {
  if (cmd.startsWith("plugin:event|listen")) return nextId++;
  if (cmd.startsWith("plugin:")) return null;
  if (cmd in FIXTURES) {
    const f = FIXTURES[cmd];
    return typeof f === "function" ? (f as (a: unknown) => unknown)(args) : structuredClone(f);
  }
  const demo = demoAnswer(cmd);
  if (demo !== undefined) return demo;
  const kind = w.__E2E_DEFAULTS__?.[cmd];
  if (!kind) w.__E2E_UNKNOWN__.add(cmd);
  switch (kind) {
    case "array":
      return [];
    case "string":
      return "";
    case "bool":
      return false;
    case "number":
      return 0;
    default:
      return null;
  }
}

w.__TAURI_INTERNALS__ = {
  metadata: { currentWindow: { label: "main" }, currentWebview: { windowLabel: "main", label: "main" } },
  transformCallback(cb: (v: unknown) => void) {
    const id = nextId++;
    callbacks.set(id, cb);
    return id;
  },
  unregisterCallback(id: number) {
    callbacks.delete(id);
  },
  convertFileSrc: (p: string) => p,
  async invoke(cmd: string, args?: Record<string, unknown>) {
    w.__E2E_CALLS__.push(cmd);
    // Un poco de espera, como el sistema de verdad: así se ven los esqueletos.
    await new Promise((r) => setTimeout(r, 5));
    return answer(cmd, args);
  },
};
w.__TAURI_EVENT_PLUGIN_INTERNALS__ = { unregisterListener() {} };
