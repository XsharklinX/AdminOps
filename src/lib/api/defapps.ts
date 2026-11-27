// Programas predeterminados (defapps.rs).
import { invoke } from "./core";

export interface DefCandidate {
  name: string;
  machine: boolean;
}

export interface DefRow {
  id: string;
  label: string;
  current: string;
  candidates: DefCandidate[];
  office: string | null;
}

export interface DefaultsView {
  rows: DefRow[];
  domain: boolean;
  policy: boolean;
  admin: boolean;
}

export const defappsApi = {
  view: () => invoke<DefaultsView>("defapps_view"),
  setOffice: (id: string, name: string) => invoke<void>("defapps_set_office", { id, name }),
  apply: () => invoke<number>("defapps_apply"),
  /** Lo mismo para los usuarios que se creen después (DISM; también fuera de dominio). */
  applyNewUsers: () => invoke<number>("defapps_apply_new_users"),
  removePolicy: () => invoke<void>("defapps_remove_policy"),
  openSettings: (name: string, machine: boolean) => invoke<void>("defapps_open_settings", { name, machine }),
};

/** Filas donde lo de ahora no es lo de la oficina (y la oficina eligió algo que hay en el equipo). */
export function pendingRows(v: DefaultsView): DefRow[] {
  return v.rows.filter((r) => r.office && r.office !== r.current && r.candidates.some((c) => c.name === r.office));
}
